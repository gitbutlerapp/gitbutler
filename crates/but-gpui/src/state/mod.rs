//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's being dragged, and each project's operation in flight.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use futures::channel::mpsc;
use gpui::{App, Context, Entity, EventEmitter, SharedString, Subscription, Task, WindowId};

use crate::backend::upstream::TargetCommit;
use crate::backend::watch::Watcher;
use crate::backend::{
    self, Branch, Checkout, Commit, CommitId, FullName, Project, SnapshotId, Workspace,
};

mod account;
mod activity;
mod background;
mod briefs;
mod checkout;
mod cli;
mod load;
mod mesh;
mod mutation;
mod picks;
mod saved;
mod seen;
mod terminals;
mod tiling;

pub use activity::{Activity, QUARTER, SeenCommit};
use load::{Change, Reload};
pub use mesh::{MeshBranch, MeshPick, MeshPickKind, MeshState};
use mutation::Running;
pub use mutation::{
    BlockedCommit, Label, NewAt, NewCommit, Overlay, Rereads, SLOW, branch_moved, conflicted_parts,
    marked_parts, up_to_date,
};
pub use picks::Picks;
pub use saved::{
    BranchStatus, FileSort, FileView, NavigatorPlace, Pane, Part, Prefs, ProjectGroup, ScopeLines,
    TerminalGrouping, TerminalRuns, WatchedOrder, screen_reader_on,
};
use saved::{Restored, Saved};
use seen::Seen;
pub use seen::{GLOW, Seeable};
pub use terminals::{ProjectCheckout, Terminals};
pub use tiling::{
    Area, Axis, Divider, DividerDrag, Drawn, Junction, Landing, Opened, RowKey, Side, Tile,
    TileDragFeel, TileIds, Tiling, Zones, along_edge, junctions, nearest_beside, tile_beside,
};

pub type ProjectId = SharedString;

/// What's selected: a row of the navigator's, or a page reached from one. Its kinds are listed
/// broadest first, the order Go to anything offers them in, and each needs a decision there
/// ([`SelectionKind`]).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    strum::EnumDiscriminants,
    serde::Serialize,
    serde::Deserialize,
)]
#[strum_discriminants(
    name(SelectionKind),
    derive(strum::EnumIter, strum::IntoStaticStr, strum::EnumString),
    // As a `but://open` link names the kind (`to=`), so renaming one renames its links: see
    // `crate::links`.
    strum(serialize_all = "kebab-case")
)]
pub enum Selection {
    /// A project's page, whose tabs are what its page keeps: see [`ProjectTab`].
    Project(ProjectId),
    /// The heading over a section of a project's card, whose page shows what it folds away.
    Section(ProjectId, Section),
    /// A worktree's heading, the main one's or a linked one's by name, whose page is its
    /// overview: what's going on in it and what just happened there.
    Checkout(ProjectId, Checkout),
    /// A worktree's uncommitted changes, first under its heading, whose page is their files
    /// and diff; the main worktree's are also the files of a commit being edited.
    Uncommitted(ProjectId, Checkout),
    /// A terminal open on one of the project's worktrees.
    Terminal(ProjectId, crate::terminal::TerminalId),
    /// A branch by its full ref, or `None` for an unnamed one.
    Branch(ProjectId, Option<FullName>),
    /// A stack of two or more branches, by its bottom branch, and what they change together.
    Stack(ProjectId, Option<FullName>),
    Commit(ProjectId, CommitId),
    /// The commit one of the project's operations is making, by the operation's id and where it
    /// goes, until the reload after it shows the commit made.
    Pending(ProjectId, usize, NewAt),
    /// A commit only a branch's remote has, by the branch's full ref. The target's new
    /// commits are listed on the project's page.
    Upstream(ProjectId, FullName, CommitId),
    /// A checkout of the project on one of the user's other machines, by the mesh's id for
    /// it, or a branch or commit in it.
    Mesh(ProjectId, Arc<str>, MeshPick),
}

/// A tab of a project's page, as a branch's page has Changes and its pull request. Which one
/// shows is the page's, so it stays with the tile showing it.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
    strum::IntoStaticStr,
    strum::EnumString,
)]
// As a `but://open` link names the tab (`tab=`), so renaming one renames its links: see
// `crate::links`.
#[strum(serialize_all = "kebab-case")]
pub enum ProjectTab {
    #[default]
    Overview,
    /// The branches outside the workspace, to apply or delete.
    Branches,
    /// The operation log, to look back at and restore.
    History,
    /// The project's commits drawn as a graph, the workspace's and the history below them.
    Graph,
}

impl ProjectTab {
    /// Every tab, in the order they show.
    pub const ALL: [ProjectTab; 4] = [
        ProjectTab::Overview,
        ProjectTab::Branches,
        ProjectTab::History,
        ProjectTab::Graph,
    ];

    /// The tab on that side of this one: none past either end.
    pub fn beside(self, forward: bool) -> Option<ProjectTab> {
        let at = ProjectTab::ALL.iter().position(|tab| *tab == self)?;
        let beside = at.checked_add_signed(if forward { 1 } else { -1 })?;
        ProjectTab::ALL.get(beside).copied()
    }

