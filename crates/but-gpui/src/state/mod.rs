//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's been cut, and each project's operation in flight.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use gpui::{Context, EventEmitter, SharedString, Subscription, Task};

use crate::backend::upstream::TargetCommit;
use crate::backend::watch::Watcher;
use crate::backend::{
    self, Branch, Commit, CommitId, Project, Selected, SnapshotId, Workspace, Worktree, short_ref,
};

mod account;
mod background;
mod load;
mod mesh;
mod mutation;
mod picks;
mod saved;
mod seen;

use load::{Change, Reload};
pub use mesh::{MeshBranch, MeshPick, MeshState};
pub use mutation::{NewAt, Overlay, Remap, Rereads, SLOW};
use mutation::{Queued, Running};
pub use picks::Picks;
pub use saved::{FileSort, Pane, Prefs, ScopeLines, WatchedOrder, saved_window_place};
use saved::{Restored, Saved};
pub use seen::Seeable;
use seen::Seen;

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
    /// A linked worktree, by name, and its uncommitted changes.
    Worktree(ProjectId, Arc<str>),
    /// A stack of two or more branches, by its bottom branch, and what they change together.
    Stack(ProjectId, String),
    /// A checkout of the project on one of the user's other machines, by the mesh's id for
    /// it, or a branch or commit in it.
    Mesh(ProjectId, Arc<str>, MeshPick),
    /// A commit only a remote has: one of a branch's (by its full ref) its remote has, or
    /// with `None` one of the target's the workspace doesn't have yet.
    Upstream(ProjectId, Option<String>, CommitId),
}

impl Selection {
    pub fn project(&self) -> &ProjectId {
        match self {
            Selection::Project(project)
            | Selection::Uncommitted(project)
            | Selection::Branch(project, _)
            | Selection::Commit(project, _)
            | Selection::Branches(project)
            | Selection::History(project)
            | Selection::Worktree(project, _)
            | Selection::Stack(project, _)
            | Selection::Mesh(project, ..)
            | Selection::Upstream(project, ..) => project,
        }
    }

    /// Whether it's `branch`, one of its commits, or one only its remote has.
    pub fn on(&self, branch: &Branch) -> bool {
        match self {
            Selection::Branch(_, ref_name) => branch.ref_name == *ref_name,
            Selection::Commit(_, id) => branch.commits.iter().any(|commit| commit.id == *id),
            Selection::Upstream(_, Some(ref_name), _) => branch.ref_name == *ref_name,
            _ => false,
        }
    }
}

/// What ⌘X picked up, waiting for ⌘V on a destination.
#[derive(Debug, Clone)]
pub enum Cut {
    /// Commits to move, the picked ones together, in the order they're listed.
    Commit(ProjectId, Arc<[CommitId]>),
    /// Commits to put copies of somewhere else, the picked ones together, in the order
    /// they're listed.
    CherryPick(ProjectId, Arc<[CommitId]>),
    /// A branch to stack on another, with the branches above it.
    Branch(ProjectId, String),
    /// Files from the worktree (`from: None`), down to the hunks checked, or from a commit.
    Files {
        project: ProjectId,
        from: Option<CommitId>,
        /// The linked worktree uncommitted files are in, or `None` for the main checkout's
        /// and for a commit's.
        worktree: Option<Arc<str>>,
        /// Shared, since every row a drag passes over works out what a drop would do.
        files: Arc<[Selected]>,
    },
}

impl Cut {
    pub fn project(&self) -> &ProjectId {
        match self {
            Cut::Commit(project, _)
            | Cut::CherryPick(project, _)
            | Cut::Branch(project, _)
            | Cut::Files { project, .. } => project,
        }
    }

