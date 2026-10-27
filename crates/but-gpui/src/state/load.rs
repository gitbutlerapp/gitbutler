//! Listing projects, and opening, watching and loading their workspaces.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{Context, SharedString, Task};

use super::{Cut, ProjectEntry, ProjectId, Selection, Store, StoreEvent};
use crate::backend::watch::Watcher;
use crate::backend::{
    self, Checkout, CommitId, FileChange, FullName, Project, Selected, Workspace,
};

/// A watcher's event: the project, whether its git directory changed, the checkout whose
/// files changed, and which of its files.
pub(super) type Change = (ProjectId, bool, Checkout, Vec<Arc<str>>);

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
    linked: HashSet<bstr::BString>,
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

    /// The uncommitted files of `checkout`, after an operation that changed only them.
    pub(super) fn files(checkout: Checkout) -> Self {
        let mut reload = Self::default();
        reload.add(false, checkout);
        reload
    }

    /// Add what a watcher saw: a change in a git directory, or files of a checkout.
    fn add(&mut self, git: bool, checkout: Checkout) {
        match checkout {
            _ if git => self.full = true,
            Checkout::Linked(name) => {
                self.linked.insert(name);
            }
            Checkout::Main => self.main = true,
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
            while let Some(first) = changed.next().await {
                // Watchers fire in bursts; let a burst settle before reloading.
                let throttle = last.map_or(Duration::ZERO, |at| {
                    MIN_INTERVAL.saturating_sub(at.elapsed())
                });
                cx.background_executor()
                    .timer(throttle.max(Duration::from_millis(60)))
                    .await;
                last = Some(Instant::now());
                let mut events = vec![first];
                while let Ok(event) = changed.try_recv() {
                    events.push(event);
                }
                let Ok(()) = this.update(cx, |store, cx| {
                    let mut reloads: HashMap<ProjectId, Reload> = HashMap::new();
                    let now = crate::ui::now();
                    for (id, git, checkout, paths) in events {
                        // What the app's own operations write isn't activity in the checkout,
                        // nor is what a repository cloned inside it writes to its own .git.
                        let own = store.running.contains_key(&id) || store.own_write(&id, now);
                        let saved = (paths.iter())
                            .filter(|path| !path.split('/').any(|part| part == ".git"))
                            .count();
                        if saved > 0 && !own {
                            store
                                .activity
                                .entry((id.clone(), checkout.clone()))
                                .or_default()
                                .record(now, saved as u32);
                        }
                        reloads.entry(id).or_default().add(git, checkout);
                    }
                    for (id, reload) in reloads {
                        // An operation under way reads these with its own reload, which the
                        // operations queued after it find their commits by, hidden or not.
                        let busy = (store.running.get(&id))
                            .is_some_and(|r| !r.settling && !r.overlay.remote_only);
                        match store.hidden && !busy {
                            true => store.unseen.entry(id).or_default().merge(reload),
                            false => store.load(&id, reload, cx),
                        }
                    }
                }) else {
                    break;
                };
            }
        })
    }

    /// Follow whether any of the window shows. Hidden, watchers' changes and the forge poll
    /// wait, so an agent writing while the app is behind an editor costs a file watcher and
    /// no more; shown again, each project changed meanwhile loads once.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        tracing::debug!(visible, unseen = self.unseen.len(), "window visibility");
        let shown = self.hidden && visible;
        self.hidden = !visible;
        if self.hidden {
            return;
        }
        for (id, reload) in std::mem::take(&mut self.unseen) {
            self.load(&id, reload, cx);
        }
        if self.forge_skipped > 0 {
            self.ask_forges(cx);
        }
        // The mesh isn't read while hidden.
        if shown {
            self.mesh_read_now();
        }
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
                Err(err) => store.failed("Couldn't list projects", &err, cx),
            })
            .ok();
        })
        .detach();
    }

    fn set_projects(&mut self, projects: Vec<Project>, cx: &mut Context<Self>) {
        // What was open last time applies to the first listing only; later ones add
        // projects the user just added, which start closed.
        // An empty one, as when the list couldn't be read and was set aside, keeps what was
        // watched for when the projects are back.
        let restored = match projects.is_empty() {
            true => self.restored.clone(),
            false => std::mem::take(&mut self.restored),
        };
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
                        entry.read();
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
                    folder_gone: false,
                    target_choice: None,
                    loading: false,
                    queued: None,
                    watcher: None,
                }
            })
            .collect();
        self.order_projects(&order);
        // The listing has every project, loaded or not, so one it doesn't have was removed.
        self.forget_unlisted();
        let first_load = !self.projects_loaded;
        // On the first launch, the project fetched last is watched, as the one GitButler was
        // last used in.
        if restored.fresh
            && let Some(entry) = (self.projects.iter_mut())
                .filter(|entry| entry.project.last_fetch.is_some())
                .max_by_key(|entry| entry.project.last_fetch)
        {
            entry.expanded = true;
        }
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
        if changed {
            self.save_soon(cx);
        }
        // A selected project that went, as when removed, leaves the first open one selected.
        if (self.selection.as_ref()).is_some_and(|s| self.project(s.project()).is_none()) {
            let first = (self.projects.iter()).find(|entry| entry.expanded);
            self.select(first.map(|entry| self.opening(&entry.id)), cx);
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
                .map(|entry| self.opening(&entry.id));
            if let Some(selection) = open {
                self.select(Some(selection), cx);
            }
        }
        cx.notify();
    }

    /// What selecting `project` as a whole picks: its Main worktree, or the project itself
    /// while its card is folded, so it stays folded.
    fn opening(&self, project: &ProjectId) -> Selection {
        match self.prefs.folded_projects.contains(project.as_ref()) {
            true => Selection::Project(project.clone()),
            false => Selection::Uncommitted(project.clone()),
        }
    }

    pub fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.project_edits += 1;
        cx.spawn(async move |this, cx| {
            let folder = path.clone();
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
                Err(err) if err.is::<backend::NotARepository>() => store.offer_init(folder, cx),
                Err(err) => store.failed("Couldn't add the project", &err, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Offer to make `folder`, which isn't a Git repository, one, then add it.
    fn offer_init(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        let name = (folder.file_name()).map_or_else(
            || folder.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        self.error_with_retry(
            format!("{name} isn't a Git repository yet"),
            "GitButler can make it one, as git init does, then add it.",
            "Initialize Git",
            move |_, cx| {
                let folder = folder.clone();
                cx.spawn(async move |this, cx| {
                    let path = folder.clone();
                    let made = backend::run(move || backend::init_repository(&path)).await;
                    this.update(cx, |store, cx| match made {
                        Ok(()) => store.add_project(folder, cx),
                        Err(err) => store.failed("Couldn't initialize Git", &err, cx),
                    })
                    .ok();
                })
                .detach();
            },
            cx,
        );
    }

    /// Take `id` out of GitButler; its repository stays on disk.
    pub fn remove_project(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        // Removing deletes GitButler's data in the repository, which an operation there would
        // fail partway through for, or write back.
        if self.pending(id, None, false) {
            return self.snack("Wait for its operations to finish, then remove it", cx);
        }
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
                Err(err) => store.failed("Couldn't remove the project", &err, cx),
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
            self.mesh_read_now();
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
        self.mesh_seed(id, cx);
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
            let path = project.path.clone();
            let watcher = backend::run(move || {
                backend::activate(&project.path)?;
                // Asked before the first commit fails for it, as the desktop app does.
                let author_missing = backend::settings::author_missing(&project.path)
                    .is_ok_and(|missing| missing.is_some());
                Ok((watch(&project, watch_id, changes)?, author_missing))
            })
            .await;
            let failure = backend::load_failure(path, watcher.as_ref().err()).await;
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
                    Ok((watcher, author_missing)) => {
                        if entry.expanded {
                            entry.watcher = Some(watcher);
                        }
                        store.refresh_forge(&id);
                        if author_missing && !std::mem::replace(&mut store.asked_author, true) {
                            cx.emit(StoreEvent::AuthorMissing(id.clone()));
                        }
                    }
                    Err(err) => {
                        tracing::warn!("Couldn't open {}: {err:#}", entry.project.name);
                        entry.failed(&err, failure);
                    }
                }
                // Behind a load still running, if one is.
                store.reload(&id, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Reload `id`'s workspace, coalescing requests that arrive while a load or an operation
    /// is running.
    pub fn reload(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        self.load(id, Reload::full(), cx);
    }

    /// Load `id` again, as after changing its target, opening it first when it's watched but
    /// couldn't be opened before, as with a target several local branches track: no watcher
    /// would see what happens in it from then on.
    pub fn reload_or_open(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        match (self.project(id)).is_some_and(|entry| entry.expanded && entry.watcher.is_none()) {
            true => self.open_project(id, cx),
            false => self.reload(id, cx),
        }
    }

    /// Load again the expanded projects and the selected one whose folder went since they last
    /// loaded, and those that failed to load: a watcher doesn't see its own folder move, and
    /// what failed may have been fixed in a terminal. One that couldn't be opened, so isn't
    /// watched, opens.
    pub fn recheck_folders(&mut self, cx: &mut Context<Self>) {
        let selected = self.selection.as_ref().map(Selection::project);
        let checked: Vec<(ProjectId, PathBuf)> = (self.projects.iter())
            .filter(|entry| entry.expanded || selected == Some(&entry.id))
            .map(|entry| (entry.id.clone(), entry.project.path.clone()))
            .collect();
        cx.spawn(async move |this, cx| {
            for (id, path) in checked {
                let gone = backend::folder_gone(path).await;
                this.update(cx, |store, cx| {
                    if store
                        .project(&id)
                        .is_some_and(|entry| gone == entry.error.is_none())
                    {
                        store.reload_or_open(&id, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// Load `id` again: all of it; or its commits and branches, keeping the files shown but
    /// for those `reload` names; or just those files, on top of the commits shown.
    pub(super) fn load(&mut self, id: &ProjectId, reload: Reload, cx: &mut Context<Self>) {
        // While an operation changes `id`, the reload after it reads this too, and loading
        // now would only queue in front of that one.
        if let Some(running) =
            (self.running.get_mut(id)).filter(|r| !r.settling && !r.overlay.remote_only)
        {
            running.deferred.merge(reload);
            return;
        }
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
            let folder = path.clone();
            let result = backend::run(move || match shown {
                Some(shown) => {
                    backend::reload_uncommitted(&path, shown, reload.main, &reload.linked)
                }
                None => backend::load_workspace(&path, kept),
            })
            .await;
            let failure = backend::load_failure(folder, result.as_ref().err()).await;
            this.update(cx, |store, cx| {
                // One that started before the project's operation finished, landing in front
                // of the reload after it, doesn't show what the operation did yet.
                let fresh = settles || !store.settles(&id);
                let Some(entry) = store.project_mut(&id) else {
                    store.settled(&id, cx);
                    return;
                };
                entry.loading = false;
                let queued = entry.queued.take();
                match result {
                    Ok(workspace) => store.workspace_loaded(&id, workspace, fresh, cx),
                    Err(err) => entry.failed(&err, failure),
                }
                // Checking against a stale one could drop a selection the operation just made.
                if fresh {
                    store.validate_selection(&id, cx);
                }
                // A project just added opens on its page while it has no target: that's
                // where one is picked. One without commits stays on its files, as there's no
                // branch to pick until the first commit makes one, which it then targets.
                let untargeted = (store.workspace(&id))
                    .is_some_and(|ws| ws.target.is_none() && ws.tree.is_some());
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

    /// `fresh` is false for a load that started before the project's operation finished.
    fn workspace_loaded(
        &mut self,
        id: &ProjectId,
        workspace: Workspace,
        fresh: bool,
        cx: &mut Context<Self>,
    ) {
        // While a commit is being edited there's nothing else to look at or act on than its
        // files, and what was seen waits for the workspace to come back.
        if workspace.editing.is_some() {
            if let Some(entry) = self.project_mut(id) {
                entry.workspace = Some(workspace);
                entry.read();
            }
            let elsewhere = (self.selection.as_ref()).is_some_and(|s| {
                s.project() == id && !matches!(s, Selection::Project(_) | Selection::Uncommitted(_))
            });
            if elsewhere {
                self.select(Some(Selection::Uncommitted(id.clone())), cx);
            }
            return;
        }
        let seen_changed = self.update_seen(id, &workspace, fresh);
        self.note_arrivals(id, &workspace);
        // An agent amending or rebasing the selected commit gives it a new id but keeps its
        // change id, or with plain Git its author, time and message; stay on it.
        // One that's gone, as squashed away, leaves its branch selected, if that's still there.
        // A selected branch renamed, as an agent names one, is followed by its newest commit.
        let old = self.workspace(id);
        let renamed = renamed(old, &workspace);
        let (rewritten, gone_from) = match &self.selection {
            Some(Selection::Commit(project, commit)) if project == id => {
                match follow_by_key(old, &workspace, *commit) {
                    Some(new) => ((new != *commit).then_some((*commit, new)), None),
                    None => {
                        let branch = old.and_then(|old| old.find_commit(*commit));
                        (None, branch.and_then(|(branch, _)| branch.ref_name.clone()))
                    }
                }
            }
            Some(Selection::Branch(project, ref_name)) if project == id => {
                let to = renamed.iter().find(|(from, _)| from == ref_name);
                (None, to.map(|(_, to)| to.clone()))
            }
            _ => (None, None),
        };
        let gone_from = gone_from.filter(|branch| workspace.find_branch(Some(branch)).is_some());
        // Gone from a linked worktree still there, as when an agent checked out another branch
        // in it, the worktree stays selected rather than the main checkout.
        let home = (self.selection.as_ref())
            .filter(|s| s.project() == id && rewritten.is_none() && gone_from.is_none())
            .filter(|s| match s {
                Selection::Branch(_, branch) => workspace.find_branch(branch.as_ref()).is_none(),
                Selection::Commit(_, commit) => workspace.find_commit(*commit).is_none(),
                _ => false,
            })
            .and_then(|s| self.worktree_for(s))
            .map(|worktree| worktree.name.clone())
            .filter(|name| workspace.worktrees.iter().any(|w| w.name == *name));
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
            let kept = follow_cut(&mut cut, self.workspace(id), &workspace);
            // Let go of one that changed out loud, rather than ⌘V doing nothing.
            self.cut = Some(cut);
            if !kept {
                self.set_cut(None, cx);
                self.snack("What was cut changed, so it was let go", cx);
            }
        }
        // What's being dragged follows too, so letting go still lands it.
        if let Some(mut dragged) = self.dragging.take_if(|cut| cut.project() == id) {
            match follow_cut(&mut dragged, self.workspace(id), &workspace) {
                true => self.dragging = Some(dragged),
                false => self.snack("What's being dragged changed, so it won't land", cx),
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
            let names = |ws: &Workspace| -> HashSet<bstr::BString> {
                ws.worktrees.iter().map(|w| w.name.clone()).collect()
            };
            old.editing.is_none() && names(old) != names(&workspace)
        });
        self.remember_commits(id, &workspace, cx);
        if let Some(entry) = self.project_mut(id) {
            entry.workspace = Some(workspace);
            entry.read();
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
            self.select(Some(Selection::Branch(id.clone(), Some(branch))), cx);
        }
        if let Some(name) = home {
            self.select(Some(Selection::Worktree(id.clone(), name)), cx);
        }
        // A stack goes by its bottom branch, so it follows that one's rename.
        if let Some(Selection::Stack(project, bottom)) = &self.selection
            && project == id
            && let Some((_, to)) = renamed.iter().find(|(from, _)| from == bottom)
        {
            self.select(Some(Selection::Stack(id.clone(), Some(to.clone()))), cx);
        }
        for (from, to) in renamed {
            cx.emit(StoreEvent::Renamed(id.clone(), from, to));
        }
        // Not on every load, as an agent writing reloads often, and quitting saves anyway.
        if seen_changed {
            self.save_soon(cx);
        }
    }
}

/// The branches of `old` renamed in `new`, by full ref, from and to: gone by name while their
/// newest commit is on a branch `new` named, in a linked worktree too. One deleted from a
/// stack isn't renamed to the branch that was there before and took its commits, nor to no
/// name, as its commits left unnamed.
fn renamed(old: Option<&Workspace>, new: &Workspace) -> Vec<(Option<FullName>, FullName)> {
    (old.iter().flat_map(|old| old.all_branches()))
        .filter(|branch| new.find_branch(branch.ref_name.as_ref()).is_none())
        .filter_map(|branch| {
            let newest = follow_by_key(old, new, branch.commits.first()?.id)?;
            let (now, _) = new.find_commit(newest)?;
            let to = (now.ref_name.clone())
                .filter(|to| old.is_some_and(|old| old.find_branch(Some(to)).is_none()))?;
            Some((branch.ref_name.clone(), to))
        })
        .collect()
}

/// Follow what's cut or dragged from `old` to where its commits are in `new`. False once it
/// changed past following.
fn follow_cut(cut: &mut Cut, old: Option<&Workspace>, new: &Workspace) -> bool {
    let follow = |commit| match old.is_some_and(|old| old.find_commit(commit).is_some()) {
        true => follow_by_key(old, new, commit),
        false => Some(commit),
    };
    match cut {
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
        // a file once it changed, as the lines picked may have moved under the pick; a linked
        // worktree's whole file too, as what an agent wrote since would go along unseen, out
        // of its worktree, which undo can't bring back. A branch goes when it's gone, as when
        // renamed.
        Cut::Files {
            from: None,
            worktree,
            files,
            ..
        } => {
            let uncommitted = new.uncommitted_in(worktree);
            let same = |f: &Selected, u: &FileChange| {
                (f.hunks.is_none() && !worktree.is_linked())
                    || (u.modified, u.size, u.lines) == (f.file.modified, f.file.size, f.file.lines)
            };
            uncommitted.is_some_and(|uncommitted| {
                // By path, as all of thousands can be cut, and this runs on every reload.
                let by_path: HashMap<&str, &FileChange> =
                    (uncommitted.iter()).map(|u| (u.path.as_str(), u)).collect();
                (files.iter())
                    .all(|f| (by_path.get(f.file.path.as_str())).is_some_and(|u| same(f, u)))
            })
        }
        Cut::Branch(_, ref_name) => new.find_branch(Some(ref_name)).is_some(),
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

/// Why a project didn't load: plainly when its folder is gone or its index sparse, otherwise
/// as reported.
pub(super) fn load_error(failure: &backend::LoadFailure, err: &anyhow::Error) -> SharedString {
    if failure.folder_gone {
        "The project's folder isn't there any more.".into()
    } else if failure.sparse_index {
        "GitButler can't read a sparse index yet. `git sparse-checkout reapply \
         --no-sparse-index` keeps the sparse checkout and makes it readable."
            .into()
    } else {
        backend::read_failure(err).into()
    }
}

/// Watch `project` for changes, sending them as `id`'s to `changes`.
fn watch(
    project: &Project,
    id: ProjectId,
    changes: mpsc::UnboundedSender<Change>,
) -> anyhow::Result<Watcher> {
    backend::watch::watch(project, move |git, checkout, paths| {
        changes
            .unbounded_send((id.clone(), git, checkout, paths))
            .ok();
    })
}

#[cfg(test)]
mod tests {
    use crate::backend::CommitId;
    use crate::backend::model::build::{branch, full, workspace, worktree};

    #[test]
    fn picks_show_together_only_as_one_run_in_one_lane() {
        let ws = workspace(
            vec![
                vec![branch("top", &[1, 2]), branch("bottom", &[3, 4])],
                vec![branch("other", &[5])],
            ],
            vec![worktree("agent", None, vec![branch("linked", &[6, 7])])],
        );
        let id = |n: u8| CommitId(gix::ObjectId::from_bytes_or_panic(&[n; 20]));
        let ids = |ns: &[u8]| ns.iter().map(|&n| id(n)).collect::<Vec<_>>();
        assert_eq!(
            ws.run_of(&ids(&[3, 2])),
            Some((id(2), id(3))),
            "across stacked branches, newest first whatever the order picked"
        );
        assert_eq!(
            ws.run_of(&ids(&[6, 7])),
            Some((id(6), id(7))),
            "in a linked worktree"
        );
        assert_eq!(ws.run_of(&ids(&[1, 3])), None, "a gap leaves 2 out");
        assert_eq!(ws.run_of(&ids(&[4, 5])), None, "two stacks");
        assert_eq!(ws.run_of(&ids(&[2])), None, "one is just itself");
    }

    #[test]
    fn a_commits_lane_is_its_stack_or_linked_worktree() {
        let ws = workspace(
            vec![
                vec![branch("top", &[1, 2]), branch("bottom", &[3])],
                vec![branch("other", &[4])],
            ],
            vec![worktree("agent", None, vec![branch("linked", &[5, 6])])],
        );
        let id = |n: u8| CommitId(gix::ObjectId::from_bytes_or_panic(&[n; 20]));
        let ids = |ns: &[u8]| ns.iter().map(|&n| id(n)).collect::<Vec<_>>();
        assert_eq!(
            ws.lane_commits(id(3)),
            Some(ids(&[1, 2, 3])),
            "a stack's commits across its branches, newest first"
        );
        assert_eq!(
            ws.lane_commits(id(5)),
            Some(ids(&[5, 6])),
            "a linked worktree's"
        );
        assert_eq!(ws.lane_commits(id(9)), None, "a commit not shown has none");
    }

    #[test]
    fn renames_are_new_named_branches_holding_a_gone_branchs_commits() {
        let old = workspace(
            vec![vec![branch("a", &[1]), branch("c", &[2])]],
            vec![worktree("agent", None, vec![branch("y", &[3])])],
        );
        let new = workspace(
            vec![vec![branch("b", &[1]), branch("c", &[2])]],
            vec![worktree("agent", None, vec![branch("yy", &[3])])],
        );
        let renamed = |from: &str, to: &str| {
            (
                Some(full(&format!("refs/heads/{from}"))),
                full(&format!("refs/heads/{to}")),
            )
        };
        assert_eq!(
            super::renamed(Some(&old), &new),
            [renamed("a", "b"), renamed("y", "yy")],
            "in the workspace and in a linked worktree alike"
        );
        let deleted = workspace(
            vec![vec![branch("c", &[1, 2])]],
            vec![worktree("agent", None, vec![branch("", &[3])])],
        );
        assert!(
            super::renamed(Some(&old), &deleted).is_empty(),
            "a branch whose commits went to one already there, or to no name, was deleted"
        );
    }
}
