//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's been cut, and the operation in flight.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use futures::{StreamExt as _, channel::mpsc};
use gpui::{Context, EventEmitter, SharedString, Task};
use serde::{Deserialize, Serialize};

use crate::backend::watch::Watcher;
use crate::backend::{self, CommitId, FileChange, Project, Workspace};

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
            Cut::Commit(_, id) => format!("Commit {}", id.short()),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Error,
}

pub struct Toast {
    pub id: usize,
    pub kind: ToastKind,
    pub message: SharedString,
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
    pub toasts: Vec<Toast>,
    /// The label of the mutation in flight, if any. Mutations run one at a time.
    pub busy: Option<SharedString>,
    next_toast: usize,
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
        let mut store = Self {
            projects: Vec::new(),
            projects_loaded: false,
            selection: None,
            cut: None,
            toasts: Vec::new(),
            busy: None,
            next_toast: 0,
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
                Err(err) => store.toast(ToastKind::Error, format!("{err:#}"), cx),
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
                            Ok(workspace) => entry.workspace = Some(workspace),
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
                Err(err) => store.toast(ToastKind::Error, format!("{err:#}"), cx),
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
        self.save();
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
                match result {
                    Ok(workspace) => {
                        entry.workspace = Some(workspace);
                        entry.error = None;
                    }
                    Err(err) => entry.error = Some(format!("{err:#}").into()),
                }
                let again = std::mem::take(&mut entry.reload_queued);
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

    pub fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        if self.selection == selection {
            return;
        }
        let project_changed = self.selection.as_ref().map(Selection::project)
            != selection.as_ref().map(Selection::project);
        self.selection = selection;
        if project_changed {
            self.save();
        }
        cx.emit(StoreEvent::SelectionChanged);
        cx.notify();
    }

    pub fn toast(
        &mut self,
        kind: ToastKind,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.push(Toast {
            id,
            kind,
            message: message.into(),
        });
        let linger = match kind {
            ToastKind::Info => Duration::from_secs(3),
            ToastKind::Error => Duration::from_secs(8),
        };
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
            self.toast(ToastKind::Info, done.to_owned(), cx);
        } else {
            let message = format!("{done}, but left out {}", left_out.join(", "));
            self.toast(ToastKind::Error, message, cx);
        }
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
            self.toast(ToastKind::Info, format!("Still busy: {busy}"), cx);
            return;
        }
        let Some(path) = self.path(project) else {
            return;
        };
        self.busy = Some(label.clone());
        self.cut = None;
        cx.notify();
        let project = project.clone();
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || op(&path)).await;
            this.update(cx, |store, cx| {
                store.busy = None;
                match result {
                    Ok(value) => done(store, value, cx),
                    Err(err) => {
                        store.toast(ToastKind::Error, format!("{label} failed: {err:#}"), cx)
                    }
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

    fn save(&self) {
        let saved = Saved {
            expanded: self
                .projects
                .iter()
                .filter(|entry| entry.expanded)
                .map(|entry| entry.id.to_string())
                .collect(),
            selection: self.selection.as_ref().map(|s| s.project().to_string()),
        };
        saved.store();
    }
}

/// The little the app remembers between launches.
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    expanded: HashSet<String>,
    selection: Option<String>,
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