    /// "Cherry-picking" for a cherry-pick, which leaves the commit where it is; "Cut"
    /// otherwise. Not "Copied", which ⌘C says of the clipboard.
    pub fn verb(&self) -> &'static str {
        if matches!(self, Cut::CherryPick(..)) {
            "Cherry-picking"
        } else {
            "Cut"
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Cut::Commit(_, ids) | Cut::CherryPick(_, ids) if ids.len() > 1 => {
                format!("{} commits", ids.len())
            }
            Cut::Commit(_, ids) | Cut::CherryPick(_, ids) => format!("commit {}", ids[0].short()),
            Cut::Branch(_, ref_name) => short_ref(ref_name).to_owned(),
            // Hunks or lines of one file: what of it isn't said here, where the diff isn't.
            Cut::Files { files, .. } if files.len() == 1 => match &files[0].hunks {
                Some(_) => format!("changes in {}", files[0].file.path),
                None => files[0].file.path.clone(),
            },
            Cut::Files { files, .. } => format!("{} files", files.len()),
        }
    }
}

#[derive(Debug)]
pub enum StoreEvent {
    /// A project's workspace was (re)loaded.
    Loaded(ProjectId),
    SelectionChanged,
    /// The selected page was clicked in the navigator again, to go back to how it opens.
    Reopened,
    /// A project's branch, by full ref, is gone by that name while its newest commit is on
    /// another now, as when an agent renames it.
    Renamed(ProjectId, String, String),
    /// What the user's other machines have of a project was read again, and changed.
    MeshRead(ProjectId),
    /// An operation on a project was queued, started or finished, which hides or shows
    /// what it takes away.
    Pending(ProjectId),
}

/// An error card's button: its label, and what it runs.
pub type Retry = (
    SharedString,
    std::rc::Rc<dyn Fn(&mut Store, &mut Context<Store>)>,
);

