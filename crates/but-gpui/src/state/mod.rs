//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's been cut, and each project's operation in flight.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use futures::channel::mpsc;
use gpui::{App, Context, Entity, EventEmitter, SharedString, Subscription, Task};
use parking_lot::Mutex;

use crate::backend::upstream::TargetCommit;
use crate::backend::watch::Watcher;
use crate::backend::{
    self, Branch, Checkout, Commit, CommitId, FullName, Project, Selected, SnapshotId, Workspace,
    short_ref,
};

mod account;
mod activity;
mod background;
mod checkout;
mod cli;
mod load;
mod mesh;
mod mutation;
mod picks;
mod saved;
mod seen;

pub use activity::{Activity, QUARTER, SeenCommit};
use load::{Change, Reload};
pub use mesh::{MeshBranch, MeshPick, MeshState};
pub use mutation::{
    Label, NewAt, OpKind, Overlay, Remap, Rereads, SLOW, branch_moved, conflicted_parts,
};
use mutation::{Queued, Running};
pub use picks::Picks;
pub use saved::{BranchStatus, FileSort, Pane, Prefs, ScopeLines, WatchedOrder};
use saved::{Restored, Saved};
use seen::Seen;
pub use seen::{GLOW, Seeable};

pub type ProjectId = SharedString;

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Project(ProjectId),
    Uncommitted(ProjectId),
    /// A branch by its full ref, or `None` for an unnamed one.
    Branch(ProjectId, Option<FullName>),
    Commit(ProjectId, CommitId),
    /// The branches outside the workspace, to apply or delete.
    Branches(ProjectId),
    /// The operation log, to look back at and restore.
    History(ProjectId),
    /// A linked worktree, by name, and its uncommitted changes.
    Worktree(ProjectId, bstr::BString),
    /// A stack of two or more branches, by its bottom branch, and what they change together.
    Stack(ProjectId, Option<FullName>),
    /// A checkout of the project on one of the user's other machines, by the mesh's id for
    /// it, or a branch or commit in it.
    Mesh(ProjectId, Arc<str>, MeshPick),
    /// A commit only a remote has: one of a branch's (by its full ref) its remote has, or
    /// with `None` one of the target's the workspace doesn't have yet.
    Upstream(ProjectId, Option<FullName>, CommitId),
    /// The heading over a section of a project's card, whose page shows what it folds away.
    Section(ProjectId, Section),
}

/// A section of a project's card that folds away under its heading.
#[derive(Debug, Clone, PartialEq)]
pub enum Section {
    /// The commits only a remote has: branch `String`'s (by its full ref), or with `None` the
    /// target's new ones.
    Upstream(Option<FullName>),
    /// The linked worktrees with nothing going on lately.
    Inactive,
    /// The checkouts on the user's other machines.
    Mesh,
}

