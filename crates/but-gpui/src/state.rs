//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's been cut, and the operation in flight.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    AppContext as _, AsyncApp, Context, EventEmitter, SharedString, Subscription, Task, WeakEntity,
};
use serde::{Deserialize, Serialize};

use crate::backend::watch::Watcher;
use crate::backend::{
    self, Branch, Commit, CommitId, FileChange, Project, PushState, Selected, SnapshotId, Workspace,
};
use crate::transfer::short_ref;

pub type ProjectId = SharedString;

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Project(ProjectId),
    Uncommitted(ProjectId),
    Branch(ProjectId, String),
    Commit(ProjectId, CommitId),
}

impl Selection {
    pub fn project(&self) -> &ProjectId {
        match self {
            Selection::Project(project)
            | Selection::Uncommitted(project)
            | Selection::Branch(project, _)
            | Selection::Commit(project, _) => project,
        }
    }
}

/// What ⌘X picked up, waiting for ⌘V on a destination.
#[derive(Debug, Clone)]
pub enum Cut {
    Commit(ProjectId, CommitId),
    /// Files from the worktree (`from: None`), down to the hunks checked, or from a commit.
    Files {
        project: ProjectId,
        from: Option<CommitId>,
        /// Shared, since every row a drag passes over works out what a drop would do.
        files: Arc<[Selected]>,
    },
}

impl Cut {
    pub fn project(&self) -> &ProjectId {
        match self {
            Cut::Commit(project, _) | Cut::Files { project, .. } => project,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Cut::Commit(_, id) => format!("commit {}", id.short()),
            Cut::Files { files, .. } if files.len() == 1 => files[0].file.path.clone(),
            Cut::Files { files, .. } => format!("{} files", files.len()),
        }
    }
}

/// How long a mutation runs before the status bar says how long it's taking.
pub const SLOW: Duration = Duration::from_millis(800);

#[derive(Debug)]
pub enum StoreEvent {
    /// A project's workspace was (re)loaded.
    Loaded(ProjectId),
    SelectionChanged,
    /// Files of a project were marked reviewed or not, which folds or unfolds them.
    Reviewed(ProjectId),
    /// An operation on a project was queued, started or finished, which hides or shows
    /// what it takes away.
    Pending(ProjectId),
}

/// What a running or queued operation takes away, hidden until the reload after it shows
/// the result, and the commit it makes, shown as a stand-in meanwhile. Nothing here predicts
/// what git will do; the reload always has the last word.
#[derive(Default)]
pub struct Overlay {
    /// Commits leaving, by key.
    pub commits: HashSet<String>,
    /// Uncommitted files leaving whole, by path.
    pub files: HashSet<String>,
    /// The branch (full ref) a new commit goes on top of, and its title.
    pub new_commit: Option<(String, SharedString)>,
}

impl Overlay {
    pub fn commits(keys: impl IntoIterator<Item = String>) -> Self {
        Self {
            commits: keys.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Hides the files taken whole; ones with only some hunks taken stay.
    pub fn files<'a>(selected: impl IntoIterator<Item = &'a backend::Selected>) -> Self {
        Self {
            files: selected
                .into_iter()
                .filter(|s| s.hunks.is_none())
                .map(|s| s.file.path.clone())
                .collect(),
            ..Self::default()
        }
    }
}

/// The commits an operation captured, found again by key when it runs, as operations queued
/// before it may have rewritten them.
pub struct Remap(HashMap<CommitId, CommitId>);

impl Remap {
    fn new(captured: &HashMap<CommitId, String>, now: Option<&Workspace>) -> Self {
        let now: HashMap<&str, CommitId> = now
            .into_iter()
            .flat_map(|ws| ws.branches())
            .flat_map(|branch| &branch.commits)
            .map(|commit| (commit.key.as_str(), commit.id))
            .collect();
        Self(
            captured
                .iter()
                .filter_map(|(id, key)| Some((*id, *now.get(key.as_str())?)))
                .collect(),
        )
    }

    /// `id` as it is now.
    pub fn get(&self, id: CommitId) -> Result<CommitId> {
        self.0
            .get(&id)
            .copied()
            .context("A commit it needed changed while it waited")
    }
}

/// An operation waiting for the ones before it.
struct Queued {
    project: ProjectId,
    label: SharedString,
    overlay: Overlay,
    /// The project's commits when it was asked for, for its [`Remap`].
    captured: HashMap<CommitId, String>,
    start: Start,
}

/// Starts a queued operation, with its commits as they are by then.
type Start = Box<dyn FnOnce(&mut Store, Remap, &mut Context<Store>)>;

/// Something to tell the user, shown the way GitButler's other apps do.
pub enum Notice {
    /// A one-line result at the bottom centre. `undo` is the oplog entry the operation
    /// recorded, which the snackbar's Undo restores.
    Snackbar {
        message: SharedString,
        undo: Option<(ProjectId, SnapshotId)>,
    },
    /// A failure: a card at the bottom right with the error kept for copying.
    Error {
        title: SharedString,
        detail: SharedString,
    },
}

/// Where an uncommitted file stands in review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    NotReviewed,
    Reviewed,
    /// Reviewed, then changed again.
    Changed,
}

pub struct Toast {
    pub id: usize,
    pub notice: Notice,
}

