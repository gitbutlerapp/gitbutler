//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's been cut, and the operation in flight.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use futures::{StreamExt as _, channel::mpsc};
use gpui::{Context, EventEmitter, SharedString, Subscription, Task};
use serde::{Deserialize, Serialize};

use crate::backend::watch::Watcher;
use crate::backend::{self, Commit, CommitId, FileChange, Project, Workspace};

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
    /// Files from the worktree (`from: None`) or from a commit.
    Files {
        project: ProjectId,
        from: Option<CommitId>,
        files: Vec<FileChange>,
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
            Cut::Files { files, .. } if files.len() == 1 => files[0].path.clone(),
            Cut::Files { files, .. } => format!("{} files", files.len()),
        }
    }
}

pub enum StoreEvent {
    /// A project's workspace was (re)loaded.
    Loaded(ProjectId),
    SelectionChanged,
}

/// Something to tell the user, shown the way GitButler's other apps do.
pub enum Notice {
    /// A one-line result at the bottom centre. `undo` names the project an Undo would
    /// restore, for operations that rewrote it.
    Snackbar {
        message: SharedString,
        undo: Option<ProjectId>,
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
    pub toasts: Vec<Toast>,
    /// The label of the mutation in flight, if any. Mutations run one at a time.
    pub busy: Option<SharedString>,
    /// The project the last mutation ran on, which an Undo offered for it restores.
    last_run: Option<ProjectId>,
    next_toast: usize,
    seen: Seen,
    /// The commits each project had before the app's last operation on it. Commits that
    /// appear after it are the app's own doing, so they load as already seen.
    rebaseline: HashMap<ProjectId, HashSet<String>>,
    save_task: Task<()>,
    save_queued: bool,
    _save_on_quit: Subscription,
    changes: mpsc::UnboundedSender<ProjectId>,
    _change_listener: Task<()>,
}

impl EventEmitter<StoreEvent> for Store {}

impl Store {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (changes, mut changed) = mpsc::unbounded::<ProjectId>();
        let listener = cx.spawn(async move |this, cx| {
            while let Some(id) = changed.next().await {
                // Watchers fire in bursts; let a burst settle before reloading.
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                let mut ids = HashSet::from([id]);
                while let Ok(id) = changed.try_recv() {
                    ids.insert(id);
                }
                let Ok(()) = this.update(cx, |store, cx| {
                    for id in ids {
                        store.reload(&id, cx);
                    }
                }) else {
                    break;
                };
            }
        });
        let saved = Saved::load();
        let mut store = Self {
            projects: Vec::new(),
            projects_loaded: false,
            selection: None,
            cut: None,
            dragging: None,
            toasts: Vec::new(),
            busy: None,
            last_run: None,
            next_toast: 0,
            seen: saved.seen,
            widths: saved.widths,
            rebaseline: HashMap::new(),
            save_task: Task::ready(()),
            save_queued: false,
            _save_on_quit: cx.on_app_quit(|store, _| {
                store.save();
                async {}
            }),
            changes,
            _change_listener: listener,
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
        let saved = Saved::load();
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
                    expanded: saved.expanded.contains(id.as_ref()),
                    id,
                    project,
                    workspace: None,
                    error: None,
                    loading: false,
                    reload_queued: false,
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
            let open = saved
                .selection
                .and_then(|id| self.project(&id).filter(|entry| entry.expanded))
                .or_else(|| self.projects.iter().find(|entry| entry.expanded))
                .map(|entry| Selection::Uncommitted(entry.id.clone()));
            if let Some(selection) = open {
                self.select(Some(selection), cx);
            }
            self.load_summaries(cx);
        }
        cx.notify();
    }