/// Something to tell the user, shown the way GitButler's other apps do.
pub enum Notice {
    /// A one-line result at the bottom centre. `undo` is the oplog entry the operation
    /// recorded, which the snackbar's Undo restores.
    Snackbar {
        message: SharedString,
        undo: Option<(ProjectId, SnapshotId)>,
    },
    /// A failure: a card at the bottom right with the error kept for copying, and what
    /// to try instead, if anything.
    Error {
        title: SharedString,
        detail: SharedString,
        retry: Option<Retry>,
    },
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
    /// What to read again once the running load lands, if anything was asked for meanwhile.
    queued: Option<Reload>,
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
    /// Commits picked along with the selected one (⌘- or ⇧-click, ⇧-arrows) to squash or
    /// uncommit together: two or more, or none, all in the selected commit's project.
    pub picked: Picks<CommitId>,
    /// The commit the selection was on and the one it moved to, when it last moved because
    /// an operation rewrote the commit.
    followed: Option<(CommitId, CommitId)>,
    /// A project just added, until its first load says which page it opens on.
    added: Option<ProjectId>,
    /// What the pointer is dragging, followed as agents rewrite it: GPUI's own copy stays as
    /// the drag began. Only meaningful while GPUI has a drag active.
    pub dragging: Option<Cut>,
    /// The row the drag is over and where on it a drop lands, when it takes the drop.
    pub drop_at: Option<(Selection, crate::transfer::Place)>,
    /// Pane widths the user dragged away from their defaults.
    widths: HashMap<Pane, f32>,
    /// Where the window is, saved for the next launch.
    window: Option<saved::WindowPlace>,
    /// What was open when the app last quit, until the first project listing uses it.
    restored: Restored,
    /// Counts projects added and removed, as each starts and lands, so a listing begun
    /// before one doesn't put back what it removed or drop what it added.
    project_edits: u64,
    /// The saved state as last written, so unchanged state isn't written again.
    last_saved: Vec<u8>,
    /// File lists show a tree of directories rather than a flat list.
    pub file_tree: bool,
    /// The order file lists show files in.
    pub file_sort: FileSort,
    pub prefs: Prefs,
    pub toasts: Vec<Toast>,
    /// Who is signed in to GitButler.
    pub account: account::Account,
    /// The version the desktop app's update feed offers, newer than this build, when the
    /// desktop app's binary started this client.
    pub newer_version: Option<String>,
    /// The user's other machines, and what they have of the watched projects. Everything the
    /// mesh does lives in `state/mesh.rs`; see the crate's AGENTS.md for its rules.
    pub mesh: mesh::Mesh,
    /// Each project's mutation in flight. A project's mutations run one at a time, each
    /// until the reload after it lands, so the next one sees what it did; projects don't
    /// wait on each other.
    running: HashMap<ProjectId, Running>,
    busy_ticker: Task<()>,
    /// Mutations waiting for their project's running one, in order.
    queue: VecDeque<Queued>,
    /// The oplog entry the last mutation recorded, which the Undo offered for it restores.
    last_snapshot: Option<(ProjectId, SnapshotId)>,
    /// Oplog entries of mutations that changed more than the oplog snapshots (a linked
    /// worktree, a branch outside the workspace), which ⌘Z doesn't take back, by project.
    /// Shared with a queued undo, which reads it once it runs, after what ran before it is
    /// kept.
    kept_snapshots: Arc<Mutex<HashMap<ProjectId, HashSet<SnapshotId>>>>,
    next_toast: usize,
    seen: Seen,
    /// Commit messages being written, by project and linked worktree, for coming back to.
    /// Like the composer's checks they last the session, so a message never comes back
    /// without them.
    pub drafts: HashMap<(ProjectId, Option<Arc<str>>), String>,
    /// The commits each merge of a remote brought in, by the merge, as read when its commits
    /// were first shown, and whether it brought more than those; `None` while reading.
    pub merged: HashMap<CommitId, Option<(Vec<TargetCommit>, bool)>>,
    /// The commits each project had before the app's last operation on it. Commits that
    /// appear after it are the app's own doing, so they load as already seen.
    rebaseline: HashMap<ProjectId, HashSet<String>>,
    save_task: Task<()>,
    save_queued: bool,
    _save_on_quit: Subscription,
    /// Where the watchers send what changed.
    changes: mpsc::UnboundedSender<Change>,
    _change_listener: Task<()>,
    _forge_poll: Task<()>,
    /// When each project was last fetched, in the background or when asked.
    fetched: HashMap<ProjectId, Instant>,
    /// When each project's background fetch last failed, as on credentials the user turned
    /// down or being offline; it waits a while before asking again, unless a fetch the user
    /// asks for works.
    fetch_failed: HashMap<ProjectId, Instant>,
    _auto_fetch: Task<()>,
    _update_check: Task<()>,
    forge_requests: mpsc::UnboundedSender<ProjectId>,
    /// Projects waiting for a forge refresh, so each waits only once.
    forge_queued: HashSet<ProjectId>,
    /// Projects whose last forge refresh failed, which the minute poll leaves alone.
    forge_failed: HashSet<ProjectId>,
    /// The project whose forge refresh is under way.
    forge_checking: Option<ProjectId>,
    /// When each project's forge refresh last worked, in seconds since the epoch. A refresh
    /// that changes nothing reloads nothing, so the cache's own time on screen falls behind.
    forge_synced: HashMap<ProjectId, i64>,
    _forge_worker: Task<()>,
}

impl EventEmitter<StoreEvent> for Store {}