    /// The tab after this one, or before it going back, round again past either end.
    pub fn round(self, forward: bool) -> ProjectTab {
        let (first, last) = (ProjectTab::Overview, ProjectTab::Graph);
        self.beside(forward)
            .unwrap_or(if forward { first } else { last })
    }
}

/// A section of a project's card that folds away under its heading. Each kind needs a
/// decision in Go to anything too ([`SectionKind`]).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    strum::EnumDiscriminants,
    serde::Serialize,
    serde::Deserialize,
)]
#[strum_discriminants(
    name(SectionKind),
    derive(strum::EnumIter, strum::IntoStaticStr, strum::EnumString),
    // As a `but://open` link names the section (`section=`), so renaming one renames its links.
    strum(serialize_all = "kebab-case")
)]
pub enum Section {
    /// The commits only a branch's remote has, by the branch's full ref.
    Upstream(FullName),
    /// The linked worktrees with nothing going on lately.
    Inactive,
    /// The checkouts on the user's other machines.
    Mesh,
}

impl Selection {
    pub fn project(&self) -> &ProjectId {
        match self {
            Selection::Project(project)
            | Selection::Checkout(project, _)
            | Selection::Uncommitted(project, _)
            | Selection::Branch(project, _)
            | Selection::Commit(project, _)
            | Selection::Pending(project, ..)
            | Selection::Stack(project, _)
            | Selection::Mesh(project, ..)
            | Selection::Upstream(project, ..)
            | Selection::Section(project, _)
            | Selection::Terminal(project, _) => project,
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
            // A commit on its way, where it goes.
            Selection::Pending(.., NewAt::Top(ref_name)) => {
                branch.ref_name.as_ref() == Some(ref_name)
            }
            Selection::Pending(.., NewAt::Above(key) | NewAt::Below(key)) => {
                branch.commits.iter().any(|commit| commit.key == *key)
            }
            Selection::Upstream(_, ref_name, _)
            | Selection::Section(_, Section::Upstream(ref_name)) => {
                branch.ref_name.as_ref() == Some(ref_name)
            }
            _ => false,
        }
    }
}

/// A place in the changes shown that a brief or a link points to.
#[derive(Debug, Clone, PartialEq)]
pub enum CodePlace {
    /// A file, at the hunk with this line of its new side if it has one.
    File { path: String, line: Option<u32> },
    /// A line of a hunk of a file.
    Line {
        path: String,
        hunk: usize,
        line: usize,
    },
}

#[derive(Debug)]
pub enum StoreEvent {
    Loaded(ProjectId),
    SelectionChanged,
    /// The selected page was asked for again, as by the crumb naming it, to go back to how it
    /// opens.
    Reopened,
    /// A tab of the project being selected was asked for, as Go to anything does.
    TabAsked(ProjectTab),
    /// A place in the changes of the selection a `but://open` link is making was asked for:
    /// their diff opens there once it's read, and says so if it lacks it.
    FileAsked(Selection, CodePlace),
    /// The project a `but://open` link names is open, for the window to go where `at` points in
    /// it.
    LinkOpened {
        project: ProjectId,
        at: Vec<(String, String)>,
    },
    /// A project's branch, by full ref, is gone by that name while its newest commit is on
    /// another now, as when an agent renames it.
    Renamed(ProjectId, Option<FullName>, FullName),
    /// What the user's other machines have of a project was read again, and changed.
    MeshRead(ProjectId),
    /// An operation on a project started or finished, which hides or shows
    /// what it takes away.
    Pending(ProjectId),
    /// Git has no author to record commits with in a project: ask for the user's name and
    /// email.
    AuthorMissing(ProjectId),
    /// The folder given to add isn't a repository but holds these: ask which to add.
    RepositoriesInside(PathBuf, Vec<PathBuf>),
    /// AI was asked to write with no provider set up: sign in to GitButler for its key, or
    /// once `signed_in`, pick one in Settings.
    AiNotSetUp {
        signed_in: bool,
    },
    /// A terminal was opened and selected, to type in.
    TerminalOpened,
    /// A tile was pinned, to use, and to take the keys with `keys`: not when `but _ui` opened it
    /// in the background.
    TilePinned {
        tile: RowKey,
        keys: bool,
    },
    /// The tile of these changes was asked to show a place in them, by the brief beside it.
    CodeAsked(RowKey, CodePlace),
    /// `tile` was taken away from the selected row's tiling, which was `before`, so the app
    /// view can draw it going: with the terminal it showed, which went with it, if it did.
    TileClosed {
        before: Tiling,
        tile: RowKey,
        terminal: Option<Entity<crate::terminal::Terminal>>,
    },
    /// A tile was pinned to a row or taken away.
    TilingChanged,
    /// The selected row's tiling is the row `to`'s now, as a terminal took the home tile's place.
    TilingMoved {
        from: RowKey,
        to: RowKey,
    },
    /// The drag of a navigator row, which can land as a tile, ended.
    RowDragEnded,
    /// This was asked to be selected by hand while a move was under way, which keeps the
    /// selection on what it moves: the move goes toward it instead, as a click on its row puts
    /// it, or where it'd land blinks where it can't, to say it's still on.
    MoveToward(Selection),
    /// A commit wasn't made, as some of its changes couldn't go where it was asked to: ask
    /// the user where they go instead ([`Store::take_blocked_commit`]).
    CommitBlocked,
    /// Updating a project would leave conflicted what isn't now, as `detail` says: ask whether
    /// to update anyway, accepting just what was `found`.
    UpdateConflicts {
        project: ProjectId,
        scope: backend::upstream::UpdateScope,
        found: backend::upstream::UpdateConflicts,
        detail: String,
    },
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
        /// It went, but not quite as asked: a warning glyph leads it.
        warning: bool,
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
    /// Its folder, or the repository in it, was gone when it last failed to read, which is
    /// what `error` says then.
    pub gone: bool,
    /// Which local branch is the target, when the failure to read it is that several track
    /// it: the project's page asks.
    pub target_choice: Option<crate::backend::TargetChoice>,
    /// The commits made on top of its workspace commit, when they're why it can't be read: its
    /// page offers to uncommit them.
    pub commits_above: Option<crate::backend::CommitsAboveWorkspaceCommit>,
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
        self.gone = failure.folder_gone || failure.repo_gone;
        // What was read of it would only mislead, as files deleted on the way out, or branches
        // without the commits made on top of them, which its page offers to uncommit.
        if self.gone || failure.commits_above.is_some() {
            self.workspace = None;
        }
        self.error = Some(load::load_error(&failure, err));
        self.target_choice = failure.target_choice;
        self.commits_above = failure.commits_above;
    }

    /// It read, so there's nothing wrong to say.
    fn read(&mut self) {
        self.gone = false;
        self.error = None;
        self.target_choice = None;
        self.commits_above = None;
    }
}