impl Selection {
    pub fn uncommitted_in(project: ProjectId, checkout: Checkout) -> Self {
        match checkout {
            Checkout::Main => Selection::Uncommitted(project),
            Checkout::Linked(name) => Selection::Worktree(project, name),
        }
    }

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
            | Selection::Upstream(project, ..)
            | Selection::Section(project, _) => project,
        }
    }

    /// Whether it's `branch`, or under it.
    pub fn on(&self, branch: &Branch) -> bool {
        matches!(self, Selection::Branch(_, ref_name) if branch.ref_name == *ref_name)
            || self.under(branch)
    }

    /// Whether it's what folding `branch` hides: one of its commits, one only its remote has,
    /// or the heading over those.
    pub fn under(&self, branch: &Branch) -> bool {
        match self {
            Selection::Commit(_, id) => branch.commits.iter().any(|commit| commit.id == *id),
            Selection::Upstream(_, Some(ref_name), _)
            | Selection::Section(_, Section::Upstream(Some(ref_name))) => {
                branch.ref_name.as_ref() == Some(ref_name)
            }
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
    Branch(ProjectId, FullName),
    /// Files from the worktree (`from: None`), down to the hunks checked, or from a commit.
    Files {
        project: ProjectId,
        from: Option<CommitId>,
        /// The checkout uncommitted files are in, the main one for a commit's.
        worktree: Checkout,
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

    /// What's cut, a commit by its title where the workspace has one for it.
    pub fn describe(&self, store: &Store) -> String {
        match self {
            Cut::Commit(project, ids) | Cut::CherryPick(project, ids) => match &**ids {
                [id] => match store.find_commit(project, *id) {
                    Some((_, commit)) if !commit.title.is_empty() => {
                        format!("\"{}\"", commit.title)
                    }
                    _ => format!("commit {}", id.short()),
                },
                ids => format!("{} commits", ids.len()),
            },
            Cut::Branch(_, ref_name) => short_ref(ref_name).into_owned(),
            Cut::Files { files, .. } => match &**files {
                // Hunks or lines of one file: what of it isn't said here, where the diff isn't.
                [file] => match &file.hunks {
                    Some(_) => format!("changes in {}", file.file.path),
                    None => file.file.path.clone(),
                },
                files => format!("{} files", files.len()),
            },
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
    Renamed(ProjectId, Option<FullName>, FullName),
    /// What the user's other machines have of a project was read again, and changed.
    MeshRead(ProjectId),
    /// An operation on a project was queued, started or finished, which hides or shows
    /// what it takes away.
    Pending(ProjectId),
    /// Git has no author to record commits with in a project: ask for the user's name and
    /// email.
    AuthorMissing(ProjectId),
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
    /// to try instead, if anything. `source` is the operation that wrote it, by project and
    /// label, whose next run that works takes it back.
    Error {
        title: SharedString,
        detail: SharedString,
        retry: Option<Retry>,
        source: Option<(ProjectId, SharedString)>,
    },
}

pub struct Toast {
    pub id: usize,
    pub notice: Notice,
    /// When it goes, by the wall clock: its timer stops while the Mac sleeps.
    until: SystemTime,
}

pub struct ProjectEntry {
    pub project: Project,
    pub id: ProjectId,
    pub expanded: bool,
    pub workspace: Option<Workspace>,
    pub error: Option<SharedString>,
    /// Its folder was gone when it last failed to read, which is what `error` says then.
    pub folder_gone: bool,
    /// Which local branch is the target, when the failure to read it is that several track
    /// it: the project's page asks.
    pub target_choice: Option<crate::backend::TargetChoice>,
    loading: bool,
    /// What to read again once the running load lands, if anything was asked for meanwhile.
    queued: Option<Reload>,
    watcher: Option<Watcher>,
}

impl ProjectEntry {
    pub fn is_loading(&self) -> bool {
        self.loading && self.workspace.is_none()
    }

    /// Keep why it couldn't be read, `err` or its folder being gone, and what to choose when
    /// that's the way out.
    fn failed(&mut self, err: &anyhow::Error, failure: crate::backend::LoadFailure) {
        // Once as it stops reading, as every reload until it's fixed fails again.
        if self.error.is_none() {
            crate::backend::metrics::load_failed(&failure, err, self.workspace.as_ref());
        }
        self.folder_gone = failure.folder_gone;
        self.error = Some(load::load_error(&failure, err));
        self.target_choice = failure.target_choice;
    }

    /// It read, so there's nothing wrong to say.
    fn read(&mut self) {
        self.folder_gone = false;
        self.error = None;
        self.target_choice = None;
    }
}

/// Spans of time in milliseconds since the Unix epoch, by project.
type Spans = HashMap<ProjectId, Vec<(i64, i64)>>;

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
    /// While an operation of a project the user has left finishes, which leaves the selection,
    /// and what's picked with it, where the user is.
    staying: bool,
    /// A project just added, until its first load says which page it opens on.
    added: Option<ProjectId>,
    /// Whether the user was asked for Git's missing author since the app started: once is
    /// enough, as it's set for every repository.
    asked_author: bool,
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
    /// Where the saved state goes.
    saved_db: Arc<backend::saved::Database>,
    /// How many times the saved state has been taken to write, numbering each.
    saves: u64,
    pub prefs: Prefs,
    pub toasts: Vec<Toast>,
    /// Who is signed in to GitButler.
    pub account: account::Account,
    /// Where a terminal finds `but`, if it does; `None` until looked for, which only macOS
    /// does.
    pub cli: Option<Option<PathBuf>>,
    /// While this app's `but` installs.
    pub installing_cli: bool,
    /// The version the desktop app's update feed offers, newer than this build, when the
    /// desktop app's binary started this client.
    pub newer_version: Option<String>,
    /// How much of it has downloaded, in percent, while it installs.
    pub updating: Option<u8>,
    /// While Later puts the update card off, the timer that brings it back.
    pub update_put_off: Option<Task<()>>,
    /// The user's other machines, and what they have of the watched projects. Everything the
    /// mesh does lives in `state/mesh.rs`; see the crate's AGENTS.md for its rules.
    pub mesh: mesh::Mesh,
    /// Each project's mutation in flight. A project's mutations run one at a time, each
    /// until the reload after it lands, so the next one sees what it did; projects don't
    /// wait on each other.
    running: HashMap<ProjectId, Running>,
    /// When, in seconds since the Unix epoch, each project's mutations of the last two
    /// minutes wrote: each from its start until the file watcher has told of its writes, a
    /// second or so after it settled. What's written then isn't activity in the checkout.
    own_writes: HashMap<ProjectId, Vec<std::ops::RangeInclusive<i64>>>,
    /// When, in milliseconds since the Unix epoch, each project last pushed: undoing what
    /// came before rewrites what the remote has.
    pushed_at: HashMap<ProjectId, i64>,
    busy_ticker: Task<()>,
    /// Mutations waiting for their project's running one, in order.
    queue: VecDeque<Queued>,
    /// The oplog entry the last mutation recorded, which the Undo offered for it restores.
    last_snapshot: Option<(ProjectId, SnapshotId)>,
    /// What ⌘Z takes back, newest last: the oplog entries the user's operations recorded, in
    /// whichever project each ran. So ⌘Z undoes what the user last did, wherever the
    /// selection is, rather than what last happened in the selected project.
    undo_steps: Vec<mutation::UndoStep>,
    /// What ⌘⇧Z redoes, newest last: the oplog entries ⌘Z recorded taking something back, in
    /// whichever project, since the user last did something.
    redo_steps: Vec<(ProjectId, SnapshotId)>,
    /// When, in milliseconds since the Unix epoch, the app's operations ran, ⌘Z and ⌘⇧Z
    /// included but not pushes and fetches, by project: the files they wrote and the branches
    /// they moved aren't changes a later ⌘Z asks about. Shared as `kept_snapshots` is, so a queued undo sees the spans of
    /// those before it.
    ran: Arc<Mutex<Spans>>,
    /// Oplog entries of mutations that changed more than the oplog snapshots (a linked
    /// worktree, a branch outside the workspace), which ⌘Z doesn't take back, by project.
    /// Shared with a queued undo, which reads it once it runs, after what ran before it is
    /// kept.
    kept_snapshots: Arc<Mutex<HashMap<ProjectId, HashSet<SnapshotId>>>>,
    next_toast: usize,
    seen: Seen,
    /// When each commit an agent made showed up, while its rows still glow: see
    /// [`Store::glows`].
    arrived: HashMap<CommitId, Instant>,
    /// Commit messages being written, by project and checkout, for coming back to.
    /// Like the composer's checks they last the session, so a message never comes back
    /// without them.
    pub drafts: HashMap<(ProjectId, Checkout), String>,
    /// The commits each merge of a remote brought in, by the merge, as read when its commits
    /// were first shown, and whether it brought more than those; `None` while reading.
    pub merged: HashMap<CommitId, Option<(Vec<TargetCommit>, bool)>>,
    /// The commits each project had before the app's last operation on it, and the checkouts
    /// it made commits in: see [`Overlay::checkouts`]. Commits that appear there after it are
    /// the app's own doing, so they load as already seen.
    rebaseline: HashMap<ProjectId, (HashSet<String>, mutation::Checkouts)>,
    save_task: Task<()>,
    save_queued: bool,
    _save_on_quit: Subscription,
    /// Where the watchers send what changed.
    changes: mpsc::UnboundedSender<Change>,
    _change_listener: Task<()>,
    _forge_poll: Task<()>,
    /// When each project was last fetched, in the background or when asked, by the wall
    /// clock, which counts the time the Mac slept.
    fetched: HashMap<ProjectId, SystemTime>,
    /// When each project's background fetch last failed, as on credentials the user turned
    /// down or being offline, or found no remote; it waits a while before asking again,
    /// unless a fetch the user asks for works.
    fetch_failed: HashMap<ProjectId, SystemTime>,
    _auto_fetch: Task<()>,
    _update_check: Task<()>,
    forge_requests: mpsc::UnboundedSender<ProjectId>,
    /// Projects waiting for a forge refresh, so each waits only once.
    forge_queued: HashSet<ProjectId>,
    /// Projects whose last forge refresh failed, which the minute poll leaves alone.
    forge_failed: HashSet<ProjectId>,
    /// No part of the window shows, as behind another app's or with the display asleep.
    /// Nobody sees the projects change then, so what watchers saw waits in `unseen`, read
    /// once the window shows again.
    hidden: bool,
    unseen: HashMap<ProjectId, Reload>,
    /// Minute forge polls skipped while hidden since the forge was last asked: see
    /// [`Store::poll_forge`].
    forge_skipped: u32,
    /// The project whose forge refresh is under way.
    forge_checking: Option<ProjectId>,
    /// When each project's forge refresh last worked, in seconds since the epoch. A refresh
    /// that changes nothing reloads nothing, so the cache's own time on screen falls behind.
    forge_synced: HashMap<ProjectId, i64>,
    _forge_worker: Task<()>,
    /// What the forge said about whether pull requests can merge, by project and number: see
    /// [`Store::read_merge_status`].
    merge_reads: HashMap<(ProjectId, u64), background::MergeRead>,
    /// What the forge said has been written on pull requests, by project and number: see
    /// [`Store::read_conversation`].
    conversation_reads: HashMap<(ProjectId, u64), background::ConversationRead>,
    /// What the file watcher saw written in each checkout lately.
    pub activity: HashMap<(ProjectId, Checkout), Activity>,
}

impl EventEmitter<StoreEvent> for Store {}

/// [`Store::snack`] from a view holding the store.
pub fn snack(store: &Entity<Store>, message: impl Into<SharedString>, cx: &mut App) {
    store.update(cx, |store, cx| store.snack(message, cx));
}

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
        let saved_db = Arc::new(backend::saved::Database::open());
        let (saved, fresh) = Saved::load(&saved_db);
        let mut store = Self {
            projects: Vec::new(),
            projects_loaded: false,
            selection: None,
            cut: None,
            picked: Picks::default(),
            followed: None,
            staying: false,
            added: None,
            asked_author: false,
            dragging: None,
            drop_at: None,
            toasts: Vec::new(),
            account: account::Account::default(),
            cli: None,
            installing_cli: false,
            newer_version: None,
            updating: None,
            update_put_off: None,
            mesh: mesh::Mesh::start(cx),
            running: HashMap::new(),
            own_writes: HashMap::new(),
            pushed_at: HashMap::new(),
            busy_ticker: Task::ready(()),
            queue: VecDeque::new(),
            last_snapshot: None,
            undo_steps: Vec::new(),
            redo_steps: Vec::new(),
            ran: Arc::default(),
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
            arrived: HashMap::new(),
            drafts: HashMap::new(),
            merged: HashMap::new(),
            widths: saved.widths,
            prefs: saved.prefs,
            window: saved.window,
            restored: Restored {
                expanded: saved.expanded,
                selection: saved.selection,
                fresh,
            },
            project_edits: 0,
            saved_db,
            saves: 0,
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
            hidden: false,
            unseen: HashMap::new(),
            forge_skipped: 0,
            forge_checking: None,
            forge_synced: HashMap::new(),
            _forge_worker: Self::forge_worker(requested, cx),
            merge_reads: HashMap::new(),
            conversation_reads: HashMap::new(),
            activity: (saved.activity.into_iter())
                .map(|saved| {
                    let checkout = (saved.worktree)
                        .map_or(Checkout::Main, |name| Checkout::Linked(name.into()));
                    ((saved.project.into(), checkout), saved.activity)
                })
                .collect(),
            _forge_poll: Self::poll_forge(cx),
            fetched: HashMap::new(),
            fetch_failed: HashMap::new(),
            _auto_fetch: Self::fetch_in_background(cx),
            _update_check: Self::check_for_updates(cx),
        };
        store.load_projects(cx);
        store.load_account(cx);
        store.read_cli(cx);
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
        let project = project.clone();
        cx.spawn(async move |this, cx| {
            let read =
                backend::run(move || backend::upstream::merged_commits(&path, id, MERGED_SHOWN))
                    .await;
            this.update(cx, |store, cx| {
                // Said once, rather than opening on nothing; reading it again on each reload
                // of the page would say so again each time.
                let read = read.unwrap_or_else(|err| {
                    let title =
                        store.card_title(&project, "Couldn't read the merge's commits".into());
                    store.failed(title, &err, cx);
                    Default::default()
                });
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
        ref_name: Option<&FullName>,
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

    /// What a menu or the palette acts on: the selection, and the commits picked along with
    /// it, in the order picked.
    pub fn acting_on(&self) -> (Option<Selection>, Vec<CommitId>) {
        (self.selection.clone(), self.picked.to_vec())
    }

    /// Whether [`Self::acting_on`] is still `was`, or the same commit under the id an agent's
    /// rewrite gave it, with as many picked.
    pub fn still_acting_on(&self, was: &(Option<Selection>, Vec<CommitId>)) -> bool {
        let picks = self.picked.len();
        match (was, &self.selection) {
            ((Some(Selection::Commit(p, from)), was), Some(Selection::Commit(q, to))) => {
                p == q && was.len() == picks && (from == to || self.followed(*from, *to))
            }
            ((was, was_picks), now) => was == now && was_picks.len() == picks,
        }
    }

    /// The branch `selection` is, or the one holding the commit it is.
    pub fn branch_for(&self, selection: &Selection) -> Option<&Branch> {
        let workspace = self.workspace(selection.project())?;
        match selection {
            Selection::Branch(_, ref_name) => workspace.find_branch(ref_name.as_ref()),
            Selection::Commit(_, id) => workspace.find_commit(*id).map(|(branch, _)| branch),
            Selection::Project(_)
            | Selection::Uncommitted(_)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Worktree(..)
            | Selection::Stack(..)
            | Selection::Mesh(..)
            | Selection::Upstream(..)
            | Selection::Section(..) => None,
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
            Selection::Branch(_, ref_name) => workspace.find_branch(ref_name.as_ref()).is_some(),
            Selection::Commit(_, id) => workspace.find_commit(*id).is_some(),
            Selection::Worktree(_, name) => workspace.worktrees.iter().any(|w| w.name == *name),
            Selection::Stack(_, bottom) => workspace
                .lane(bottom.as_ref())
                .is_some_and(|lane| lane.branches.len() > 1),
            // The mesh's reads keep it current.
            Selection::Mesh(..) | Selection::Section(_, Section::Mesh) => true,
            Selection::Section(_, Section::Upstream(ref_name)) => {
                !workspace.upstream(ref_name.as_ref()).is_empty()
            }
            Selection::Section(_, Section::Inactive) => {
                workspace.worktrees.iter().any(|wt| wt.is_inactive())
            }
            Selection::Upstream(project, ref_name, id) => {
                (workspace.upstream(ref_name.as_ref()).iter()).any(|c| c.id == *id)
                    || self
                        .merged_upstream(project, ref_name.as_ref(), *id)
                        .is_some()
            }
        };
        if !still_there {
            // A remote's commit, pulled or brought in by an update, goes up to its branch, or
            // the target's to the project's page, as does a section emptied.
            let project = selection.project().clone();
            let home = match selection {
                Selection::Upstream(_, Some(ref_name), _)
                | Selection::Section(_, Section::Upstream(Some(ref_name)))
                    if workspace.find_branch(Some(&ref_name)).is_some() =>
                {
                    Selection::Branch(project, Some(ref_name))
                }
                Selection::Upstream(_, None, _) | Selection::Section(..) => {
                    Selection::Project(project)
                }
                _ => Selection::Uncommitted(project),
            };
            self.select(Some(home), cx);
        }
    }

    pub fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        if self.staying {
            return;
        }
        self.followed = None;
        if !self.picked.is_empty() {
            self.picked.clear();
            cx.notify();
        }
        if self.selection == selection {
            return;
        }
        if let Some(selection) = &selection {
            self.count_view(selection);
        }
        // Opened, it's read, by both ids, so a rebase by Git keeps it read, as does pulling
        // one only its branch's remote had.
        if let Some(Selection::Commit(project, id) | Selection::Upstream(project, Some(_), id)) =
            &selection
        {
            self.mark_read(project, [*id], true, cx);
        }
        if let Some(selection) = &selection
            && let Some(id) = (self.workspace(selection.project()))
                .and_then(|workspace| seen::lone_commit(workspace, selection))
        {
            self.mark_read(selection.project(), [id], true, cx);
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

    /// For the metrics: what the user opens in the window, with the app as it is then.
    fn count_view(&self, selection: &Selection) {
        // Selecting what's left as it changes, while the user is away, isn't use, nor is the
        // launch putting back what was selected, before the workspace has loaded.
        let workspace = self.workspace(selection.project());
        let Some(workspace) = workspace.filter(|_| !self.hidden) else {
            return;
        };
        let view = match selection {
            Selection::Project(_) => "project",
            Selection::Uncommitted(_) => "uncommitted",
            Selection::Branch(..) => "branch",
            Selection::Commit(..) => "commit",
            Selection::Branches(_) => "branches",
            Selection::History(_) => "history",
            Selection::Worktree(..) => "worktree",
            Selection::Stack(..) => "stack",
            Selection::Mesh(..) => "mesh",
            Selection::Upstream(..) => "upstream",
            Selection::Section(..) => "section",
        };
        backend::metrics::viewed(view, || backend::metrics::State {
            theme: match self.prefs.appearance {
                crate::theme::Appearance::System => "system",
                crate::theme::Appearance::Light => "light",
                crate::theme::Appearance::Dark => "dark",
            },
            zoom: f64::from(self.prefs.ui_scale.percent()) / 100.,
            editor: self.prefs.editor.as_deref(),
            ligatures: self.prefs.ligatures,
            projects: self.projects.len(),
            workspace,
        });
    }

    /// Select `selection`, or with it selected already, take its page back to how it opens.
    pub fn reopen(&mut self, selection: Selection, cx: &mut Context<Self>) {
        let again = self.selection.as_ref() == Some(&selection);
        self.select(Some(selection), cx);
        if again {
            cx.emit(StoreEvent::Reopened);
        }
    }

    /// ⌘-click picks `id` along with the selected commit, or drops it from the picks;
    /// ⇧-click and ⇧-arrows pick the run of commits to `id` in `range`, the commits as
    /// they're shown, from where the last other click left off. The clicked commit becomes
    /// the selection while it's picked. Picks stay in one stack, or linked worktree's lane, so
    /// their changes show as one diff: a commit in another is selected alone.
    pub fn pick(
        &mut self,
        project: &ProjectId,
        id: CommitId,
        range: Option<Vec<CommitId>>,
        cx: &mut Context<Self>,
    ) {
        let lane = (self.workspace(project))
            .and_then(|ws| ws.lane_commits(id))
            .unwrap_or_default();
        let at = match &self.selection {
            Some(Selection::Commit(p, at)) if p == project && lane.contains(at) => *at,
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
            self.keep_last_snapshot(cx);
        }
        self.snack_undoable(message, cx)
    }

    /// Keep ⌘Z, and the snackbar's Undo, from taking back the mutation that just ran, as a
    /// land, which the target keeps; History can still restore to before it.
    pub fn drop_last_step(&mut self) -> Option<(ProjectId, SnapshotId)> {
        let (project, snapshot) = self.last_snapshot.take()?;
        (self.undo_steps).retain(|step| step.project != project || step.snapshot != snapshot);
        Some((project, snapshot))
    }

    /// Whether something written in `project` at `at`, in seconds since the Unix epoch, is the
    /// app's own operation's doing rather than activity in the checkout.
    pub fn own_write(&self, project: &ProjectId, at: i64) -> bool {
        (self.own_writes.get(project)).is_some_and(|spans| spans.iter().any(|s| s.contains(&at)))
    }

    /// Whether ⌘Z has something the user did to take back.
    pub fn can_undo(&self) -> bool {
        !self.undo_steps.is_empty()
    }

    /// Keep ⌘Z, and the snackbar's Undo, from taking back the mutation that just ran: it
    /// changed what the oplog can't bring back, as a linked worktree.
    pub fn keep_last_snapshot(&mut self, cx: &mut Context<Self>) {
        if let Some((project, snapshot)) = self.drop_last_step() {
            (self.kept_snapshots.lock())
                .entry(project)
                .or_default()
                .insert(snapshot);
            self.save_soon(cx);
        }
    }

    /// What acting on commit `id` of `project` takes: the picked commits, in the order they're
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

    /// The newest and oldest of the commits picked in `project`, as [`Workspace::run_of`].
    pub fn picked_run(&self, project: &ProjectId) -> Option<(CommitId, CommitId)> {
        self.workspace(project)?.run_of(&self.picked)
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

    /// A snackbar offering to undo the mutation that just ran.
    pub fn snack_undoable(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let message = message.into();
        let undo = self.last_snapshot.clone();
        // Once the snackbar goes, the status bar says it while ⌘Z still takes it back.
        if let Some((project, snapshot)) = &undo
            && let Some(step) = (self.undo_steps.iter_mut())
                .find(|step| step.project == *project && step.snapshot == *snapshot)
        {
            step.said = Some(message.clone());
        }
        // Done in another project than the one shown, it says which its Undo takes back.
        let message = match &undo {
            Some((project, _)) => self.card_title(project, message.to_string()).into(),
            None => message,
        };
        self.notify_user(Notice::Snackbar { message, undo }, cx);
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
                source: None,
            },
            cx,
        );
    }

    /// An error card saying `title`, with what went wrong in `err` in the user's words where
    /// there are some. The metrics name it by where it was asked for.
    #[track_caller]
    pub fn failed(
        &mut self,
        title: impl Into<SharedString>,
        err: &anyhow::Error,
        cx: &mut Context<Self>,
    ) {
        backend::metrics::failure_shown(
            std::panic::Location::caller(),
            err,
            self.projects
                .iter()
                .filter_map(|project| project.workspace.as_ref()),
        );
        self.error(title, backend::failure_detail(err), cx)
    }

    /// `title` for a card or the status bar about `project`, naming it when the selection is
    /// in another project.
    pub fn card_title(&self, project: &ProjectId, title: String) -> String {
        title + &self.in_project(project)
    }

    /// " in shop" when `project` isn't the one shown, for a card's text to say where it was.
    pub fn in_project(&self, project: &ProjectId) -> String {
        match self.project(project) {
            Some(entry) if (self.selection.as_ref()).is_some_and(|s| s.project() != project) => {
                format!(" in {}", entry.project.name)
            }
            _ => String::new(),
        }
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
                source: None,
            },
            cx,
        );
    }

    fn notify_user(&mut self, notice: Notice, cx: &mut Context<Self>) {
        // Every error card shown, for the log to say what went wrong for the user and when.
        if let Notice::Error { title, detail, .. } = &notice {
            tracing::error!("{title}: {detail}");
        }
        let id = self.next_toast;
        self.next_toast += 1;
        let linger = match notice {
            Notice::Snackbar { undo: None, .. } => Duration::from_secs(3),
            Notice::Snackbar { undo: Some(_), .. } => Duration::from_secs(6),
            // Long enough to read what going ahead anyway would do, but not to go ahead on
            // what has changed since, as a push forced over newer commits.
            Notice::Error { retry: Some(_), .. } => Duration::from_secs(60),
            Notice::Error { retry: None, .. } => Duration::from_secs(15),
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
        let until = SystemTime::now() + linger;
        self.toasts.push(Toast { id, notice, until });
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

    /// Dismiss card `id` to go ahead with what it offers, and whether it's still current:
    /// one shown before the Mac slept can outlast its time, offering to go ahead on what
    /// has changed since.
    pub fn take_toast(&mut self, id: usize, cx: &mut Context<Self>) -> bool {
        let now = SystemTime::now();
        let current = (self.toasts.iter()).any(|toast| toast.id == id && now < toast.until);
        self.dismiss_toast(id, cx);
        if !current {
            self.snack("That was a while ago, so try it again", cx);
        }
        current
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