pub struct ProjectEntry {
    pub project: Project,
    pub id: ProjectId,
    pub expanded: bool,
    pub workspace: Option<Workspace>,
    pub error: Option<SharedString>,
    loading: bool,
    reload_queued: bool,
    /// The queued reload needs the commits too, not just the worktree.
    queued_full: bool,
    watcher: Option<Watcher>,
}

impl ProjectEntry {
    pub fn is_loading(&self) -> bool {
        self.loading && self.workspace.is_none()
    }
}

pub struct Store {
    pub projects: Vec<ProjectEntry>,
    pub projects_loaded: bool,
    /// Collapsed projects' cards still need their counts, once the open ones have loaded.
    summaries_pending: bool,
    pub selection: Option<Selection>,
    pub cut: Option<Cut>,
    /// What the pointer is dragging. Only meaningful while GPUI has a drag active.
    pub dragging: Option<Cut>,
    /// Pane widths the user dragged away from their defaults.
    widths: HashMap<Pane, f32>,
    /// What was open when the app last quit, until the first project listing uses it.
    restored: Restored,
    /// The saved state as last written, so unchanged state isn't written again.
    last_saved: Vec<u8>,
    /// File lists show a tree of directories rather than a flat list.
    pub file_tree: bool,
    pub toasts: Vec<Toast>,
    /// The label of the mutation in flight, if any. Mutations run one at a time, each
    /// until the reload after it lands, so the next one sees what it did.
    pub busy: Option<SharedString>,
    /// The running mutation's project, overlay and start.
    running: Option<(ProjectId, Overlay, Instant)>,
    /// Mutations asked for while one ran, in order.
    queue: VecDeque<Queued>,
    /// The project a finished mutation waits on. Its next load to start ends `busy`.
    settling: Option<ProjectId>,
    /// The project an operation is changing right now. Its own reload follows and sees all
    /// of it, so the watcher's reloads for it would only queue in front.
    mutating: Option<ProjectId>,
    /// The oplog entry the last mutation recorded, which the Undo offered for it restores.
    last_snapshot: Option<(ProjectId, SnapshotId)>,
    next_toast: usize,
    seen: Seen,
    /// The commits each project had before the app's last operation on it. Commits that
    /// appear after it are the app's own doing, so they load as already seen.
    rebaseline: HashMap<ProjectId, HashSet<String>>,
    save_task: Task<()>,
    save_queued: bool,
    _save_on_quit: Subscription,
    /// Projects whose files changed, and whether that included the git directory.
    changes: mpsc::UnboundedSender<(ProjectId, bool)>,
    _change_listener: Task<()>,
    _forge_poll: Task<()>,
    forge_requests: mpsc::UnboundedSender<ProjectId>,
    /// Projects waiting for a forge refresh, so each waits only once.
    forge_queued: HashSet<ProjectId>,
    /// Projects whose last forge refresh failed, which the minute poll leaves alone.
    forge_failed: HashSet<ProjectId>,
    _forge_worker: Task<()>,
}

impl EventEmitter<StoreEvent> for Store {}

