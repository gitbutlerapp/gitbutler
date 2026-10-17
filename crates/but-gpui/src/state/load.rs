//! Listing projects, and opening, watching and loading their workspaces.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{Context, SharedString, Task};

use super::{Cut, ProjectEntry, ProjectId, Selection, Store, StoreEvent};
use crate::backend::watch::Watcher;
use crate::backend::{self, CommitId, Project, Workspace};

/// A project whose files changed, whether that included its git directory, and the linked
/// worktree they're in, by name, unless they're the main checkout's.
pub(super) type Change = (ProjectId, bool, Option<Arc<str>>);

/// What a reload reads again: everything, or just the uncommitted files of the checkouts
/// that changed.
#[derive(Debug, Default)]
pub(super) struct Reload {
    /// The commits and branches too, as after a change in a git directory.
    full: bool,
    /// The main checkout's files.
    main: bool,
    /// These linked worktrees' files.
    linked: HashSet<Arc<str>>,
}

impl Reload {
    pub(super) fn full() -> Self {
        Self {
            full: true,
            ..Self::default()
        }
    }

    /// The main checkout's files, after an operation that changed only them.
    pub(super) fn files() -> Self {
        Self {
            main: true,
            ..Self::default()
        }
    }

    /// Add what a watcher saw: a change in a git directory, or files of a linked worktree
    /// (by name) or of the main checkout.
    fn add(&mut self, git: bool, linked: Option<Arc<str>>) {
        match linked {
            _ if git => self.full = true,
            Some(name) => {
                self.linked.insert(name);
            }
            None => self.main = true,
        }
    }

    pub(super) fn merge(&mut self, other: Reload) {
        self.full |= other.full;
        self.main |= other.main;
        self.linked.extend(other.linked);
    }
}