impl Store {
    /// Projects listed and nothing loading or mutating, for the dev script's timings.
    /// Nothing waits without something running: queued mutations wait on a running one,
    /// and queued loads on a load in flight.
    #[cfg(feature = "dev-script")]
    pub fn is_settled(&self) -> bool {
        self.projects_loaded && self.running.is_empty() && self.projects.iter().all(|p| !p.loading)
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        let (changes, changed) = mpsc::unbounded();
        let (forge_requests, requested) = mpsc::unbounded();
        let saved = Saved::load();
        let mut store = Self {
            projects: Vec::new(),
            projects_loaded: false,
            selection: None,
            cut: None,
            picked: Picks::default(),
            followed: None,
            added: None,
            dragging: None,
            drop_at: None,
            toasts: Vec::new(),
            account: account::Account::default(),
            newer_version: None,
            mesh: mesh::Mesh::start(cx),
            running: HashMap::new(),
            busy_ticker: Task::ready(()),
            queue: VecDeque::new(),
            last_snapshot: None,
            kept_snapshots: Arc::new(Mutex::new(
                (saved.kept_by_project.into_iter())
                    .map(|(project, ids)| {
                        let ids = (ids.iter())
                            .filter_map(|id| gix::ObjectId::from_hex(id.as_bytes()).ok())
                            .map(SnapshotId)
                            .collect();
                        (ProjectId::from(project), ids)
                    })
                    .collect(),
            )),
            next_toast: 0,
            seen: saved.seen,
            drafts: HashMap::new(),
            merged: HashMap::new(),
            widths: saved.widths,
            file_tree: saved.file_tree,
            file_sort: saved.file_sort,
            prefs: saved.prefs,
            window: saved.window,
            restored: Restored {
                expanded: saved.expanded,
                selection: saved.selection,
            },
            project_edits: 0,
            last_saved: Vec::new(),
            rebaseline: HashMap::new(),
            save_task: Task::ready(()),
            save_queued: false,
            _save_on_quit: cx.on_app_quit(|store, _| {
                store.save();
                async {}
            }),
            changes,
            _change_listener: Self::reload_on_change(changed, cx),
            forge_requests,
            forge_queued: HashSet::new(),
            forge_failed: HashSet::new(),
            forge_checking: None,
            forge_synced: HashMap::new(),
            _forge_worker: Self::forge_worker(requested, cx),
            _forge_poll: Self::poll_forge(cx),
            fetched: HashMap::new(),
            fetch_failed: HashMap::new(),
            _auto_fetch: Self::fetch_in_background(cx),
            _update_check: Self::check_for_updates(cx),
        };
        store.load_projects(cx);
        store.load_account(cx);
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

    /// `project`'s commit `id`, and the branch holding it.
    pub fn find_commit(&self, project: &str, id: CommitId) -> Option<(&Branch, &Commit)> {
        self.workspace(project)?.find_commit(id)
    }

    /// Read the commits a remote's merge `id` of `project` brought in, unless they've been
    /// asked for.
    pub fn load_merged(&mut self, project: &ProjectId, id: CommitId, cx: &mut Context<Self>) {
        /// How many of a merge's commits are listed.
        const MERGED_SHOWN: usize = 200;
        let Some(path) = self
            .path(project)
            .filter(|_| !self.merged.contains_key(&id))
        else {
            return;
        };
        self.merged.insert(id, None);
        cx.spawn(async move |this, cx| {
            let read =
                backend::run(move || backend::upstream::merged_commits(&path, id, MERGED_SHOWN))
                    .await;
            this.update(cx, |store, cx| {
                let read = read
                    .inspect_err(|err| tracing::warn!(?err, "couldn't read a merge's commits"))
                    .unwrap_or_default();
                store.merged.insert(id, Some(read));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Commit `id` of those the merges only `ref_name`'s remote has (the target's with
    /// `None`) brought in, as read so far, and the merge that brought it.
    pub fn merged_upstream(
        &self,
        project: &str,
        ref_name: Option<&str>,
        id: CommitId,
    ) -> Option<(CommitId, &TargetCommit)> {
        let merges = self.workspace(project)?.upstream(ref_name).iter();
        merges.filter(|commit| commit.merge).find_map(|merge| {
            let (merged, _) = self.merged.get(&merge.id)?.as_ref()?;
            Some((merge.id, merged.iter().find(|commit| commit.id == id)?))
        })
    }

    pub fn path(&self, id: &str) -> Option<PathBuf> {
        Some(self.project(id)?.project.path.clone())
    }

    /// What a menu or the palette acts on: the selection, and how many commits are picked
    /// along with it.
    pub fn acting_on(&self) -> (Option<Selection>, usize) {
        (self.selection.clone(), self.picked.len())
    }

    /// Whether [`Self::acting_on`] is still `was`, or the same commit under the id an agent's
    /// rewrite gave it.
    pub fn still_acting_on(&self, was: &(Option<Selection>, usize)) -> bool {
        match (was, self.acting_on()) {
            ((Some(Selection::Commit(p, from)), picks), (Some(Selection::Commit(q, to)), now)) => {
                *p == q && *picks == now && (*from == to || self.followed(*from, to))
            }
            (was, now) => *was == now,
        }
    }

    /// The linked worktree whose branches `selection`, a branch or a commit, is on, and not
    /// the workspace's too. Its agent works on them there, so nothing here changes them.
    pub fn worktree_of(&self, selection: &Selection) -> Option<&Worktree> {
        // A stack is where its bottom branch is.
        let bottom;
        let selection = match selection {
            Selection::Stack(project, ref_name) => {
                bottom = Selection::Branch(project.clone(), ref_name.clone());
                &bottom
            }
            _ => selection,
        };
        let workspace = self.workspace(selection.project())?;
        if workspace.branches().any(|branch| selection.on(branch)) {
            return None;
        }
        (workspace.worktrees.iter()).find(|wt| wt.branches.iter().any(|b| selection.on(b)))
    }

    /// Whether the selection is on a linked worktree's branches, saying to do `what`, as
    /// "Push it", there instead when it is.
    pub fn changes_elsewhere(&mut self, what: &str, cx: &mut Context<Self>) -> bool {
        let selection = self.selection.as_ref();
        let Some(name) = selection
            .and_then(|s| self.worktree_of(s))
            .map(|wt| wt.name.clone())
        else {
            return false;
        };
        self.snack(
            format!("{what} in worktree {name}, where it's checked out"),
            cx,
        );
        true
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
            | Selection::History(_)
            | Selection::Worktree(..)
            | Selection::Stack(..)
            | Selection::Mesh(..)
            | Selection::Upstream(..) => None,
        }
    }

    /// Whether watched project `id`'s card shows only its row: it's folded, and the selection
    /// isn't on something in it.
    pub fn project_folded(&self, id: &ProjectId) -> bool {
        self.prefs.folded_projects.contains(id.as_ref())
            && !(self.selection.as_ref())
                .is_some_and(|s| s.project() == id && !matches!(s, Selection::Project(_)))
    }

    /// Fold watched project `id`'s card to its row, or unfold it. Folding it over the selection
    /// takes the selection up to the project, as the navigator's other folds do.
    pub fn set_project_folded(&mut self, id: &ProjectId, folded: bool, cx: &mut Context<Self>) {
        let inside = (self.selection.as_ref())
            .is_some_and(|s| s.project() == id && !matches!(s, Selection::Project(_)));
        self.set_prefs(
            |prefs| {
                match folded {
                    true => prefs.folded_projects.insert(id.to_string()),
                    false => prefs.folded_projects.remove(id.as_ref()),
                };
            },
            cx,
        );
        if folded && inside {
            self.select(Some(Selection::Project(id.clone())), cx);
        }
    }

    /// Fold every watched project's card to its row, or with `folded` false open them all.
    /// The selection goes up to its project when a fold hides it.
    pub fn fold_all(&mut self, folded: bool, cx: &mut Context<Self>) {
        let watched = self.expanded_ids();
        self.set_prefs(
            |prefs| match folded {
                true => (prefs.folded_projects).extend(watched.iter().map(|id| id.to_string())),
                false => prefs.folded_projects.clear(),
            },
            cx,
        );
        if folded
            && let Some(project) = self.selection.as_ref().map(|s| s.project().clone())
            && watched.contains(&project)
        {
            self.select(Some(Selection::Project(project)), cx);
        }
    }

    /// Put the watched projects in `order`. Going back to [`WatchedOrder::Manual`] keeps
    /// them as `shown`, so nothing moves until one's moved by hand.
    pub fn set_watched_order(
        &mut self,
        order: WatchedOrder,
        shown: Vec<ProjectId>,
        cx: &mut Context<Self>,
    ) {
        if order == WatchedOrder::Manual && self.prefs.watched_order != order {
            self.order_projects(&shown);
            self.save_soon(cx);
        }
        self.set_prefs(|prefs| prefs.watched_order = order, cx);
    }

    /// The projects open in the navigator.
    pub(crate) fn expanded_ids(&self) -> Vec<ProjectId> {
        (self.projects.iter())
            .filter(|entry| entry.expanded)
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// Keep the selection pointing at something that still exists after `id` reloaded.
    /// Another project's selection waits for its own reload: it may be what an operation
    /// there just made.
    fn validate_selection(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.clone().filter(|s| s.project() == id) else {
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
            Selection::Worktree(_, name) => workspace.worktrees.iter().any(|w| w.name == *name),
            Selection::Stack(_, bottom) => workspace.lane(bottom).is_some_and(|b| b.len() > 1),
            // The mesh's reads keep it current.
            Selection::Mesh(..) => true,
            Selection::Upstream(project, ref_name, id) => {
                (workspace.upstream(ref_name.as_deref()).iter()).any(|c| c.id == *id)
                    || self
                        .merged_upstream(project, ref_name.as_deref(), *id)
                        .is_some()
            }
        };
        if !still_there {
            // A remote's commit, pulled or brought in by an update, goes up to its branch, or
            // the target's to the project's page.
            let project = selection.project().clone();
            let home = match selection {
                Selection::Upstream(_, Some(ref_name), _)
                    if workspace.find_branch(&ref_name).is_some() =>
                {
                    Selection::Branch(project, ref_name)
                }
                Selection::Upstream(_, None, _) => Selection::Project(project),
                _ => Selection::Uncommitted(project),
            };
            self.select(Some(home), cx);
        }
    }

    pub fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        self.followed = None;
        if !self.picked.is_empty() {
            self.picked.clear();
            cx.notify();
        }
        if self.selection == selection {
            return;
        }
        // Opened, it's read, by both ids, so a rebase by Git keeps it read, as does pulling
        // one only its branch's remote had.
        if let Some(Selection::Commit(project, id) | Selection::Upstream(project, Some(_), id)) =
            &selection
        {
            self.mark_read(project, [*id], true, cx);
        }
        // An unwatched project's page reads it once, unwatched, so it can say whether the
        // target moved on and update it.
        if let Some(Selection::Project(project)) = &selection
            && self.project(project).is_some_and(|entry| !entry.expanded)
        {
            // A moment later, so arrowing past unwatched projects doesn't read each one.
            let project = project.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                this.update(cx, |store, cx| {
                    if store.selection == Some(Selection::Project(project.clone())) {
                        store.load(&project, Reload::full(), cx)
                    }
                })
                .ok();
            })
            .detach();
        }
        self.selection = selection;
        self.save_soon(cx);
        cx.emit(StoreEvent::SelectionChanged);
        cx.notify();
    }

    /// ⌘-click picks `id` along with the selected commit, or drops it from the picks;
    /// ⇧-click and ⇧-arrows pick the run of commits to `id` in `range`, the commits as
    /// they're shown, from where the last other click left off. The clicked commit becomes
    /// the selection while it's picked.
    pub fn pick(
        &mut self,
        project: &ProjectId,
        id: CommitId,
        range: Option<Vec<CommitId>>,
        cx: &mut Context<Self>,
    ) {
        let at = match &self.selection {
            Some(Selection::Commit(p, at)) if p == project => *at,
            _ => return self.select(Some(Selection::Commit(project.clone(), id)), cx),
        };
        // Taken out while selecting, which lets picks go.
        let mut picks = std::mem::take(&mut self.picked);
        let next = match range {
            Some(order) => picks.run(&order, &at, &id),
            None => Some(picks.toggle(&at, &id)),
        };
        if let Some(next) = next {
            self.select(Some(Selection::Commit(project.clone(), next)), cx);
        }
        self.picked = picks;
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

    /// A snackbar for the mutation that just ran, offering to undo it when it's `undoable`.
    /// Otherwise ⌘Z won't take it back either.
    pub fn snack_after(
        &mut self,
        undoable: bool,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        if !undoable {
            self.keep_last_snapshot();
        }
        self.snack_undoable(message, cx)
    }

    /// Keep ⌘Z, and the snackbar's Undo, from taking back the mutation that just ran: it
    /// changed what the oplog can't bring back, as a linked worktree.
    pub fn keep_last_snapshot(&mut self) {
        if let Some((project, snapshot)) = self.last_snapshot.take() {
            (self.kept_snapshots.lock().expect("not poisoned"))
                .entry(project)
                .or_default()
                .insert(snapshot);
        }
    }

    /// What moving commit `id` of `project` takes: the picked commits, in the order they're
    /// listed, when it's one of several, or else just it.
    pub fn with_picks(&self, project: &ProjectId, id: CommitId) -> Arc<[CommitId]> {
        if !self.among_picks(project, id) {
            return [id].into();
        }
        (self.workspace(project).into_iter())
            .flat_map(|ws| ws.commits())
            .filter(|commit| self.picked.contains(&commit.id))
            .map(|commit| commit.id)
            .collect()
    }

    /// Whether commit `id` of `project` is picked. Picks are the selected project's: another
    /// project can have the same commits, as a second clone of a repository does.
    pub fn is_picked(&self, project: &ProjectId, id: CommitId) -> bool {
        (self.selection.as_ref()).is_some_and(|s| s.project() == project)
            && self.picked.contains(&id)
    }

    /// Whether commit `id` of `project` is one of several picked.
    pub fn among_picks(&self, project: &ProjectId, id: CommitId) -> bool {
        self.picked.len() > 1 && self.is_picked(project, id)
    }

    /// Whether `id` of `project` is a linked worktree's commit.
    pub fn in_worktree(&self, project: &ProjectId, id: CommitId) -> bool {
        self.worktree_of(&Selection::Commit(project.clone(), id))
            .is_some()
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
                retry: None,
            },
            cx,
        );
    }

    /// An error card whose button runs `retry`, as to try again or go ahead anyway.
    pub fn error_with_retry(
        &mut self,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        retry_label: impl Into<SharedString>,
        retry: impl Fn(&mut Store, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.notify_user(
            Notice::Error {
                title: title.into(),
                detail: detail.into(),
                retry: Some((retry_label.into(), std::rc::Rc::new(retry))),
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
        // One snackbar at a time; errors stack, though the same failure again replaces its card.
        self.toasts.retain(|toast| match (&toast.notice, &notice) {
            (Notice::Snackbar { .. }, Notice::Snackbar { .. }) => false,
            (
                Notice::Error { title, detail, .. },
                Notice::Error {
                    title: t,
                    detail: d,
                    ..
                },
            ) => (title, detail) != (t, d),
            _ => true,
        });
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
        // The snackbar saying a cut is ready to paste is wrong once it's pasted or let go.
        if cut.is_none() && self.cut.is_some() {
            (self.toasts)
                .retain(|toast| !matches!(toast.notice, Notice::Snackbar { undo: None, .. }));
        }
        self.cut = cut;
        cx.notify();
    }
}