impl Store {
    /// Projects listed and nothing loading or mutating, for the dev script's timings.
    #[cfg(feature = "dev-script")]
    pub fn is_settled(&self) -> bool {
        self.projects_loaded
            && self.busy.is_none()
            && self.queue.is_empty()
            && self.projects.iter().all(|p| !p.loading && !p.reload_queued)
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        let (changes, mut changed) = mpsc::unbounded::<(ProjectId, bool)>();
        let listener = cx.spawn(async move |this, cx| {
            // While an agent keeps writing, reload at most this often: each reload runs a
            // full status, and back to back they kept a core busy.
            const MIN_INTERVAL: Duration = Duration::from_millis(500);
            let mut last: Option<Instant> = None;
            while let Some((id, git)) = changed.next().await {
                // Watchers fire in bursts; let a burst settle before reloading.
                let throttle = last.map_or(Duration::ZERO, |at| {
                    MIN_INTERVAL.saturating_sub(at.elapsed())
                });
                cx.background_executor()
                    .timer(throttle.max(Duration::from_millis(60)))
                    .await;
                last = Some(Instant::now());
                let mut ids = HashMap::from([(id, git)]);
                while let Ok((id, git)) = changed.try_recv() {
                    *ids.entry(id).or_default() |= git;
                }
                let Ok(()) = this.update(cx, |store, cx| {
                    for (id, git) in ids {
                        if store.mutating.as_ref() == Some(&id) {
                            continue;
                        }
                        // Worktree files alone can't move commits or branches.
                        store.load(&id, git, cx);
                    }
                }) else {
                    break;
                };
            }
        });
        let saved = Saved::load();
        let (forge_requests, requested) = mpsc::unbounded::<ProjectId>();
        let mut store = Self {
            projects: Vec::new(),
            projects_loaded: false,
            summaries_pending: false,
            selection: None,
            cut: None,
            dragging: None,
            toasts: Vec::new(),
            busy: None,
            settling: None,
            mutating: None,
            running: None,
            queue: VecDeque::new(),
            last_snapshot: None,
            next_toast: 0,
            seen: saved.seen,
            widths: saved.widths,
            file_tree: saved.file_tree,
            restored: Restored {
                expanded: saved.expanded,
                selection: saved.selection,
            },
            last_saved: Vec::new(),
            rebaseline: HashMap::new(),
            save_task: Task::ready(()),
            save_queued: false,
            _save_on_quit: cx.on_app_quit(|store, _| {
                store.save();
                async {}
            }),
            changes,
            _change_listener: listener,
            forge_requests,
            forge_queued: HashSet::new(),
            forge_failed: HashSet::new(),
            _forge_worker: cx
                .spawn(async move |this, cx| Self::forge_worker(this, requested, cx).await),
            // Pull requests and CI change on the forge, where no watcher sees them: ask
            // when a project opens, then every minute.
            _forge_poll: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_secs(60))
                        .await;
                    // Not the projects that failed last time: that's a missing account or a
                    // keychain prompt the user turned down, and asking again every minute
                    // would only nag. Opening a project or pushing asks again.
                    let Ok(ids) = this.update(cx, |store, _| {
                        let open = store.projects.iter().filter(|e| e.expanded);
                        open.filter(|entry| !store.forge_failed.contains(&entry.id))
                            .map(|entry| entry.id.clone())
                            .collect::<Vec<_>>()
                    }) else {
                        break;
                    };
                    for id in ids {
                        if this
                            .update(cx, |store, cx| store.refresh_forge(&id, cx))
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }),
        };
        store.load_projects(cx);
        store
    }

    pub fn project(&self, id: &str) -> Option<&ProjectEntry> {
        self.projects.iter().find(|entry| entry.id == id)
    }

    fn project_mut(&mut self, id: &str) -> Option<&mut ProjectEntry> {
        self.projects.iter_mut().find(|entry| entry.id == id)
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.project(id)?.workspace.as_ref()
    }

    pub fn path(&self, id: &str) -> Option<PathBuf> {
        Some(self.project(id)?.project.path.clone())
    }

    /// The branch `selection` is, or the one holding the commit it is.
    pub fn branch_for(&self, selection: &Selection) -> Option<&Branch> {
        let workspace = self.workspace(selection.project())?;
        match selection {
            Selection::Branch(_, ref_name) => workspace.find_branch(ref_name),
            Selection::Commit(_, id) => workspace.find_commit(*id).map(|(branch, _)| branch),
            Selection::Project(_) | Selection::Uncommitted(_) => None,
        }
    }

    pub fn load_projects(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let projects = backend::run(backend::list_projects).await;
            this.update(cx, |store, cx| match projects {
                Ok(projects) => store.set_projects(projects, cx),
                Err(err) => store.error("Couldn't list projects", format!("{err:#}"), cx),
            })
            .ok();
        })
        .detach();
    }

    fn set_projects(&mut self, projects: Vec<Project>, cx: &mut Context<Self>) {
        // What was open last time applies to the first listing only; later ones add
        // projects the user just added, which start closed.
        let restored = std::mem::take(&mut self.restored);
        let previous: HashSet<ProjectId> = self.projects.iter().map(|p| p.id.clone()).collect();
        let mut old: Vec<ProjectEntry> = std::mem::take(&mut self.projects);
        self.projects = projects
            .into_iter()
            .map(|project| {
                let id: ProjectId = project.id.clone().into();
                if let Some(ix) = old.iter().position(|entry| entry.id == id) {
                    let mut entry = old.swap_remove(ix);
                    entry.project = project;
                    return entry;
                }
                ProjectEntry {
                    expanded: restored.expanded.contains(id.as_ref()),
                    id,
                    project,
                    workspace: None,
                    error: None,
                    loading: false,
                    reload_queued: false,
                    queued_full: false,
                    watcher: None,
                }
            })
            .collect();
        // A single project is always open.
        if self.projects.len() == 1 {
            self.projects[0].expanded = true;
        }
        let first_load = !self.projects_loaded;
        self.projects_loaded = true;

        let new: Vec<ProjectId> = self
            .projects
            .iter()
            .filter(|entry| !previous.contains(&entry.id))
            .map(|entry| entry.id.clone())
            .collect();
        for id in &new {
            if self.project(id).is_some_and(|entry| entry.expanded) {
                self.open_project(id, cx);
            }
        }
        if first_load {
            let open = restored
                .selection
                .and_then(|id| self.project(&id).filter(|entry| entry.expanded))
                .or_else(|| self.projects.iter().find(|entry| entry.expanded))
                .map(|entry| Selection::Uncommitted(entry.id.clone()));
            if let Some(selection) = open {
                self.select(Some(selection), cx);
            }
            self.summaries_pending = true;
            self.load_summaries(cx);
        }
        cx.notify();
    }

    /// Load collapsed projects one after another, so their cards can show counts.
    /// Waits for the open projects first, so their loads don't compete with these.
    fn load_summaries(&mut self, cx: &mut Context<Self>) {
        let opening = self
            .projects
            .iter()
            .any(|entry| entry.expanded && entry.workspace.is_none() && entry.error.is_none());
        if !self.summaries_pending || opening {
            return;
        }
        self.summaries_pending = false;
        let pending: Vec<(ProjectId, PathBuf)> = self
            .projects
            .iter()
            .filter(|entry| !entry.expanded && entry.workspace.is_none())
            .map(|entry| (entry.id.clone(), entry.project.path.clone()))
            .collect();
        cx.spawn(async move |this, cx| {
            for (id, path) in pending {
                let result = backend::run(move || backend::load_workspace(&path)).await;
                let Ok(()) = this.update(cx, |store, cx| {
                    if let Some(entry) = store.project_mut(&id)
                        && entry.workspace.is_none()
                        && !entry.loading
                    {
                        match result {
                            Ok(workspace) => store.workspace_loaded(&id, workspace, true, cx),
                            Err(err) => entry.error = Some(format!("{err:#}").into()),
                        }
                        cx.notify();
                    }
                }) else {
                    return;
                };
            }
        })
        .detach();
    }

    pub fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                backend::add_project(&path)?;
                backend::list_projects().map(|projects| (projects, path))
            })
            .await;
            this.update(cx, |store, cx| match result {
                Ok((projects, path)) => {
                    let added = projects
                        .iter()
                        .find(|project| project.path == path)
                        .map(|project| ProjectId::from(project.id.clone()));
                    store.set_projects(projects, cx);
                    if let Some(id) = added {
                        store.set_expanded(&id, true, cx);
                        store.select(Some(Selection::Uncommitted(id)), cx);
                    }
                }
                Err(err) => store.error("Couldn't add the project", format!("{err:#}"), cx),
            })
            .ok();
        })
        .detach();
    }

    pub fn set_expanded(&mut self, id: &ProjectId, expanded: bool, cx: &mut Context<Self>) {
        let Some(entry) = self.project_mut(id) else {
            return;
        };
        if entry.expanded == expanded {
            return;
        }
        entry.expanded = expanded;
        if expanded {
            self.open_project(id, cx);
        } else {
            entry.watcher = None;
            if self
                .selection
                .as_ref()
                .is_some_and(|selection| selection.project() == id)
            {
                self.select(Some(Selection::Project(id.clone())), cx);
            }
        }
        self.save_soon(cx);
        cx.notify();
    }

    /// Prepare `id` for display and start watching it, then load it.
    fn open_project(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        let Some(entry) = self.project_mut(id) else {
            return;
        };
        let project = entry.project.clone();
        // Hold off reloads until the project is ready.
        entry.loading = true;
        let changes = self.changes.clone();
        let id = id.clone();
        cx.spawn(async move |this, cx| {
            let watch_id = id.clone();
            let watcher = backend::run(move || {
                backend::activate(&project.path)?;
                backend::watch::watch(&project, move |git| {
                    changes.unbounded_send((watch_id.clone(), git)).ok();
                })
            })
            .await;
            this.update(cx, |store, cx| {
                let Some(entry) = store.project_mut(&id) else {
                    return;
                };
                entry.loading = false;
                entry.reload_queued = false;
                if watcher.is_ok() {
                    store.refresh_forge(&id, cx);
                }
                let Some(entry) = store.project_mut(&id) else {
                    return;
                };
                match watcher {
                    Ok(watcher) if entry.expanded => entry.watcher = Some(watcher),
                    Ok(_) => {}
                    Err(err) => {
                        tracing::warn!("Couldn't open {}: {err:#}", entry.project.name);
                        entry.error = Some(format!("{err:#}").into());
                    }
                }
                store.reload(&id, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Reload `id`'s workspace, coalescing requests that arrive while a load is running.
    pub fn reload(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        self.load(id, true, cx);
    }

    /// Load `id` again: all of it, or with `full` false just its uncommitted files on top
    /// of the commits already shown, which skips walking the commit graph.
    fn load(&mut self, id: &ProjectId, full: bool, cx: &mut Context<Self>) {
        let Some(entry) = self.project_mut(id) else {
            self.settled(id, cx);
            return;
        };
        if entry.loading {
            entry.reload_queued = true;
            entry.queued_full |= full;
            return;
        }
        entry.loading = true;
        // Loads of one project don't overlap, so these commits are still current.
        let shown = (!full).then(|| entry.workspace.clone()).flatten();
        let path = entry.project.path.clone();
        let id = id.clone();
        // Started after the mutation finished, so it sees what the mutation did.
        let settles = self.settling.as_ref() == Some(&id);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || match shown {
                Some(shown) => {
                    backend::load_uncommitted(&path).map(|uncommitted| backend::Workspace {
                        uncommitted,
                        ..shown
                    })
                }
                None => backend::load_workspace(&path),
            })
            .await;
            this.update(cx, |store, cx| {
                let Some(entry) = store.project_mut(&id) else {
                    store.settled(&id, cx);
                    return;
                };
                entry.loading = false;
                let again = std::mem::take(&mut entry.reload_queued);
                let again_full = std::mem::take(&mut entry.queued_full);
                match result {
                    Ok(workspace) => store.workspace_loaded(&id, workspace, !again, cx),
                    Err(err) => entry.error = Some(format!("{err:#}").into()),
                }
                // A queued reload has newer data; checking against this one could drop a
                // selection a mutation just made.
                if !again {
                    store.validate_selection(cx);
                }
                if settles {
                    store.settled(&id, cx);
                }
                cx.emit(StoreEvent::Loaded(id.clone()));
                cx.notify();
                if again {
                    store.load(&id, again_full, cx);
                }
                store.load_summaries(cx);
            })
            .ok();
        })
        .detach();
    }

    /// End the mutation in `busy` once `id` has loaded what it did, or can't load at all,
    /// and start the next one waiting.
    fn settled(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        if self.settling.as_ref() == Some(id) {
            self.settling = None;
            self.busy = None;
            self.running = None;
            cx.emit(StoreEvent::Pending(id.clone()));
            self.start_next(cx);
        }
    }

    /// Ask the forge about `id`'s pull requests and CI in the background, then show what
    /// changed. Quiet when it can't: plenty of projects have no forge account.
    pub fn refresh_forge(&mut self, id: &ProjectId, _cx: &mut Context<Self>) {
        if self.forge_queued.insert(id.clone()) {
            self.forge_requests.unbounded_send(id.clone()).ok();
        }
    }

    /// Works through forge refreshes one at a time. Reading the forge token can stop on a
    /// keychain prompt, and one prompt is plenty; it also keeps the forge's rate limit happy.
    async fn forge_worker(
        this: WeakEntity<Store>,
        mut requested: mpsc::UnboundedReceiver<ProjectId>,
        cx: &mut AsyncApp,
    ) {
        while let Some(id) = requested.next().await {
            let Ok(path) = this.update(cx, |store, _| {
                store.forge_queued.remove(&id);
                let open = store.project(&id).filter(|entry| entry.expanded);
                open.map(|entry| entry.project.path.clone())
            }) else {
                return;
            };
            let Some(path) = path else {
                continue;
            };
            // Pull requests show as soon as they're listed; their checks take a request each.
            let reviews_path = path.clone();
            let refreshed = backend::run(move || backend::refresh_reviews(&reviews_path)).await;
            let Ok(_) = this.update(cx, |store, _| match &refreshed {
                Ok(()) => store.forge_failed.remove(&id),
                Err(_) => store.forge_failed.insert(id.clone()),
            }) else {
                return;
            };
            if let Err(err) = refreshed {
                tracing::debug!(%id, "forge refresh failed: {err:#}");
                continue;
            }
            if this.update(cx, |store, cx| store.reload(&id, cx)).is_err() {
                return;
            }
            match backend::run(move || backend::refresh_checks(&path)).await {
                Ok(()) => {
                    if this.update(cx, |store, cx| store.reload(&id, cx)).is_err() {
                        return;
                    }
                }
                Err(err) => tracing::debug!(%id, "CI refresh failed: {err:#}"),
            }
        }
    }

    pub fn reload_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<ProjectId> = self
            .projects
            .iter()
            .filter(|entry| entry.expanded)
            .map(|entry| entry.id.clone())
            .collect();
        for id in ids {
            self.reload(&id, cx);
        }
    }

    /// Keep the selection pointing at something that still exists after a reload.
    fn validate_selection(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.clone() else {
            return;
        };
        let Some(workspace) = self.workspace(selection.project()) else {
            return;
        };
        let still_there = match &selection {
            Selection::Project(_) | Selection::Uncommitted(_) => true,
            Selection::Branch(_, ref_name) => workspace.find_branch(ref_name).is_some(),
            Selection::Commit(_, id) => workspace.find_commit(*id).is_some(),
        };
        if !still_there {
            self.select(
                Some(Selection::Uncommitted(selection.project().clone())),
                cx,
            );
        }
    }

    /// `settled` is false when a newer load is already queued behind this one.
    fn workspace_loaded(
        &mut self,
        id: &ProjectId,
        workspace: Workspace,
        settled: bool,
        cx: &mut Context<Self>,
    ) {
        let commits: HashSet<&str> = workspace
            .branches()
            .flat_map(|branch| &branch.commits)
            .map(|commit| commit.key.as_str())
            .collect();
        let first_sight = !self.seen.commits.contains_key(id.as_ref());
        let seen = self.seen.commits.entry(id.to_string()).or_default();
        seen.retain(|key| commits.contains(key.as_str()));
        // Nothing is news the first time a project shows up, and neither is what the
        // app's own operations created.
        let before = self.rebaseline.get(id);
        seen.extend(
            commits
                .iter()
                .filter(|key| first_sight || before.is_some_and(|before| !before.contains(**key)))
                .map(|key| key.to_string()),
        );
        if settled {
            self.rebaseline.remove(id);
        }
        let files: HashSet<&str> = workspace
            .uncommitted
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        if let Some(reviewed) = self.seen.files.get_mut(id.as_ref()) {
            reviewed.retain(|path, _| files.contains(path.as_str()));
        }
        // An agent amending or rebasing the selected commit gives it a new id but keeps its
        // change id; stay on it.
        let rewritten = match &self.selection {
            Some(Selection::Commit(project, commit)) if project == id => self
                .workspace(id)
                .and_then(|old| old.find_commit(*commit))
                .filter(|_| workspace.find_commit(*commit).is_none())
                .and_then(|(_, old)| {
                    let mut commits = workspace.branches().flat_map(|branch| &branch.commits);
                    commits.find(|new| new.key == old.key).map(|new| new.id)
                }),
            _ => None,
        };
        if let Some(entry) = self.project_mut(id) {
            entry.workspace = Some(workspace);
            entry.error = None;
        }
        if let Some(commit) = rewritten {
            self.select(Some(Selection::Commit(id.clone(), commit)), cx);
        }
        self.save_soon(cx);
    }

    pub fn is_unread(&self, project: &str, commit: &Commit) -> bool {
        self.seen
            .commits
            .get(project)
            .is_some_and(|seen| !seen.contains(&commit.key))
    }

    pub fn review(&self, project: &str, file: &FileChange) -> Review {
        match self
            .seen
            .files
            .get(project)
            .and_then(|files| files.get(&file.path))
        {
            None => Review::NotReviewed,
            Some(version) if *version == file.modified.unwrap_or(0) => Review::Reviewed,
            Some(_) => Review::Changed,
        }
    }

    /// Uncommitted files of `project` that still want a look.
    pub fn to_review(&self, project: &str) -> usize {
        self.workspace(project).map_or(0, |ws| {
            ws.uncommitted
                .iter()
                .filter(|file| self.review(project, file) != Review::Reviewed)
                .count()
        })
    }

    pub fn set_reviewed(
        &mut self,
        project: &ProjectId,
        files: &[FileChange],
        reviewed: bool,
        cx: &mut Context<Self>,
    ) {
        let entry = self.seen.files.entry(project.to_string()).or_default();
        for file in files {
            if reviewed {
                entry.insert(file.path.clone(), file.modified.unwrap_or(0));
            } else {
                entry.remove(&file.path);
            }
        }
        cx.emit(StoreEvent::Reviewed(project.clone()));
        self.save_soon(cx);
        cx.notify();
    }

    pub fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        if self.selection == selection {
            return;
        }
        if let Some(Selection::Commit(project, id)) = &selection
            && let Some((_, commit)) = self.workspace(project).and_then(|ws| ws.find_commit(*id))
        {
            let key = commit.key.clone();
            self.seen
                .commits
                .entry(project.to_string())
                .or_default()
                .insert(key);
        }
        self.selection = selection;
        self.save_soon(cx);
        cx.emit(StoreEvent::SelectionChanged);
        cx.notify();
    }

    pub fn snack(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notify_user(
            Notice::Snackbar {
                message: message.into(),
                undo: None,
            },
            cx,
        );
    }

    /// A snackbar offering to undo the mutation that just ran.
    pub fn snack_undoable(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let undo = self.last_snapshot.clone();
        self.notify_user(
            Notice::Snackbar {
                message: message.into(),
                undo,
            },
            cx,
        );
    }

    pub fn error(
        &mut self,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.notify_user(
            Notice::Error {
                title: title.into(),
                detail: detail.into(),
            },
            cx,
        );
    }

    fn notify_user(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let id = self.next_toast;
        self.next_toast += 1;
        let linger = match notice {
            Notice::Snackbar { undo: None, .. } => Duration::from_secs(3),
            Notice::Snackbar { undo: Some(_), .. } => Duration::from_secs(6),
            Notice::Error { .. } => Duration::from_secs(15),
        };
        // One snackbar at a time; errors stack.
        if matches!(notice, Notice::Snackbar { .. }) {
            self.toasts
                .retain(|toast| !matches!(toast.notice, Notice::Snackbar { .. }));
        }
        self.toasts.push(Toast { id, notice });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(linger).await;
            this.update(cx, |store, cx| {
                store.toasts.retain(|toast| toast.id != id);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Confirm an operation, or warn about the changes it had to leave out.
    pub fn report_left_out(&mut self, done: &str, left_out: &[String], cx: &mut Context<Self>) {
        if left_out.is_empty() {
            self.snack_undoable(done.to_owned(), cx);
        } else {
            self.error(
                format!("{done}, with gaps"),
                format!("Left out {}", left_out.join(", ")),
                cx,
            );
        }
    }

    /// Restore `project` to before its last operation.
    pub fn undo(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        self.run(
            project,
            "Undo",
            Overlay::default(),
            |path, _| backend::undo(path),
            |store, undone, cx| match undone {
                Some(operation) => store.snack(format!("Undone: {operation}"), cx),
                None => store.snack("Nothing to undo", cx),
            },
            cx,
        );
    }

    /// Undo the operation that recorded `snapshot`, as long as nothing came after it.
    pub fn undo_snapshot(
        &mut self,
        project: &ProjectId,
        snapshot: SnapshotId,
        cx: &mut Context<Self>,
    ) {
        self.run(
            project,
            "Undo",
            Overlay::default(),
            move |path, _| backend::undo_snapshot(path, snapshot),
            |store, operation, cx| store.snack(format!("Undone: {operation}"), cx),
            cx,
        );
    }

    pub fn fetch(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        self.run(
            project,
            "Fetch",
            Overlay::default(),
            |path, _| backend::fetch(path),
            |store, (), cx| store.snack("Fetched", cx),
            cx,
        );
    }

    /// Rebase the project's branches onto its target's latest commits.
    pub fn update_from_target(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        self.run(
            project,
            "Update",
            Overlay::default(),
            |path, _| backend::update_from_target(path),
            |store, (), cx| store.snack_undoable("Updated from target", cx),
            cx,
        );
    }

    pub fn push(&mut self, project: &ProjectId, ref_name: String, cx: &mut Context<Self>) {
        let force = self
            .workspace(project)
            .and_then(|ws| ws.find_branch(&ref_name))
            .is_some_and(|branch| branch.push == PushState::NeedsForce);
        let name = short_ref(&ref_name).to_owned();
        let pushed = project.clone();
        self.run(
            project,
            format!("Push {name}"),
            Overlay::default(),
            move |path, _| backend::push(path, &ref_name, force),
            move |store, (), cx| {
                store.snack(format!("Pushed {name}"), cx);
                // A push can open or move a pull request's checks.
                store.refresh_forge(&pushed, cx);
            },
            cx,
        );
    }

    pub fn uncommit(&mut self, project: &ProjectId, id: CommitId, cx: &mut Context<Self>) {
        self.run(
            project,
            "Uncommit",
            Overlay::commits(self.key_of(project, id)),
            move |path, remap| backend::uncommit(path, &[remap.get(id)?]),
            |store, (), cx| store.snack_undoable("Uncommitted", cx),
            cx,
        );
    }

    pub fn dismiss_toast(&mut self, id: usize, cx: &mut Context<Self>) {
        self.toasts.retain(|toast| toast.id != id);
        cx.notify();
    }

    /// Run a mutation on `project` in the background, then reload it, busy until the
    /// reload lands. `done` runs on success with the operation's result.
    /// Run `op` once the mutations before it have finished, hiding what `overlay` says it
    /// takes away until then. It gets the commits it captured as they are by the time it runs.
    pub fn run<T: Send + 'static>(
        &mut self,
        project: &ProjectId,
        label: impl Into<SharedString>,
        overlay: Overlay,
        op: impl FnOnce(&Path, &Remap) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let label = label.into();
        let captured = self.workspace(project).map_or_else(HashMap::new, |ws| {
            ws.branches()
                .flat_map(|branch| &branch.commits)
                .map(|commit| (commit.id, commit.key.clone()))
                .collect()
        });
        let id = project.clone();
        let start_label = label.clone();
        self.queue.push_back(Queued {
            project: project.clone(),
            label,
            overlay,
            captured,
            start: Box::new(move |store, remap, cx| {
                store.start(&id, start_label, op, done, remap, cx)
            }),
        });
        self.cut = None;
        cx.emit(StoreEvent::Pending(project.clone()));
        cx.notify();
        if self.busy.is_none() {
            self.start_next(cx);
        }
    }

    fn start_next(&mut self, cx: &mut Context<Self>) {
        let Some(next) = self.queue.pop_front() else {
            return;
        };
        let remap = Remap::new(&next.captured, self.workspace(&next.project));
        self.busy = Some(next.label);
        self.running = Some((next.project.clone(), next.overlay, Instant::now()));
        (next.start)(self, remap, cx);
        self.tick_while_busy(cx);
    }

    /// Redraw while a mutation runs, so the status bar can say how long it's taking.
    fn tick_while_busy(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let Ok(true) = this.update(cx, |store, cx| {
                    let busy = store.running.is_some();
                    if store.busy_for().is_some_and(|t| t >= SLOW) {
                        cx.notify();
                    }
                    busy
                }) else {
                    break;
                };
            }
        })
        .detach();
    }

    fn start<T: Send + 'static>(
        &mut self,
        project: &ProjectId,
        label: SharedString,
        op: impl FnOnce(&Path, &Remap) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
        remap: Remap,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.path(project) else {
            self.busy = None;
            self.running = None;
            cx.emit(StoreEvent::Pending(project.clone()));
            return self.start_next(cx);
        };
        self.mutating = Some(project.clone());
        let before: HashSet<String> = self.workspace(project).map_or_else(HashSet::new, |ws| {
            ws.branches()
                .flat_map(|branch| &branch.commits)
                .map(|commit| commit.key.clone())
                .collect()
        });
        cx.notify();
        let project = project.clone();
        cx.spawn(async move |this, cx| {
            // Which oplog entry the operation recorded, if it did, for its Undo button.
            let result = backend::run(move || {
                let before = backend::oplog_head(&path);
                let value = op(&path, &remap)?;
                let recorded = backend::oplog_head(&path).filter(|after| Some(*after) != before);
                Ok((value, recorded))
            })
            .await;
            this.update(cx, |store, cx| {
                store.mutating = None;
                store.settling = Some(project.clone());
                // Set here rather than at the start, so a load the watcher started
                // mid-operation can't use it up.
                store.rebaseline.insert(project.clone(), before);
                match result {
                    Ok((value, recorded)) => {
                        store.last_snapshot = recorded.map(|snapshot| (project.clone(), snapshot));
                        done(store, value, cx)
                    }
                    Err(err) => store.error(format!("{label} failed"), format!("{err:#}"), cx),
                }
                store.reload(&project, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The key of `project`'s commit `id`, which survives rewrites.
    pub fn key_of(&self, project: &ProjectId, id: CommitId) -> Option<String> {
        let (_, commit) = self.workspace(project)?.find_commit(id)?;
        Some(commit.key.clone())
    }

    /// How long the running mutation has taken so far.
    pub fn busy_for(&self) -> Option<Duration> {
        self.running
            .as_ref()
            .map(|(_, _, started)| started.elapsed())
    }

    /// Mutations waiting for the running one.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// The overlays of the mutations running and waiting on `project`.
    fn overlays<'a>(&'a self, project: &'a ProjectId) -> impl Iterator<Item = &'a Overlay> {
        let running = self.running.iter().filter(move |(p, ..)| p == project);
        let queued = self.queue.iter().filter(move |q| q.project == *project);
        running
            .map(|(_, overlay, _)| overlay)
            .chain(queued.map(|q| &q.overlay))
    }

    /// Whether a pending mutation takes the commit with `key` away.
    pub fn hides_commit(&self, project: &ProjectId, key: &str) -> bool {
        self.overlays(project).any(|o| o.commits.contains(key))
    }

    /// Whether a pending mutation takes the uncommitted file at `path` away.
    pub fn hides_file(&self, project: &ProjectId, path: &str) -> bool {
        self.overlays(project).any(|o| o.files.contains(path))
    }

    /// The titles of commits pending mutations will make on top of `ref_name`.
    pub fn new_commits(&self, project: &ProjectId, ref_name: &str) -> Vec<SharedString> {
        self.overlays(project)
            .filter_map(|o| o.new_commit.as_ref())
            .filter(|(branch, _)| branch == ref_name)
            .map(|(_, title)| title.clone())
            .collect()
    }

    /// Keep the selection on commits an operation rewrote.
    pub fn follow(&mut self, rewritten: &backend::Rewritten, cx: &mut Context<Self>) {
        if let Some(Selection::Commit(project, id)) = &self.selection
            && let Some(new) = rewritten.get(id)
        {
            let selection = Selection::Commit(project.clone(), *new);
            self.select(Some(selection), cx);
        }
    }

    pub fn set_cut(&mut self, cut: Option<Cut>, cx: &mut Context<Self>) {
        self.cut = cut;
        cx.notify();
    }

    /// Write the saved state shortly, so a burst of changes writes once. A steady stream
    /// of changes still writes every half second.
    fn save_soon(&mut self, cx: &mut Context<Self>) {
        if self.save_queued {
            return;
        }
        self.save_queued = true;
        self.save_task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let Ok(Some(bytes)) = this.update(cx, |store, _| {
                store.save_queued = false;
                store.changed_state()
            }) else {
                return;
            };
            cx.background_spawn(async move { Saved::write(&bytes) })
                .await;
        });
    }

    /// Write the saved state now, as on quit, even if a background save of the same state
    /// hasn't finished: the process may end before it does.
    fn save(&mut self) {
        if let Ok(bytes) = serde_json::to_vec_pretty(&self.saved()) {
            Saved::write(&bytes);
            self.last_saved = bytes;
        }
    }

    /// The state to save, unless it's what was saved last.
    fn changed_state(&mut self) -> Option<Vec<u8>> {
        let bytes = serde_json::to_vec_pretty(&self.saved()).ok()?;
        if bytes == self.last_saved {
            return None;
        }
        self.last_saved = bytes.clone();
        Some(bytes)
    }

    fn saved(&self) -> Saved {
        Saved {
            expanded: self
                .projects
                .iter()
                .filter(|entry| entry.expanded)
                .map(|entry| entry.id.to_string())
                .collect(),
            selection: self.selection.as_ref().map(|s| s.project().to_string()),
            seen: self.seen.clone(),
            widths: self.widths.clone(),
            file_tree: self.file_tree,
        }
    }

    pub fn set_file_tree(&mut self, tree: bool, cx: &mut Context<Self>) {
        self.file_tree = tree;
        self.save_soon(cx);
        cx.notify();
    }

    pub fn width(&self, pane: Pane) -> f32 {
        self.widths
            .get(&pane)
            .copied()
            .unwrap_or(pane.default_width())
    }

    /// Resize `pane`, within its limits, or put it back to its default with `None`.
    pub fn set_width(&mut self, pane: Pane, width: Option<f32>, cx: &mut Context<Self>) {
        let (min, max) = pane.limits();
        match width {
            Some(width) => self.widths.insert(pane, width.clamp(min, max)),
            None => self.widths.remove(&pane),
        };
        self.save_soon(cx);
        cx.notify();
    }
}

/// A pane whose width can be dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Navigator,
    /// The file list beside a diff.
    Files,
}

impl Pane {
    fn default_width(self) -> f32 {
        match self {
            Pane::Navigator => 340.,
            Pane::Files => 300.,
        }
    }

    fn limits(self) -> (f32, f32) {
        match self {
            Pane::Navigator => (220., 640.),
            Pane::Files => (200., 560.),
        }
    }
}

/// The part of the saved state that's only read at startup.
#[derive(Default)]
struct Restored {
    expanded: HashSet<String>,
    selection: Option<String>,
}

/// The little the app remembers between launches.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    expanded: HashSet<String>,
    selection: Option<String>,
    #[serde(default)]
    seen: Seen,
    #[serde(default)]
    widths: HashMap<Pane, f32>,
    #[serde(default)]
    file_tree: bool,
}

