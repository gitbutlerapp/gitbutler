//! Listing projects, and opening, watching and loading their workspaces.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{Context, SharedString, Task};

use super::{Cut, ProjectEntry, ProjectId, Selection, Store, StoreEvent};
use crate::backend::watch::Watcher;
use crate::backend::{self, CommitId, FileChange, Project, Selected, Workspace};

/// A project whose files changed, whether that included its git directory, and the linked
/// worktree they're in, by name, unless they're the main checkout's.
pub(super) type Change = (ProjectId, bool, Option<Arc<str>>);

/// What a reload reads again: everything, the commits and branches, or just the uncommitted
/// files of the checkouts that changed.
#[derive(Debug, Default)]
pub(super) struct Reload {
    /// The commits and branches too, as after a change in a git directory.
    full: bool,
    /// The commits and branches, keeping the files shown but for the ones asked for below.
    commits: bool,
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

    /// The commits and branches, after an operation on them, and the files of the checkouts
    /// whose `HEAD` tree it changed.
    pub(super) fn commits() -> Self {
        Self {
            commits: true,
            ..Self::default()
        }
    }

    /// The uncommitted files of linked worktree `checkout`, or of the main checkout for
    /// `None`, after an operation that changed only them.
    pub(super) fn files(checkout: Option<Arc<str>>) -> Self {
        Self {
            main: checkout.is_none(),
            linked: checkout.into_iter().collect(),
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
        self.commits |= other.commits;
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
            // full status, which gix spreads over every core. Twice a second kept one busy in
            // a repository of mise's size; a change after a quiet spell still shows at once.
            const MIN_INTERVAL: Duration = Duration::from_millis(1000);
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
        let edits = self.project_edits;
        // Started now rather than once the task first runs, which at launch is after the
        // window's first frame.
        let projects = backend::run(backend::list_projects);
        cx.spawn(async move |this, cx| {
            let projects = projects.await;
            this.update(cx, |store, cx| match projects {
                Ok(_) if store.project_edits != edits => {}
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
        // The watched keep the order they were in, the first listing the saved one.
        let order: Vec<ProjectId> = (old.iter().filter(|entry| entry.expanded))
            .map(|entry| entry.id.clone())
            .chain(
                restored
                    .expanded
                    .iter()
                    .map(|id| ProjectId::from(id.as_str())),
            )
            .collect();
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
                    expanded: restored.expanded.iter().any(|saved| saved == id.as_ref()),
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
        self.order_projects(&order);
        let first_load = !self.projects_loaded;
        // A single project is always open, and one left closed, as after removing the
        // others, has nothing loaded or watched yet. Only when the list changed: reading it
        // again as the window comes forward leaves a project the user collapsed alone.
        let changed = first_load || !open.is_empty() || !old.is_empty();
        if let [only] = self.projects.as_mut_slice()
            && changed
            && !only.expanded
        {
            only.expanded = true;
            if !open.contains(&only.id) {
                open.push(only.id.clone());
            }
        }
        self.projects_loaded = true;
        // A selected project that went, as when removed, leaves the first open one selected.
        if (self.selection.as_ref()).is_some_and(|s| self.project(s.project()).is_none()) {
            let first = (self.projects.iter()).find(|entry| entry.expanded);
            self.select(
                first.map(|entry| Selection::Uncommitted(entry.id.clone())),
                cx,
            );
        }

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
        self.project_edits += 1;
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                let added = backend::add_project(&path)?;
                backend::list_projects().map(|projects| (projects, added))
            })
            .await;
            this.update(cx, |store, cx| match result {
                Ok((projects, (id, existed))) => {
                    store.project_edits += 1;
                    let id = ProjectId::from(id);
                    store.set_projects(projects, cx);
                    store.set_expanded(&id, true, cx);
                    store.select(Some(Selection::Uncommitted(id.clone())), cx);
                    match (existed, store.project(&id)) {
                        (true, Some(entry)) => {
                            let name = entry.project.name.clone();
                            store.snack(format!("{name} is already added"), cx)
                        }
                        _ => store.added = Some(id),
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
        let removed = id.to_string();
        self.project_edits += 1;
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                backend::remove_project(&removed)?;
                backend::list_projects()
            })
            .await;
            this.update(cx, |store, cx| match result {
                Ok(projects) => {
                    store.project_edits += 1;
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
        if !expanded {
            entry.watcher = None;
        }
        // Watched, it goes after the others watched; not, back among the rest by name.
        let mut order = self.expanded_ids();
        if expanded {
            order.retain(|other| other != id);
            order.push(id.clone());
        }
        self.order_projects(&order);
        if expanded {
            self.open_project(id, cx);
            self.read_mesh();
        } else if self
            .selection
            .as_ref()
            .is_some_and(|selection| selection.project() == id)
        {
            self.select(Some(Selection::Project(id.clone())), cx);
        }
        self.save_soon(cx);
        cx.notify();
    }

    /// Move watched project `id` before watched project `before`, or after the others.
    pub fn move_watched(
        &mut self,
        id: &ProjectId,
        before: Option<&ProjectId>,
        cx: &mut Context<Self>,
    ) {
        let mut order = self.expanded_ids();
        let Some(from) = order.iter().position(|other| other == id) else {
            return;
        };
        if before == Some(id) {
            return;
        }
        let id = order.remove(from);
        let at = before
            .and_then(|before| order.iter().position(|other| other == before))
            .unwrap_or(order.len());
        order.insert(at, id);
        self.order_projects(&order);
        self.save_soon(cx);
        cx.notify();
    }

    /// List the watched projects as `order` has them, and the others by name.
    pub(super) fn order_projects(&mut self, order: &[ProjectId]) {
        let at = |entry: &ProjectEntry| order.iter().position(|id| *id == entry.id);
        self.projects.sort_by(|a, b| {
            let name = |entry: &ProjectEntry| entry.project.name.to_lowercase();
            at(a).cmp(&at(b)).then_with(|| name(a).cmp(&name(b)))
        });
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
            this.update(cx, |store, cx| match (store.project_mut(&id), watcher) {
                (Some(entry), Ok(watcher)) if entry.expanded => {
                    entry.watcher = Some(watcher);
                    // What the old watcher saw last it hadn't passed on yet, and dropped.
                    store.load(&id, Reload::full(), cx);
                }
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
        // Hold off reloads until the project is ready, unless a load runs already, as one an
        // unwatched project's page started: that one holds them off, and lands as usual.
        let held = !std::mem::replace(&mut entry.loading, true);
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
                // The full reload below reads whatever was asked for meanwhile.
                if held {
                    entry.loading = false;
                    entry.queued = None;
                }
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
                // Behind a load still running, if one is.
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

    /// Load again the expanded projects whose folder went since they last loaded, and those
    /// that failed to load: a watcher doesn't see its own folder move, and what failed may
    /// have been fixed in a terminal. One that couldn't be opened, so isn't watched, opens.
    pub fn recheck_folders(&mut self, cx: &mut Context<Self>) {
        let stale: Vec<(ProjectId, bool)> = (self.projects.iter())
            .filter(|entry| entry.expanded && entry.project.path.exists() == entry.error.is_some())
            .map(|entry| (entry.id.clone(), entry.watcher.is_none()))
            .collect();
        for (id, unwatched) in stale {
            match unwatched {
                true => self.open_project(&id, cx),
                false => self.reload(&id, cx),
            }
        }
    }

    /// Load `id` again: all of it; or its commits and branches, keeping the files shown but
    /// for those `reload` names; or just those files, on top of the commits shown.
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
        // Loads of one project don't overlap, so what's shown is still current.
        let (shown, kept) = match entry.workspace.as_ref().filter(|_| !reload.full) {
            Some(shown) if reload.commits => (
                None,
                backend::Kept::from(shown, reload.main, &reload.linked),
            ),
            shown => (shown.cloned(), backend::Kept::default()),
        };
        let path = entry.project.path.clone();
        let id = id.clone();
        // Started after the mutation finished, so it sees what the mutation did.
        let settles = self.settles(&id);
        cx.spawn(async move |this, cx| {
            let result = backend::run(move || match shown {
                Some(shown) => {
                    backend::reload_uncommitted(&path, shown, reload.main, &reload.linked)
                }
                None => backend::load_workspace(&path, kept),
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
                    Ok(workspace) => store.workspace_loaded(&id, workspace, queued.is_none(), cx),
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
        // While a commit is being edited there's nothing else to look at or act on than its
        // files, and what was seen waits for the workspace to come back.
        if workspace.editing.is_some() {
            if let Some(entry) = self.project_mut(id) {
                entry.workspace = Some(workspace);
                entry.error = None;
            }
            let elsewhere = (self.selection.as_ref()).is_some_and(|s| {
                s.project() == id && !matches!(s, Selection::Project(_) | Selection::Uncommitted(_))
            });
            if elsewhere {
                self.select(Some(Selection::Uncommitted(id.clone())), cx);
            }
            return;
        }
        self.update_seen(id, &workspace, latest);
        // An agent amending or rebasing the selected commit gives it a new id but keeps its
        // change id, or with plain Git its author, time and message; stay on it.
        // One that's gone, as squashed away, leaves its branch selected, if that's still there.
        // A selected branch renamed, as an agent names one, is followed by its newest commit.
        let old = self.workspace(id);
        let renamed: Vec<(String, String)> = (old.iter().flat_map(|old| old.branches()))
            .filter(|branch| workspace.find_branch(&branch.ref_name).is_none())
            .filter_map(|branch| {
                let newest = follow_by_key(old, &workspace, branch.commits.first()?.id)?;
                let (now, _) = workspace.find_commit(newest)?;
                Some((branch.ref_name.clone(), now.ref_name.clone()))
            })
            .collect();
        let (rewritten, gone_from) = match &self.selection {
            Some(Selection::Commit(project, commit)) if project == id => {
                match follow_by_key(old, &workspace, *commit) {
                    Some(new) => ((new != *commit).then_some((*commit, new)), None),
                    None => {
                        let branch = old.and_then(|old| old.find_commit(*commit));
                        (None, branch.map(|(branch, _)| branch.ref_name.clone()))
                    }
                }
            }
            Some(Selection::Branch(project, ref_name)) if project == id => {
                let to = renamed.iter().find(|(from, _)| from == ref_name);
                (None, to.map(|(_, to)| to.clone()))
            }
            _ => (None, None),
        };
        let gone_from = gone_from
            .filter(|branch| !branch.is_empty() && workspace.find_branch(branch).is_some());
        // Picks follow rewritten commits the same way, and go when a commit is gone.
        if !self.picked.is_empty() && self.selection.as_ref().is_some_and(|s| s.project() == id) {
            let mut picks = std::mem::take(&mut self.picked);
            let old = self.workspace(id);
            picks.follow(|commit| follow_by_key(old, &workspace, *commit));
            self.picked = picks;
        }
        // A cut commit follows the same way, and the cut goes with the commit; a commit
        // picked from elsewhere, as the target's history, stays as it is.
        if let Some(mut cut) = self.cut.take_if(|cut| cut.project() == id) {
            let old = self.workspace(id);
            let follow = |commit| match old.is_some_and(|old| old.find_commit(commit).is_some()) {
                true => follow_by_key(old, &workspace, commit),
                false => Some(commit),
            };
            let kept = match &mut cut {
                Cut::Commit(_, commits) | Cut::CherryPick(_, commits) => {
                    let followed: Option<Arc<[CommitId]>> =
                        commits.iter().map(|commit| follow(*commit)).collect();
                    followed.map(|followed| *commits = followed).is_some()
                }
                // Some of a commit's file goes once the commit is rewritten, as the lines
                // picked may have moved with it.
                Cut::Files {
                    from: Some(commit),
                    files,
                    ..
                } => follow(*commit)
                    .filter(|new| new == commit || files.iter().all(|f| f.hunks.is_none()))
                    .map(|new| *commit = new)
                    .is_some(),
                // Uncommitted files go when one of them is no longer uncommitted, and some of
                // a file once it changed, as the lines picked may have moved under the pick;
                // a branch goes when it's gone, as when renamed.
                Cut::Files {
                    from: None,
                    worktree,
                    files,
                    ..
                } => {
                    let uncommitted = workspace.uncommitted_in(worktree.as_deref());
                    let same = |f: &Selected, u: &FileChange| {
                        f.hunks.is_none()
                            || (u.modified, u.lines) == (f.file.modified, f.file.lines)
                    };
                    uncommitted.is_some_and(|uncommitted| {
                        (files.iter()).all(|f| {
                            uncommitted
                                .iter()
                                .any(|u| u.path == f.file.path && same(f, u))
                        })
                    })
                }
                Cut::Branch(_, ref_name) => workspace.find_branch(ref_name).is_some(),
            };
            // Let go of one that changed out loud, rather than ⌘V doing nothing.
            self.cut = Some(cut);
            if !kept {
                self.set_cut(None, cx);
                self.snack("What was cut changed, so it was let go", cx);
            }
        }
        // The commit a drag is held over follows too, so letting go there without moving
        // still drops on it.
        if let Some((Selection::Commit(project, commit), place)) = self.drop_at.clone()
            && project == *id
        {
            self.drop_at = follow_by_key(self.workspace(id), &workspace, commit)
                .map(|new| (Selection::Commit(project, new), place));
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
        if let Some(branch) = gone_from {
            self.select(Some(Selection::Branch(id.clone(), branch)), cx);
        }
        for (from, to) in renamed {
            cx.emit(StoreEvent::Renamed(id.clone(), from, to));
        }
        self.save_soon(cx);
    }
}

/// Where `old`'s commit `commit` is in `new`, if it's still there: under the same id, or
/// under the new id that amending or rebasing it gave it, which keeps its key, or after Git
/// rewrote it without the key, what Git kept, as unread marks pair them.
fn follow_by_key(old: Option<&Workspace>, new: &Workspace, commit: CommitId) -> Option<CommitId> {
    if new.find_commit(commit).is_some() {
        return Some(commit);
    }
    let (_, old) = old?.find_commit(commit)?;
    (new.commits().find(|c| c.key == old.key))
        .or_else(|| new.commits().find(|c| c.authored == old.authored))
        .map(|c| c.id)
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