impl Store {
    /// Load the projects whose watchers saw changes, a burst at a time.
    pub(super) fn reload_on_change(
        mut changed: mpsc::UnboundedReceiver<Change>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            // While an agent keeps writing, reload at most this often: each reload runs a
            // full status, and back to back they kept a core busy.
            const MIN_INTERVAL: Duration = Duration::from_millis(500);
            let mut last: Option<Instant> = None;
            while let Some((id, git, linked)) = changed.next().await {
                // Watchers fire in bursts; let a burst settle before reloading.
                let throttle = last.map_or(Duration::ZERO, |at| {
                    MIN_INTERVAL.saturating_sub(at.elapsed())
                });
                cx.background_executor()
                    .timer(throttle.max(Duration::from_millis(60)))
                    .await;
                last = Some(Instant::now());
                let mut reloads: HashMap<ProjectId, Reload> = HashMap::new();
                reloads.entry(id).or_default().add(git, linked);
                while let Ok((id, git, linked)) = changed.try_recv() {
                    reloads.entry(id).or_default().add(git, linked);
                }
                let Ok(()) = this.update(cx, |store, cx| {
                    for (id, reload) in reloads {
                        store.load_or_defer(&id, reload, cx);
                    }
                }) else {
                    break;
                };
            }
        })
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
        let mut old: Vec<ProjectEntry> = std::mem::take(&mut self.projects);
        // Projects to open if they're expanded: the new ones, and the ones whose folder
        // moved.
        let mut open = Vec::new();
        self.projects = projects
            .into_iter()
            .map(|project| {
                let id: ProjectId = project.id.clone().into();
                if let Some(ix) = old.iter().position(|entry| entry.id == id) {
                    let mut entry = old.swap_remove(ix);
                    if entry.project.path != project.path {
                        entry.error = None;
                        entry.watcher = None;
                        entry.workspace = None;
                        open.push(id);
                    }
                    entry.project = project;
                    return entry;
                }
                open.push(id.clone());
                ProjectEntry {
                    expanded: restored.expanded.contains(id.as_ref()),
                    id,
                    project,
                    workspace: None,
                    error: None,
                    loading: false,
                    queued: None,
                    watcher: None,
                }
            })
            .collect();
        // A single project is always open, and one left closed, as after removing the
        // others, has nothing loaded or watched yet.
        if let [only] = self.projects.as_mut_slice()
            && !only.expanded
        {
            only.expanded = true;
            if !open.contains(&only.id) {
                open.push(only.id.clone());
            }
        }
        let first_load = !self.projects_loaded;
        self.projects_loaded = true;

        for id in &open {
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
                        store.select(Some(Selection::Uncommitted(id.clone())), cx);
                        store.added = Some(id);
                    }
                }
                Err(err) => store.error("Couldn't add the project", format!("{err:#}"), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Take `id` out of GitButler; its repository stays on disk.
    pub fn remove_project(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        if self.selection.as_ref().is_some_and(|s| s.project() == id) {
            self.select(None, cx);
        }
        let removed = id.to_string();
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                backend::remove_project(&removed)?;
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

    /// Watch and load every expanded project again, after a setting changed what they
    /// watch and list.
    pub fn reopen_expanded(&mut self, cx: &mut Context<Self>) {
        for id in self.expanded_ids() {
            self.rewatch(&id, cx);
            self.reload(&id, cx);
        }
    }

    /// Start watching `id` over, as when the worktrees it should watch changed.
    fn rewatch(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        let Some(entry) = self.project(id) else {
            return;
        };
        let project = entry.project.clone();
        let changes = self.changes.clone();
        let id = id.clone();
        cx.spawn(async move |this, cx| {
            let watch_id = id.clone();
            let watcher = backend::run(move || watch(&project, watch_id, changes)).await;
            this.update(cx, |store, _| match (store.project_mut(&id), watcher) {
                (Some(entry), Ok(watcher)) if entry.expanded => entry.watcher = Some(watcher),
                (_, Err(err)) => tracing::warn!("couldn't watch {id} again: {err:#}"),
                _ => {}
            })
            .ok();
        })
        .detach();
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
                watch(&project, watch_id, changes)
            })
            .await;
            this.update(cx, |store, cx| {
                let Some(entry) = store.project_mut(&id) else {
                    return;
                };
                entry.loading = false;
                // The full reload below reads whatever was asked for meanwhile.
                entry.queued = None;
                match watcher {
                    Ok(watcher) => {
                        if entry.expanded {
                            entry.watcher = Some(watcher);
                        }
                        store.refresh_forge(&id);
                    }
                    Err(err) => {
                        tracing::warn!("Couldn't open {}: {err:#}", entry.project.name);
                        entry.error = Some(load_error(&entry.project, &err));
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
        self.load(id, Reload::full(), cx);
    }

    pub fn reload_all(&mut self, cx: &mut Context<Self>) {
        for id in self.expanded_ids() {
            self.reload(&id, cx);
        }
    }

    /// Load `id` again: all of it, or unless `reload` is full, just the uncommitted files it
    /// names on top of the commits already shown, which skips walking the commit graph.
    pub(super) fn load(&mut self, id: &ProjectId, reload: Reload, cx: &mut Context<Self>) {
        let Some(entry) = self.project_mut(id) else {
            self.settled(id, cx);
            return;
        };
        if entry.loading {
            entry.queued.get_or_insert_default().merge(reload);
            return;
        }
        entry.loading = true;
        // Only a project with nothing loaded yet shows that it's loading, so other loads start
        // without a redraw.
        if entry.workspace.is_none() {
            cx.notify();
        }
        // Loads of one project don't overlap, so these commits are still current.
        let shown = (!reload.full).then(|| entry.workspace.clone()).flatten();
        let full = shown.is_none();
        let path = entry.project.path.clone();
        let id = id.clone();
        // Started after the mutation finished, so it sees what the mutation did.
        let settles = self.settles(&id);
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || match shown {
                Some(shown) => {
                    backend::reload_uncommitted(&path, shown, reload.main, &reload.linked)
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
                let queued = entry.queued.take();
                match result {
                    Ok(workspace) => {
                        if full && workspace.worktrees_listed {
                            store.forget_gone_worktrees(&id, &workspace);
                        }
                        store.workspace_loaded(&id, workspace, queued.is_none(), cx)
                    }
                    Err(err) => entry.error = Some(load_error(&entry.project, &err)),
                }
                // A queued reload has newer data; checking against this one could drop a
                // selection a mutation just made.
                if queued.is_none() {
                    store.validate_selection(&id, cx);
                }
                // A project just added opens on its page while it has no target: that's
                // where one is picked.
                let untargeted = store.workspace(&id).is_some_and(|ws| ws.target.is_none());
                if store.added.take_if(|added| *added == id).is_some()
                    && untargeted
                    && store.selection == Some(Selection::Uncommitted(id.clone()))
                {
                    store.select(Some(Selection::Project(id.clone())), cx);
                }
                if settles {
                    store.settled(&id, cx);
                }
                cx.emit(StoreEvent::Loaded(id.clone()));
                cx.notify();
                if let Some(queued) = queued {
                    store.load(&id, queued, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `latest` is false when a newer load is already queued behind this one.
    fn workspace_loaded(
        &mut self,
        id: &ProjectId,
        workspace: Workspace,
        latest: bool,
        cx: &mut Context<Self>,
    ) {
        // While a commit is being edited there's nothing else to look at or act on, and
        // what was seen and reviewed waits for the workspace to come back.
        if workspace.editing.is_some() {
            if let Some(entry) = self.project_mut(id) {
                entry.workspace = Some(workspace);
                entry.error = None;
            }
            let elsewhere = (self.selection.as_ref())
                .is_some_and(|s| s.project() == id && !matches!(s, Selection::Project(_)));
            if elsewhere {
                self.select(Some(Selection::Project(id.clone())), cx);
            }
            return;
        }
        self.update_seen(id, &workspace, latest);
        // An agent amending or rebasing the selected commit gives it a new id but keeps its
        // change id; stay on it.
        let rewritten = match &self.selection {
            Some(Selection::Commit(project, commit)) if project == id => {
                follow_by_key(self.workspace(id), &workspace, *commit)
                    .filter(|new| new != commit)
                    .map(|new| (*commit, new))
            }
            _ => None,
        };
        // Picks follow rewritten commits the same way, and go when a commit is gone.
        if !self.picked.is_empty() && self.selection.as_ref().is_some_and(|s| s.project() == id) {
            let old = self.workspace(id);
            let picked: Vec<CommitId> = (self.picked.iter())
                .filter_map(|commit| follow_by_key(old, &workspace, *commit))
                .collect();
            self.picked = if picked.len() > 1 { picked } else { Vec::new() };
        }
        // A cut commit follows the same way, and the cut goes with the commit; a commit
        // picked from outside the workspace stays as it is.
        if let Some(mut cut) = self.cut.take_if(|cut| cut.project() == id) {
            let old = self.workspace(id);
            let follow = |commit| match old.is_some_and(|old| old.find_commit(commit).is_some()) {
                true => follow_by_key(old, &workspace, commit),
                false => Some(commit),
            };
            let kept = match &mut cut {
                Cut::Commit(_, commit)
                | Cut::CherryPick(_, commit)
                | Cut::Files {
                    from: Some(commit), ..
                } => follow(*commit).map(|new| *commit = new).is_some(),
                // Uncommitted files go when one of them is no longer uncommitted, and a
                // branch when it's gone, as when renamed.
                Cut::Files {
                    from: None, files, ..
                } => (files.iter())
                    .all(|f| (workspace.uncommitted.iter()).any(|u| u.path == f.file.path)),
                Cut::Branch(_, ref_name) => workspace.find_branch(ref_name).is_some(),
            };
            self.cut = kept.then_some(cut);
        }
        // The file watcher learns which worktrees to watch when it starts, so one added,
        // shown again or archived starts it over. Edit mode lists none, so going in and out
        // of it doesn't count.
        let unwatched = self.workspace(id).is_some_and(|old| {
            let names = |ws: &Workspace| -> HashSet<Arc<str>> {
                ws.worktrees.iter().map(|w| w.name.clone()).collect()
            };
            old.editing.is_none() && names(old) != names(&workspace)
        });
        if let Some(entry) = self.project_mut(id) {
            entry.workspace = Some(workspace);
            entry.error = None;
        }
        if unwatched {
            self.rewatch(id, cx);
        }
        if let Some((from, to)) = rewritten {
            let picked = std::mem::take(&mut self.picked);
            self.select_followed(id.clone(), from, to, cx);
            self.picked = picked;
        }
        self.save_soon(cx);
    }
}

/// Where `old`'s commit `commit` is in `new`, if it's still there: under the same id, or
/// under the new id that amending or rebasing it gave it, which keeps its key.
fn follow_by_key(old: Option<&Workspace>, new: &Workspace, commit: CommitId) -> Option<CommitId> {
    if new.find_commit(commit).is_some() {
        return Some(commit);
    }
    let (_, old) = old?.find_commit(commit)?;
    new.commits().find(|c| c.key == old.key).map(|c| c.id)
}

/// Why a project didn't load: plainly when its folder is gone, otherwise as reported.
fn load_error(project: &Project, err: &anyhow::Error) -> SharedString {
    if project.path.exists() {
        format!("{err:#}").into()
    } else {
        "The project's folder isn't there any more.".into()
    }
}

/// Watch `project` for changes, sending them as `id`'s to `changes`.
fn watch(
    project: &Project,
    id: ProjectId,
    changes: mpsc::UnboundedSender<Change>,
) -> anyhow::Result<Watcher> {
    backend::watch::watch(project, move |git, linked| {
        changes.unbounded_send((id.clone(), git, linked)).ok();
    })
}
