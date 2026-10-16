//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's been cut, and the operation in flight.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{AsyncApp, Context, EventEmitter, SharedString, Subscription, Task, WeakEntity};
use serde::{Deserialize, Serialize};

use crate::backend::watch::Watcher;
use crate::backend::{
    self, Branch, Commit, CommitId, FileChange, Project, Selected, SnapshotId, Workspace,
};
use crate::transfer::short_ref;

mod mutation;
mod saved;

use mutation::Queued;
pub use mutation::{Overlay, Remap, SLOW};
pub use saved::{Pane, Prefs, saved_window_place};
use saved::{Restored, Saved};

pub type ProjectId = SharedString;

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Project(ProjectId),
    Uncommitted(ProjectId),
    Branch(ProjectId, String),
    Commit(ProjectId, CommitId),
    /// The branches outside the workspace, to apply or delete.
    Branches(ProjectId),
    /// The operation log, to look back at and restore.
    History(ProjectId),
}

impl Selection {
    pub fn project(&self) -> &ProjectId {
        match self {
            Selection::Project(project)
            | Selection::Uncommitted(project)
            | Selection::Branch(project, _)
            | Selection::Commit(project, _)
            | Selection::Branches(project)
            | Selection::History(project) => project,
        }
    }
}

/// What ⌘X picked up, waiting for ⌘V on a destination.
#[derive(Debug, Clone)]
pub enum Cut {
    Commit(ProjectId, CommitId),
    /// A commit to put a copy of somewhere else: a cherry-pick.
    Pick(ProjectId, CommitId),
    /// A branch to stack on another, with the branches above it.
    Branch(ProjectId, String),
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
            Cut::Commit(project, _)
            | Cut::Pick(project, _)
            | Cut::Branch(project, _)
            | Cut::Files { project, .. } => project,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Cut::Commit(_, id) | Cut::Pick(_, id) => format!("commit {}", id.short()),
            Cut::Branch(_, ref_name) => short_ref(ref_name).to_owned(),
            Cut::Files { files, .. } if files.len() == 1 => files[0].file.path.clone(),
            Cut::Files { files, .. } => format!("{} files", files.len()),
        }
    }
}

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
    pub selection: Option<Selection>,
    pub cut: Option<Cut>,
    /// What the pointer is dragging. Only meaningful while GPUI has a drag active.
    pub dragging: Option<Cut>,
    /// Pane widths the user dragged away from their defaults.
    widths: HashMap<Pane, f32>,
    /// Where the window is, saved for the next launch.
    window: Option<saved::WindowPlace>,
    /// What was open when the app last quit, until the first project listing uses it.
    restored: Restored,
    /// The saved state as last written, so unchanged state isn't written again.
    last_saved: Vec<u8>,
    /// File lists show a tree of directories rather than a flat list.
    pub file_tree: bool,
    pub prefs: Prefs,
    pub toasts: Vec<Toast>,
    /// The label of the mutation in flight, if any. Mutations run one at a time, each
    /// until the reload after it lands, so the next one sees what it did.
    pub busy: Option<SharedString>,
    /// The running mutation's project, overlay and start.
    running: Option<(ProjectId, Overlay, Instant)>,
    busy_ticker: Task<()>,
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
    /// When each project was last fetched in the background.
    fetched: HashMap<ProjectId, Instant>,
    _auto_fetch: Task<()>,
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
            selection: None,
            cut: None,
            dragging: None,
            toasts: Vec::new(),
            busy: None,
            settling: None,
            mutating: None,
            running: None,
            busy_ticker: Task::ready(()),
            queue: VecDeque::new(),
            last_snapshot: None,
            next_toast: 0,
            seen: saved.seen,
            widths: saved.widths,
            file_tree: saved.file_tree,
            prefs: saved.prefs,
            window: saved.window,
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
            fetched: HashMap::new(),
            _auto_fetch: Self::fetch_in_background(cx),
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
                            .update(cx, |store, _| store.refresh_forge(&id))
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
            Selection::Project(_)
            | Selection::Uncommitted(_)
            | Selection::Branches(_)
            | Selection::History(_) => None,
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
        }
        cx.notify();
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

    /// Fetch the expanded projects as often as GitButler's shared setting says, so what the
    /// target and remotes have stays current. Quietly: a failed fetch only logs, and the
    /// next one tries again.
    fn fetch_in_background(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                let interval = backend::run(|| Ok(backend::settings::fetch_interval())).await;
                let Ok(Some(minutes)) = interval else {
                    continue;
                };
                let interval = Duration::from_secs(minutes * 60);
                let Ok(due) = this.update(cx, |store, _| store.due_for_fetch(interval)) else {
                    return;
                };
                for (project, path) in due {
                    if let Err(err) = backend::run(move || backend::fetch(&path)).await {
                        tracing::warn!(%project, "background fetch failed: {err:#}");
                    }
                    let reloaded = this.update(cx, |store, cx| {
                        store.fetched.insert(project.clone(), Instant::now());
                        store.reload(&project, cx);
                    });
                    if reloaded.is_err() {
                        return;
                    }
                }
            }
        })
    }

    fn due_for_fetch(&self, interval: Duration) -> Vec<(ProjectId, PathBuf)> {
        self.projects
            .iter()
            .filter(|entry| entry.expanded && entry.workspace.is_some())
            .filter(|entry| {
                self.fetched
                    .get(&entry.id)
                    .is_none_or(|at| at.elapsed() >= interval)
            })
            .map(|entry| (entry.id.clone(), entry.project.path.clone()))
            .collect()
    }

    /// Take `id` out of GitButler; its repository stays on disk.
    pub fn remove_project(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        if self.selection.as_ref().is_some_and(|s| s.project() == id) {
            self.select(None, cx);
        }
        let removed = id.to_string();
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                backend::settings::remove_project(&removed)?;
                backend::list_projects()
            })
            .await;
            this.update(cx, |store, cx| match result {
                Ok(projects) => {
                    store.set_projects(projects, cx);
                    store.snack("Project removed", cx);
                }
                Err(err) => store.error("Couldn't remove the project", format!("{err:#}"), cx),
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
                    store.refresh_forge(&id);
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
    pub fn refresh_forge(&mut self, id: &ProjectId) {
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
            Selection::Project(_)
            | Selection::Uncommitted(_)
            | Selection::Branches(_)
            | Selection::History(_) => true,
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
            .commits()
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
                    let mut commits = workspace.commits();
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

    pub fn dismiss_toast(&mut self, id: usize, cx: &mut Context<Self>) {
        self.toasts.retain(|toast| toast.id != id);
        cx.notify();
    }

    pub fn set_cut(&mut self, cut: Option<Cut>, cx: &mut Context<Self>) {
        self.cut = cut;
        cx.notify();
    }
}

/// What the user has looked at, so new work by agents stands out. Per project id.
#[derive(Default, Clone, Serialize, Deserialize)]
struct Seen {
    /// Commit keys (see [`Commit::key`]) that have been looked at.
    commits: HashMap<String, HashSet<String>>,
    /// Uncommitted files marked reviewed, with the file version reviewed.
    files: HashMap<String, HashMap<String, i64>>,
}