    /// Load collapsed projects one after another, so their cards can show counts.
    fn load_summaries(&mut self, cx: &mut Context<Self>) {
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
                backend::watch::watch(&project, move || {
                    changes.unbounded_send(watch_id.clone()).ok();
                })
            })
            .await;
            this.update(cx, |store, cx| {
                let Some(entry) = store.project_mut(&id) else {
                    return;
                };
                entry.loading = false;
                entry.reload_queued = false;
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
        let Some(entry) = self.project_mut(id) else {
            return;
        };
        if entry.loading {
            entry.reload_queued = true;
            return;
        }
        entry.loading = true;
        let path = entry.project.path.clone();
        let id = id.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || backend::load_workspace(&path)).await;
            this.update(cx, |store, cx| {
                let Some(entry) = store.project_mut(&id) else {
                    return;
                };
                entry.loading = false;
                let again = std::mem::take(&mut entry.reload_queued);
                match result {
                    Ok(workspace) => store.workspace_loaded(&id, workspace, !again, cx),
                    Err(err) => entry.error = Some(format!("{err:#}").into()),
                }
                // A queued reload has newer data; checking against this one could drop a
                // selection a mutation just made.
                if !again {
                    store.validate_selection(cx);
                }
                cx.emit(StoreEvent::Loaded(id.clone()));
                cx.notify();
                if again {
                    store.reload(&id, cx);
                }
            })
            .ok();
        })
        .detach();
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
        let undo = self.last_run.clone();
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
            backend::undo,
            |store, undone, cx| match undone {
                Some(operation) => store.snack(format!("Undone: {operation}"), cx),
                None => store.snack("Nothing to undo", cx),
            },
            cx,
        );
    }

    pub fn dismiss_toast(&mut self, id: usize, cx: &mut Context<Self>) {
        self.toasts.retain(|toast| toast.id != id);
        cx.notify();
    }

    /// Run a mutation on `project` in the background, then reload it. `done` runs on
    /// success with the operation's result.
    pub fn run<T: Send + 'static>(
        &mut self,
        project: &ProjectId,
        label: impl Into<SharedString>,
        op: impl FnOnce(&Path) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let label = label.into();
        if let Some(busy) = &self.busy {
            self.snack(format!("Still busy: {busy}"), cx);
            return;
        }
        let Some(path) = self.path(project) else {
            return;
        };
        self.busy = Some(label.clone());
        self.last_run = Some(project.clone());
        self.cut = None;
        let before: HashSet<String> = self.workspace(project).map_or_else(HashSet::new, |ws| {
            ws.branches()
                .flat_map(|branch| &branch.commits)
                .map(|commit| commit.key.clone())
                .collect()
        });
        cx.notify();
        let project = project.clone();
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || op(&path)).await;
            this.update(cx, |store, cx| {
                store.busy = None;
                // Set here rather than at the start, so a load the watcher started
                // mid-operation can't use it up.
                store.rebaseline.insert(project.clone(), before);
                match result {
                    Ok(value) => done(store, value, cx),
                    Err(err) => store.error(format!("{label} failed"), format!("{err:#}"), cx),
                }
                store.reload(&project, cx);
            })
            .ok();
        })
        .detach();
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
            this.update(cx, |store, _| {
                store.save_queued = false;
                store.save();
            })
            .ok();
        });
    }

    fn save(&self) {
        let saved = Saved {
            expanded: self
                .projects
                .iter()
                .filter(|entry| entry.expanded)
                .map(|entry| entry.id.to_string())
                .collect(),
            selection: self.selection.as_ref().map(|s| s.project().to_string()),
            seen: self.seen.clone(),
            widths: self.widths.clone(),
        };
        saved.store();
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

/// The little the app remembers between launches.
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    expanded: HashSet<String>,
    selection: Option<String>,
    #[serde(default)]
    seen: Seen,
    #[serde(default)]
    widths: HashMap<Pane, f32>,
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
        Self::path()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn store(&self) {
        if let (Some(path), Ok(bytes)) = (Self::path(), serde_json::to_vec_pretty(self)) {
            std::fs::write(path, bytes).ok();
        }
    }
}