/// What the user has looked at, so new work by agents stands out. Per project id.
#[derive(Default, Clone, Serialize, Deserialize)]
struct Seen {
    /// Commit keys (see [`Commit::key`]) that have been looked at.
    commits: HashMap<String, HashSet<String>>,
    /// Uncommitted files marked reviewed, with the file version reviewed.
    files: HashMap<String, HashMap<String, i64>>,
}

impl Saved {
    fn path() -> Option<PathBuf> {
        Some(but_path::app_config_dir().ok()?.join("gpui-state.json"))
    }

    fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|err| {
            // Keep what couldn't be read rather than write over it.
            tracing::warn!("couldn't read {}: {err}", path.display());
            std::fs::rename(&path, path.with_extension("json.bak")).ok();
            Self::default()
        })
    }

    /// Through a temporary file, so a crash mid-write leaves the last state rather than
    /// half a file, which would read back as nothing at all.
    fn write(bytes: &[u8]) {
        // One writer at a time: a background save can still be going when quit saves.
        static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _writing = WRITING.lock();
        let Some(path) = Self::path() else {
            return;
        };
        let temporary = path.with_extension("json.tmp");
        if std::fs::write(&temporary, bytes).is_ok() {
            std::fs::rename(&temporary, &path).ok();
        }
    }
}