/// Spans of time in milliseconds since the Unix epoch, by project.
type Spans = HashMap<ProjectId, Vec<(i64, i64)>>;

pub struct Store {
    pub projects: Vec<ProjectEntry>,
    pub projects_loaded: bool,
    pub selection: Option<Selection>,
    /// The row selected last in each project the selection has left, to go back to.
    pub left_on: HashMap<ProjectId, Selection>,
    /// The row selected last in each checkout the selection has left, likewise.
    pub left_in: HashMap<(ProjectId, Checkout), Selection>,
    /// Commits picked along with the selected one (⌘- or ⇧-click, ⇧-arrows) to squash or
    /// uncommit together: two or more, or none, all in the selected commit's project.
    pub picked: Picks<CommitId>,
    /// The commit the selection was on and the one it moved to, when it last moved because
    /// an operation rewrote the commit.
    followed: Option<(CommitId, CommitId)>,
    /// The new commit an operation selected, or the commit a link did, until the selection
    /// moves: a reload follows it by change id, as the workspace before it may never have had
    /// the commit to follow.
    made: Option<backend::Made>,
    /// While an operation of a project the user has left finishes, which leaves the selection,
    /// and what's picked with it, where the user is.
    staying: bool,
    /// What a link selected, and what to say if it isn't there, until its project's read.
    linked: Option<(Selection, &'static str)>,
    /// Whether the user was asked for Git's missing author since the app started: once is
    /// enough, as it's set for every repository.
    asked_author: bool,
    /// What the pointer is dragging, followed as agents rewrite it: GPUI's own copy stays as
    /// the drag began. Only meaningful while GPUI has a drag active.
    pub dragging: Option<crate::transfer::Dragged>,
    /// Whether a drag holds [`Self::moving`], the pointer stepping it rather than the keys.
    pub move_dragged: bool,
    /// What a terminal pastes of a drag from the diff, as it began: the patch of what it
    /// carries ([`Store::pasted`]). Only meaningful while GPUI has a drag active.
    pub dragged_patch: Option<String>,
    /// Commits being moved with the keys, followed as agents rewrite them, until they land or
    /// the selection goes elsewhere.
    pub moving: Option<crate::transfer::Move>,
    /// The checkout a file with no changes is opened to edit in: its changes are listed and
    /// shown while it is, though there may be none.
    opened: Option<(ProjectId, Checkout)>,
    /// Pane widths the user dragged away from their defaults.
    widths: HashMap<Pane, f32>,
    /// The pane sliding back to its default width after a double-click on its line, while it
    /// is: see [`Store::put_back_width`].
    width_glide: Option<(Pane, crate::ui::fold::Glide)>,
    /// Where the window is, saved for the next launch.
    window: Option<saved::WindowPlace>,
    /// What was open when the app last quit, until the first project listing uses it.
    restored: Restored,
    /// Counts projects added and removed, as each starts and lands, so a listing begun
    /// before one doesn't put back what it removed or drop what it added.
    project_edits: u64,
    saved_db: Arc<backend::saved::Database>,
    /// How many times the saved state has been taken to write, numbering each.
    saves: u64,
    pub prefs: Prefs,
    /// Diffs show only the cursor's file rather than every file one after another, as
    /// GitButler's experimental `gpuiSingleFileDiff` says.
    pub single_file_diff: bool,
    /// Whether Briefs is asked about the projects' branches.
    pub briefs: bool,
    pub toasts: Vec<Toast>,
    pub account: account::Account,
    /// Where a terminal finds `but`, if it does; `None` until looked for, which only macOS
    /// does.
    pub cli: Option<Option<PathBuf>>,
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
    pub terminals: Terminals,
    /// How each row that has tiles pinned beside its page lays them out. A row missing here
    /// shows its home tile alone.
    tilings: HashMap<RowKey, Tiling>,
    /// For each tiling, by its row, the rows its tiles show that had the keys, the last last:
    /// the home tile's as the row itself. By row, so they keep as a terminal takes the home
    /// tile's place.
    tiles_with_keys: HashMap<RowKey, Vec<RowKey>>,
    /// The number of each tile there is, for `but _ui` to name them by.
    tile_ids: TileIds,
    /// While a tile slides open or shut or is dragged, the selected row's tiling as it's drawn
    /// in this frame: see [`Self::drawn_areas`].
    drawn: Option<Drawn>,
    /// How dragging a tile feels, for this run of the app.
    pub tile_drag_feel: TileDragFeel,
    /// The navigator row being dragged, which a side of the tiles shows as a tile.
    pub dragged_row: Option<Selection>,
    /// The side of the tiles letting that row go now shows it on.
    pub tile_zone: Option<Side>,
    /// The gap being dragged, between two tiles or the navigator and the detail, until the
    /// button comes up: GPUI calls its strip unhovered as it's dragged.
    pub dragged_divider: Option<DividerDrag>,
    /// Each project's operation in flight, one at a time until the reload after it lands, so
    /// the next sees what it did, plus those that run beside it ([`backend::OpKind::beside`]).
    /// Projects don't wait on each other.
    running: Vec<Running>,
    /// How many operations have started, which numbers the next.
    ops_started: usize,
    /// When, in seconds since the Unix epoch, each project's mutations of the last two
    /// minutes wrote: each from its start until the file watcher has told of its writes, a
    /// second or so after it settled. What's written then isn't activity in the checkout.
    own_writes: HashMap<ProjectId, Vec<std::ops::RangeInclusive<i64>>>,
    /// When, in milliseconds since the Unix epoch, each project last pushed: undoing what
    /// came before rewrites what the remote has.
    pushed_at: HashMap<ProjectId, i64>,
    busy_ticker: Task<()>,
    /// The oplog entry the last mutation recorded, which the Undo offered for it restores.
    last_snapshot: Option<(ProjectId, SnapshotId)>,
    /// What ⌘Z takes back, newest last: the oplog entries the user's operations recorded, in
    /// whichever project each ran. So ⌘Z undoes what the user last did, wherever the
    /// selection is, rather than what last happened in the selected project.
    undo_steps: Vec<mutation::UndoStep>,
    /// A commit not made as some of its changes couldn't go where it was asked to, till the
    /// window asks the user where they go instead.
    blocked_commit: Option<BlockedCommit>,
    /// What ⌘⇧Z redoes, newest last: the oplog entries ⌘Z recorded taking something back, in
    /// whichever project, since the user last did something.
    redo_steps: Vec<(ProjectId, SnapshotId)>,
    /// When, in milliseconds since the Unix epoch, the app's operations ran, ⌘Z and ⌘⇧Z
    /// included but not pushes and fetches, by project: the files they wrote and the branches
    /// they moved aren't changes a later ⌘Z asks about.
    ran: Spans,
    /// Oplog entries of mutations that changed more than the oplog snapshots (a linked
    /// worktree, a branch outside the workspace), which ⌘Z doesn't take back, by project.
    kept_snapshots: HashMap<ProjectId, HashSet<SnapshotId>>,
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
    /// the app's own doing, so they load as already seen. A person's operations with the CLI
    /// or the TUI set it too, for every checkout, when the load after them shows they ran.
    rebaseline: HashMap<ProjectId, (HashSet<String>, mutation::Checkouts)>,
    save_task: Task<()>,
    save_queued: bool,
    _save_on_quit: Subscription,
    /// Where the watchers send what changed.
    changes: mpsc::UnboundedSender<Change>,
    _change_listener: Task<()>,
    _forge_poll: Task<()>,
    _brief_poll: Task<()>,
    /// What Briefs says about the projects' branches: see [`Store::brief_at`].
    brief_answers: briefs::BriefAnswers,
    /// When each project was last fetched, in the background or when asked, by the wall
    /// clock, which counts the time the Mac slept.
    fetched: HashMap<ProjectId, SystemTime>,
    /// When each project's background fetch last failed, as on credentials the user turned
    /// down or being offline, or found no remote; it waits a while before asking again,
    /// unless a fetch the user asks for works.
    fetch_failed: HashMap<ProjectId, SystemTime>,
    /// When the last pull request each project's forge has reported merged, in seconds since
    /// the epoch, once it has reported at all.
    last_merged: HashMap<ProjectId, Option<i64>>,
    /// Projects whose forge reported a pull request merged since they were last fetched. The
    /// target has its commits only once fetched, so the next background fetch takes them
    /// whatever the interval.
    fetch_soon: HashSet<ProjectId>,
    _auto_fetch: Task<()>,
    _update_check: Task<()>,
    forge_requests: mpsc::UnboundedSender<ProjectId>,
    /// Projects waiting for a forge refresh, so each waits only once.
    forge_queued: HashSet<ProjectId>,
    /// Projects whose last forge refresh failed, and whether that may pass by itself, so the
    /// minute poll asks again. The others it leaves alone.
    forge_failed: HashMap<ProjectId, bool>,
    /// No part of the window shows, as behind another app's or with the display asleep.
    /// Nobody sees the projects change then, so what watchers saw waits in `unseen`, read
    /// once the window shows again.
    hidden: bool,
    /// The windows any of which shows.
    shown_windows: HashSet<WindowId>,
    unseen: HashMap<ProjectId, Reload>,
    /// Minute forge polls skipped while hidden since the forge was last asked: see
    /// [`Store::poll_forge`].
    forge_skipped: u32,
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
    /// Nothing waits without something running: queued loads wait on a load in flight.
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
            left_on: HashMap::new(),
            left_in: HashMap::new(),
            picked: Picks::default(),
            followed: None,
            made: None,
            staying: false,
            linked: None,
            asked_author: false,
            dragging: None,
            move_dragged: false,
            dragged_patch: None,
            moving: None,
            opened: None,
            toasts: Vec::new(),
            account: account::Account::default(),
            cli: None,
            installing_cli: false,
            newer_version: None,
            updating: None,
            update_put_off: None,
            mesh: mesh::Mesh::start(cx),
            terminals: Terminals::default(),
            tilings: HashMap::new(),
            dragged_row: None,
            tile_zone: None,
            tiles_with_keys: HashMap::new(),
            tile_ids: TileIds::default(),
            drawn: None,
            tile_drag_feel: TileDragFeel::default(),
            dragged_divider: None,
            running: Vec::new(),
            ops_started: 0,
            own_writes: HashMap::new(),
            pushed_at: HashMap::new(),
            busy_ticker: Task::ready(()),
            last_snapshot: None,
            undo_steps: Vec::new(),
            blocked_commit: None,
            redo_steps: Vec::new(),
            ran: HashMap::new(),
            kept_snapshots: (saved.kept_by_project.into_iter())
                .map(|(project, ids)| {
                    let ids = (ids.iter())
                        .filter_map(|id| gix::ObjectId::from_hex(id.as_bytes()).ok())
                        .map(SnapshotId)
                        .collect();
                    (ProjectId::from(project), ids)
                })
                .collect(),
            next_toast: 0,
            seen: saved.seen,
            arrived: HashMap::new(),
            drafts: HashMap::new(),
            merged: HashMap::new(),
            widths: saved.widths,
            width_glide: None,
            prefs: saved.prefs,
            // Read with the saved state, so the diff doesn't start in the other layout.
            single_file_diff: backend::settings::single_file_diff(),
            briefs: backend::settings::briefs(),
            window: saved.window,
            restored: Restored {
                expanded: saved.expanded,
                selection: saved.selection,
                handed: None,
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
            forge_failed: HashMap::new(),
            hidden: false,
            shown_windows: HashSet::new(),
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
            _brief_poll: Self::poll_briefs(cx),
            brief_answers: briefs::BriefAnswers::default(),
            fetched: HashMap::new(),
            fetch_failed: HashMap::new(),
            last_merged: HashMap::new(),
            fetch_soon: HashSet::new(),
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

    pub fn find_commit(&self, project: &str, id: CommitId) -> Option<(&Branch, &Commit)> {
        self.workspace(project)?.find_commit(id)
    }

    /// What `project`'s commit `id` is shown by: see [`Workspace::short_reference`].
    pub fn short_reference(&self, project: &str, id: CommitId) -> String {
        (self.workspace(project)).map_or_else(|| id.short(), |ws| ws.short_reference(id))
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

    /// Commit `id` of those the merges only `ref_name`'s remote has brought in, as read so
    /// far, and the merge that brought it.
    pub fn merged_upstream(
        &self,
        project: &str,
        ref_name: &FullName,
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
            | Selection::Pending(..)
            | Selection::Checkout(..)
            | Selection::Uncommitted(_, Checkout::Main)
            | Selection::Uncommitted(_, Checkout::Linked(_))
            | Selection::Stack(..)
            | Selection::Mesh(..)
            | Selection::Upstream(..)
            | Selection::Section(..)
            | Selection::Terminal(..) => None,
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

    /// Unfold the card of the project the selection was taken into, when it's on a row in it, so
    /// it stays open once the selection leaves, as jumping there should.
    pub fn unfold_to_selection(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selection.as_ref().map(|s| s.project().clone()) else {
            return;
        };
        if self.prefs.folded_projects.contains(id.as_ref()) && !self.project_folded(&id) {
            self.set_project_folded(&id, false, cx);
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

    /// Put the watched projects in `order` by hand, switching to the manual order from the one
    /// picked if it's another and that changes where they are.
    pub fn order_watched(&mut self, order: &[ProjectId], cx: &mut Context<Self>) {
        let watched = self.expanded_ids();
        self.order_projects(order);
        if self.expanded_ids() == watched && self.prefs.watched_order == WatchedOrder::Manual {
            return;
        }
        self.save_soon(cx);
        match self.prefs.watched_order {
            WatchedOrder::Manual => cx.notify(),
            _ => self.set_prefs(|prefs| prefs.watched_order = WatchedOrder::Manual, cx),
        }
    }

    /// The projects open in the navigator.
    pub(crate) fn expanded_ids(&self) -> Vec<ProjectId> {
        (self.projects.iter())
            .filter(|entry| entry.expanded)
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// Whether a file with no changes is opened to edit in `project`'s `checkout`.
    pub fn opened_in(&self, project: &ProjectId, checkout: &Checkout) -> bool {
        (self.opened.as_ref()).is_some_and(|(p, c)| p == project && c == checkout)
    }

    /// Say a file with no changes is opened to edit in `at`, or none is. Once none is, the
    /// changes it kept listed go if there are none, and the selection with them.
    pub fn set_opened(&mut self, at: Option<(ProjectId, Checkout)>, cx: &mut Context<Self>) {
        if self.opened == at {
            return;
        }
        let was = std::mem::replace(&mut self.opened, at);
        if let Some((project, _)) = was {
            self.validate_selection(&project, cx);
        }
        cx.notify();
    }

    /// Keep the selection pointing at something that still exists after `id` reloaded.
    /// Another project's selection waits for its own reload: it may be what an operation
    /// there just made.
    fn validate_selection(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.clone().filter(|s| s.project() == id) else {
            return;
        };
        let Some(workspace) = self.workspace(selection.project()) else {
            // Of a project that's gone, or has commits made on top of its workspace, only its
            // page is left, which offers to remove it or to uncommit them.
            if (self.project(id)).is_some_and(|entry| entry.gone || entry.commits_above.is_some())
                && !matches!(selection, Selection::Project(_))
            {
                self.select(Some(Selection::Project(id.clone())), cx);
            }
            return;
        };
        if !self.still_there(&selection) {
            // A remote's commit, pulled or brought in by an update, goes up to its branch while
            // it's there, else to the project's page, as does a section emptied.
            let project = selection.project().clone();
            let home = match selection {
                Selection::Upstream(_, ref_name, _)
                | Selection::Section(_, Section::Upstream(ref_name))
                    if workspace.find_branch(Some(&ref_name)).is_some() =>
                {
                    Selection::Branch(project, Some(ref_name))
                }
                // Changes all committed or discarded go up to their linked worktree's heading,
                // if it's still there.
                Selection::Uncommitted(_, checkout @ Checkout::Linked(_))
                    if self
                        .still_there(&Selection::Checkout(project.clone(), checkout.clone())) =>
                {
                    Selection::Checkout(project, checkout)
                }
                _ => Selection::Project(project),
            };
            self.select(Some(home), cx);
        }
    }

    /// Whether `selection` points at something its project's last read still has, or at
    /// something in a project not read yet.
    pub fn still_there(&self, selection: &Selection) -> bool {
        let Some(workspace) = self.workspace(selection.project()) else {
            return true;
        };
        match selection {
            Selection::Project(_) | Selection::Checkout(_, Checkout::Main) => true,
            Selection::Branch(_, ref_name) => workspace.find_branch(ref_name.as_ref()).is_some(),
            Selection::Commit(_, id) => workspace.find_commit(*id).is_some(),
            Selection::Pending(project, op, _) => self.pending_commit(project, *op).is_some(),
            Selection::Checkout(_, Checkout::Linked(name)) => {
                workspace.worktrees.iter().any(|w| w.name == *name)
            }
            // Listed only with something uncommitted, or as a commit being edited's files, or
            // while a file's opened to edit there.
            Selection::Uncommitted(project, checkout) => {
                self.opened_in(project, checkout)
                    || (workspace.editing.is_some() && *checkout == Checkout::Main)
                    || workspace
                        .uncommitted_in(checkout)
                        .is_some_and(|f| !f.is_empty())
            }
            Selection::Terminal(_, id) => self.terminals.get(*id).is_some(),
            Selection::Stack(_, bottom) => workspace
                .lane(bottom.as_ref())
                .is_some_and(|lane| lane.branches.len() > 1),
            // The mesh's reads keep it current, while connected or not read yet.
            Selection::Mesh(..) | Selection::Section(_, Section::Mesh) => {
                self.mesh_connected_or_unread()
            }
            Selection::Section(_, Section::Upstream(ref_name)) => {
                !workspace.upstream(ref_name).is_empty()
            }
            Selection::Section(_, Section::Inactive) => {
                let after = self.prefs.inactive_after;
                workspace.worktrees.iter().any(|wt| wt.is_inactive(after))
            }
            Selection::Upstream(project, ref_name, id) => {
                (workspace.upstream(ref_name).iter()).any(|c| c.id == *id)
                    || self.merged_upstream(project, ref_name, *id).is_some()
            }
        }
    }

    /// Whether no part of the window shows.
    pub fn hidden(&self) -> bool {
        self.hidden
    }

    pub fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        if self.staying {
            return;
        }
        self.followed = None;
        self.made = None;
        // A move is of what's selected, so it ends once the selection goes elsewhere, as a
        // click on another row takes it.
        if let Some(moving) = &self.moving
            && !selection.as_ref().is_some_and(|item| moving.holds(item))
        {
            self.set_moving(None, cx);
        }
        if !self.picked.is_empty() {
            self.picked.clear();
            cx.notify();
        }
        if self.selection == selection {
            return;
        }
        match &selection {
            Some(selection) => self.count_view(selection),
            None => backend::metrics::shown(None),
        }
        // Opened, it's read, by both ids, so a rebase by Git keeps it read, as does pulling
        // one only its branch's remote had.
        if let Some(Selection::Commit(project, id) | Selection::Upstream(project, _, id)) =
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
        if let Some(left) = &self.selection
            && selection
                .as_ref()
                .is_none_or(|s| s.project() != left.project())
        {
            self.left_on.insert(left.project().clone(), left.clone());
        }
        if let Some(left) = &self.selection
            && let Some(checkout) = self.checkout_of(left)
            && selection.as_ref().is_none_or(|s| {
                s.project() != left.project() || self.checkout_of(s).as_ref() != Some(&checkout)
            })
        {
            let at = (left.project().clone(), checkout);
            self.left_in.insert(at, left.clone());
        }
        self.selection = selection;
        self.number_tiles();
        self.show_terminals(cx);
        self.save_soon(cx);
        cx.emit(StoreEvent::SelectionChanged);
        cx.notify();
    }

    /// For the metrics: what the user opens in the window, with the app as it is then.
    fn count_view(&self, selection: &Selection) {
        let view = match selection {
            // Not opened: it stands in for the commit, which counts once it's there.
            Selection::Pending(..) => return,
            Selection::Project(_) => "project",
            Selection::Checkout(..) => "checkout",
            Selection::Uncommitted(_, Checkout::Main) => "uncommitted",
            Selection::Branch(..) => "branch",
            Selection::Commit(..) => "commit",
            Selection::Uncommitted(_, Checkout::Linked(_)) => "worktree",
            Selection::Stack(..) => "stack",
            Selection::Mesh(..) => "mesh",
            Selection::Upstream(..) => "upstream",
            Selection::Section(..) => "section",
            Selection::Terminal(..) => "terminal",
        };
        backend::metrics::shown(Some(view));
        self.count_view_named(view, selection.project());
    }

    /// For the metrics: the user opened `tab` of `project`'s page.
    pub fn count_tab(&self, project: &ProjectId, tab: ProjectTab) {
        let view = match tab {
            ProjectTab::Overview => "project",
            ProjectTab::Branches => "branches",
            ProjectTab::History => "history",
            ProjectTab::Graph => "graph",
        };
        backend::metrics::shown(Some(view));
        self.count_view_named(view, project);
    }

    /// For the metrics: the user opened `view` of something in `project`. A page's own tab is
    /// counted, but its time goes to the page.
    pub fn count_view_named(&self, view: &'static str, project: &ProjectId) {
        // Selecting what's left as it changes, while the user is away, isn't use, nor is the
        // launch putting back what was selected, before the workspace has loaded.
        let workspace = self.workspace(project);
        let Some(workspace) = workspace.filter(|_| !self.hidden) else {
            return;
        };
        backend::metrics::viewed(view, || backend::metrics::State {
            prefs: &self.prefs,
            hooks: self.runs_hooks(project),
            single_file_diff: self.single_file_diff,
            briefs: self.briefs,
            projects: self.projects.len(),
            workspace,
        });
    }

    /// Select `selection`, or with it selected already, take its page back to how it opens.
    pub fn reopen(&mut self, selection: Selection, cx: &mut Context<Self>) {
        let again = self.selection.as_ref() == Some(&selection);
        if self.select_by_hand(selection, cx) && again {
            cx.emit(StoreEvent::Reopened);
        }
    }

    /// Select `selection` as the user asked, from a row, a crumb or a link, and say whether it
    /// was. While a move is under way the selection stays on what it moves, so nothing the user
    /// goes to ends it, only Escape or Return: the move goes toward it instead
    /// ([`StoreEvent::MoveToward`]).
    pub fn select_by_hand(&mut self, selection: Selection, cx: &mut Context<Self>) -> bool {
        if (self.moving.as_ref()).is_some_and(|moving| !moving.holds(&selection)) {
            cx.emit(StoreEvent::MoveToward(selection));
            return false;
        }
        // Only a deliberate navigation opens the group. Restoring the selected page on
        // launch, or a watcher following a rewritten commit, keeps the saved folds.
        let group = self
            .prefs
            .project_group(selection.project())
            .map(str::to_owned);
        let folded = self
            .prefs
            .project_groups
            .iter_mut()
            .find(|g| Some(&g.id) == group.as_ref())
            .map(|g| &mut g.folded)
            .unwrap_or(&mut self.prefs.ungrouped_folded);
        if std::mem::take(folded) {
            self.save_soon(cx);
            cx.notify();
        }
        self.select(Some(selection), cx);
        true
    }

    /// Select `project` as the user asked, and have its page show `tab`. Says whether it was.
    /// The tab's asked for first, so the page opens on it rather than on the tab it was left on.
    pub fn show_tab(
        &mut self,
        project: ProjectId,
        tab: ProjectTab,
        cx: &mut Context<Self>,
    ) -> bool {
        let selection = Selection::Project(project);
        if (self.moving.as_ref()).is_none_or(|moving| moving.holds(&selection)) {
            cx.emit(StoreEvent::TabAsked(tab));
        }
        self.select_by_hand(selection, cx)
    }

    /// Select `selection` as the user asked, and have its diff open at `place`, as a
    /// `but://open` link does. Says whether it was. The place is asked for first, as
    /// [`Store::show_tab`] asks for its tab.
    fn show_file(
        &mut self,
        selection: Selection,
        place: CodePlace,
        cx: &mut Context<Self>,
    ) -> bool {
        if (self.moving.as_ref()).is_none_or(|moving| moving.holds(&selection)) {
            cx.emit(StoreEvent::FileAsked(selection.clone(), place));
        }
        self.select_by_hand(selection, cx)
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
                warning: false,
            },
            cx,
        );
    }

    /// A snackbar for what went, but not quite as asked, as but-ui-react's warning one says it.
    pub fn snack_warning(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notify_user(
            Notice::Snackbar {
                message: message.into(),
                undo: None,
                warning: true,
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
    /// merge, which the target keeps; History can still restore to before it.
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
            (self.kept_snapshots)
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
        self.notify_user(
            Notice::Snackbar {
                message,
                undo,
                warning: false,
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

    /// [`Store::failed`] for AI writing, except that with no provider set up the window
    /// goes to setting one up ([`StoreEvent::AiNotSetUp`]) rather than showing a card.
    #[track_caller]
    pub fn ai_failed(&mut self, title: &'static str, err: &anyhow::Error, cx: &mut Context<Self>) {
        match err.downcast_ref::<backend::ai::NotSetUp>() {
            Some(&backend::ai::NotSetUp { signed_in }) => {
                cx.emit(StoreEvent::AiNotSetUp { signed_in })
            }
            None => self.failed(title, err, cx),
        }
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

    pub fn set_moving(&mut self, moving: Option<crate::transfer::Move>, cx: &mut Context<Self>) {
        self.moving = moving;
        self.move_dragged = false;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::backend::model::build::full;
    use crate::backend::{Lane, PushState, Worktree};
    use crate::transfer::{Changes, Moving};

    #[test]
    fn a_tab_has_the_tab_on_either_side() {
        assert_eq!(
            ProjectTab::Branches.beside(true),
            Some(ProjectTab::History),
            "Operations history is right of Branches"
        );
        assert_eq!(
            ProjectTab::Branches.beside(false),
            Some(ProjectTab::Overview),
            "Overview is left of Branches"
        );
    }

    #[test]
    fn nothing_is_left_of_the_first_tab() {
        assert_eq!(
            ProjectTab::Overview.beside(false),
            None,
            "the keys leave the tabs here rather than going round"
        );
    }

    #[test]
    fn nothing_is_right_of_the_last_tab() {
        assert_eq!(
            ProjectTab::Graph.beside(true),
            None,
            "the keys leave the tabs here rather than going round"
        );
    }

    #[test]
    fn going_round_the_tabs_passes_the_ends() {
        assert_eq!(
            ProjectTab::Graph.round(true),
            ProjectTab::Overview,
            "past the last tab is the first"
        );
        assert_eq!(
            ProjectTab::Overview.round(false),
            ProjectTab::Graph,
            "before the first tab is the last"
        );
    }

    #[test]
    fn a_row_dropped_on_a_terminal_pastes_its_name_or_folder() {
        let [kept, gone] = [1, 2].map(|n| CommitId(gix::ObjectId::from([n; 20])));
        let feat = full("refs/heads/feat");
        let workspace = Workspace {
            lanes: vec![Lane {
                branches: vec![Branch {
                    name: "feat".into(),
                    ref_name: Some(feat.clone()),
                    commits: vec![Commit {
                        key: "vtlrvnlmsxrmrsnrkvqlwrzqokpsoxzk".into(),
                        ..Commit::bare(kept.0)
                    }],
                    incoming: Vec::new(),
                    remote: None,
                    push: PushState::NeverPushed,
                    review: None,
                }],
                checkout: Checkout::Main,
                rests_on: None,
            }],
            worktrees: vec![Worktree {
                name: "agent".into(),
                path: "/work/agent".into(),
                branch: None,
                uncommitted: Vec::new(),
                git_operation: None,
                tree: None,
                updated_at: None,
            }],
            ..Workspace::default()
        };
        let pasted = |what: Moving| what.pasted(Path::new("/work/repo"), &workspace);
        let words = |words: &[&str]| Some(words.iter().map(|w| w.to_string()).collect::<Vec<_>>());
        let files = |worktree| {
            Moving::Files(Changes {
                from: None,
                worktree,
                files: Arc::from([]),
            })
        };
        let linked = |name: &str| Moving::Worktree {
            name: name.into(),
            order: Arc::from([]),
        };
        assert_eq!(
            pasted(Moving::Commits(Arc::from([kept]))),
            words(&["vtlrvnlmsxrmrsnrkvqlwrzqokpsoxzk"]),
            "a commit by its change ID"
        );
        assert_eq!(
            pasted(Moving::Commits(Arc::from([kept, gone]))),
            None,
            "nothing once a commit it holds is gone, rather than a stale id"
        );
        assert_eq!(
            pasted(Moving::Branch(feat)),
            words(&["feat"]),
            "a branch by its short name"
        );
        assert_eq!(
            pasted(linked("agent")),
            words(&["/work/agent"]),
            "a linked worktree by its folder"
        );
        assert_eq!(
            pasted(linked("removed")),
            None,
            "nothing for a worktree that's gone"
        );
        assert_eq!(
            pasted(files(Checkout::Main)),
            words(&["/work/repo"]),
            "the main worktree's row drags its files, which paste its folder"
        );
        assert_eq!(
            pasted(files(Checkout::Linked("agent".into()))),
            words(&["/work/agent"]),
            "and a linked worktree's its own"
        );
    }
}
