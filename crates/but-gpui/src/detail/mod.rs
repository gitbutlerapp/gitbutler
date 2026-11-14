//! The right side: whatever is selected in the navigator, with its files and diff.
//! Uncommitted changes are for reviewing what an agent did, and committing it;
//! a commit shows its message.

mod blame;
mod branches;
mod brief;
pub(crate) use brief::{
    ChangesBeside, GoBack, NextBlock, NextBrief, PreviousBrief, SendNotes, WriteNote,
};
mod composer;
mod conversation;
mod diff_view;
mod edit_lines;
mod file_list;
mod file_tree;
mod find;
mod graph;
mod key_panes;
pub use branches::{
    ApplyBranch, EditDescription, NewWorktreeFromBranch, ShowBranchesWithoutCommits,
    ShowOnlyLocalBranches, ShowRemoteBranches,
};
pub use file_list::{MarkResolved, SplitIntoNewCommit, UncommitChanges};
pub use find::FindInDiff;
pub use history::RestoreOperation;
mod history;
mod lines;
mod machine;
mod page;
mod panes;
mod pull_request;
mod rename;
mod resolve;
mod review_edit;
mod slots;
mod upstream;
mod worktree;

pub use diff_view::DiffPane;
#[cfg(feature = "dev-script")]
pub(crate) use graph::GraphWindow;
pub(crate) use key_panes::{KeyMove, KeyPane};
pub use page::KeptPages;
pub(crate) use page::{PageTab, has_tabs, shows_its_diff};
pub use resolve::{ResolveConflicts, oldest_conflict};

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, ListAlignment,
    ListOffset, ListState, Pixels, Point, RenderImage, ScrollHandle, SharedString, Subscription,
    Task, Window, actions, div, prelude::*,
};

use crate::app::{
    AddProject, AppHandle, CopyAbsolutePath, CopyRelativePath, NextProjectPage, PreviousProjectPage,
};
use crate::backend::edit::Editing;
use crate::backend::{
    self, Checkout, CommitId, DiffLine, DiffSource, FileChange, FileDiff, FullName, HunkKey,
    LineKind, Selected,
};
use crate::clipboard;
use crate::clock;
use crate::edits::Discard;
use crate::overlays::MenuItem;
use crate::palette::score;
use crate::state::{
    Area, CodePlace, FileSort, FileView, Pane, Part, Prefs, ProjectId, ProjectTab, RowKey, Section,
    Selection, Side, Store, StoreEvent, Tile,
};
use crate::theme::Theme;
use crate::ui::change_marks::{Change, ChangeMarks};
use crate::ui::diff::{self, DiffRow};
use crate::ui::fold::{FOLD, Folds, Slide, edge, inert};
use crate::ui::illustration::{Illustration, illustration};
use crate::ui::message_editor::{COMMIT_PLACEHOLDERS, MessageEditor, MessageSize};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, TextPoint, button, commit_title, dp, empty_state, fixed, h_flex, placed,
    resize_handle, resize_pane, v_flex,
};

actions!(
    diff,
    [
        NextFile,
        PreviousFile,
        PickNextFile,
        PickPreviousFile,
        PickAllFiles,
        FirstFile,
        LastFile,
        NextHunk,
        PreviousHunk,
        FoldFile,
        UnfoldFile,
        ToggleFileFold,
        PageDown,
        PageUp,
        HalfPageDown,
        HalfPageUp,
        ScrollDown,
        ScrollUp,
        SwitchTab,
        PreviousTab,
        ShowBrief,
        ShowBranchesWithBriefs,
        PickCommitTarget,
        ToggleWrap,
        TogglePlainLines,
        ToggleSplit,
        ToggleFiles,
        FilterFiles,
        ToggleCheck,
        CheckAll,
        UncheckAll,
        WriteMessage,
        /// Edit the lines of the uncommitted file the cursor's in, in place.
        EditLines,
        /// Save the file being edited now, rather than once typing rests.
        SaveEdits,
        /// Keep what's typed in the file being edited, and stop editing it.
        FinishEdit,
    ]
);

/// How far Space and Page Down scroll the diff, in its heights, keeping a few lines in view.
pub const PAGE: f32 = 0.85;

/// How many code lines ⇧J and ⇧K scroll the diff.
const SCROLL_LINES: f32 = 3.;

/// Where the keyboard is in the diff.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    file: usize,
    at: Stop,
}

/// What of its file the cursor is on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The messages, a commit's or a branch's commits', above every file, where they list in
    /// the diff: in no file, so the cursor's `file` says nothing.
    Message,
    /// The file's header.
    #[default]
    File,
    /// The box above hunk `.0`, saying what's hidden before it: the file's, as the unchanged
    /// lines it stands for are, not the hunk's, though checking and copying take the hunk.
    Gap(usize),
    /// Hunk `.0`'s lines, from its first change to its last: what the hunk keys act on.
    Hunk(usize),
}

impl Cursor {
    fn on_file(file: usize) -> Self {
        Cursor {
            file,
            at: Stop::File,
        }
    }

    fn on_hunk(file: usize, hunk: usize) -> Self {
        Cursor {
            file,
            at: Stop::Hunk(hunk),
        }
    }

    fn on_message() -> Self {
        Cursor {
            file: 0,
            at: Stop::Message,
        }
    }

    /// The entry the cursor is in: the message, or its file.
    fn entry(self) -> Entry {
        match self.at {
            Stop::Message => Entry::Message,
            Stop::File | Stop::Gap(_) | Stop::Hunk(_) => Entry::File(self.file),
        }
    }

    /// The file the cursor is in, none on the message.
    fn in_file(self) -> Option<usize> {
        match self.entry() {
            Entry::File(file) => Some(file),
            Entry::Message => None,
        }
    }
}

/// One of what the diff lists: the messages, a commit's or a branch's commits', first where
/// they list in place of the page's header, then each file. In that order, as the file
/// list has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Entry {
    Message,
    File(usize),
}

/// Where the file list's keys stop: an entry, or in a tree a folder, by its path.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ListStop {
    Entry(Entry),
    Folder(String),
}

/// Whose messages the diff lists first, ahead of its files.
enum Messages {
    Commit(ProjectId, CommitId),
    Branch(ProjectId, Option<FullName>),
    /// The commits only `ref_name`'s remote has.
    Upstream(ProjectId, FullName),
    /// One commit only `ref_name`'s remote has.
    UpstreamCommit(ProjectId, FullName, CommitId),
    /// The commit a list page shows over its diff: the project page's, or the one picked of
    /// the opened branch on the Branches page.
    TargetCommit(ProjectId),
    /// The branch opened on the Branches page, with no commit of it picked.
    OtherBranch(ProjectId),
}

/// What of the detail y copied, which blinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Copied {
    /// The text selected in the diff.
    Text,
    /// What the cursor's on: its file's row in the file list, or in the diff its hunk or file,
    /// whichever has the keys.
    Cursor,
}

impl Stop {
    /// The hunk the hunk keys act on, not one with the cursor on the box above it.
    fn hunk(self) -> Option<usize> {
        match self {
            Stop::Hunk(hunk) => Some(hunk),
            Stop::Message | Stop::File | Stop::Gap(_) => None,
        }
    }

    /// The hunk the cursor is on or just above.
    fn near(self) -> Option<usize> {
        match self {
            Stop::Gap(hunk) | Stop::Hunk(hunk) => Some(hunk),
            Stop::Message | Stop::File => None,
        }
    }
}

/// How quickly the diff glides to where the keys moved its cursor: in each such time it goes
/// about two thirds of the way it has left, so it sets out quickly and eases in.
const GLIDE: Duration = Duration::from_millis(40);

/// The diff's way to where the keys moved its cursor, in rows, as gpui knows the heights only
/// of rows it has drawn: to place `to`, a row and how far down it by its height, a share of
/// the way each frame by how long since the `last`. One that `reveal`s what the cursor's on
/// aims afresh as it goes: see [`Detail::follow_glide`].
struct Glide {
    to: f32,
    last: Instant,
    reveal: bool,
}

/// The rows a file folding or unfolding moves: how tall each is, and how far the diff shows
/// below its header, past which they go or come at once. A row not drawn yet, as one of a file
/// opening, has only a guess, a line tall even where it wraps, so each drawn while it moves
/// keeps how tall it was for the next frame.
struct FoldRows {
    heights: Rc<[Cell<Pixels>]>,
    room: Option<Pixels>,
    /// How many rows under the header go at once as it starts, as they're out of sight: above
    /// the diff's top, or under the header stuck there.
    left_out: usize,
}

/// What folds on the pages beside the diff: a folder of a tree of files, a stack's branches or
/// a branch's commits on the Branches page, the commits a merge on the target brought in, the
/// People card's rest, or on a pull request, a thread or the checks that passed.
#[derive(Clone, PartialEq, Eq, Hash)]
enum PageFold {
    Dir(String),
    Stack(FullName),
    Branch(FullName),
    Merge(CommitId),
    People,
    Thread(String),
    Checks,
}

#[derive(Debug, Clone, PartialEq)]
struct DiffKey {
    project: ProjectId,
    source: DiffSource,
}

impl DiffKey {
    /// The uncommitted files this diff follows, as the store has them: the project's
    /// own checkout's, or a linked worktree's. `None` for a diff of commits.
    fn uncommitted<'a>(&self, store: &'a Store) -> Option<&'a [FileChange]> {
        store
            .workspace(&self.project)?
            .uncommitted_in(&self.checkout()?)
    }

    /// The checkout whose uncommitted files this diff is. `None` for a diff of commits.
    fn checkout(&self) -> Option<Checkout> {
        match &self.source {
            DiffSource::Worktree => Some(Checkout::Main),
            DiffSource::Linked(name) => Some(Checkout::Linked(name.clone())),
            _ => None,
        }
    }

    /// Whether a pending operation takes a file, by path, out of this diff: out of the
    /// uncommitted files it is, or out of its commit.
    fn leaving<'a>(&'a self, store: &'a Store) -> impl Fn(&str) -> bool + 'a {
        let commit = match self.source {
            DiffSource::Commit(id) => store.key_of(&self.project, id),
            _ => None,
        };
        move |path| {
            let project = &self.project;
            (self.checkout()).is_some_and(|checkout| store.hides_file(project, &checkout, path))
                || (commit.as_ref())
                    .is_some_and(|commit| store.hides_commit_file(project, commit, path))
        }
    }

    /// The checkout this diff's files open from: a commit's from the main one.
    fn files_checkout(&self) -> Checkout {
        self.checkout().unwrap_or_default()
    }
}

/// How long the pointer rests on the files button before it lists the files.
const FILES_POPOVER_DELAY: Duration = Duration::from_millis(100);

/// How long a diff loads before the pane says it's loading.
const SHOW_LOADING: Duration = Duration::from_millis(150);

/// How tall the uncommitted changes' commit footer is, and the composer's row of buttons.
pub const FOOTER_HEIGHT: f32 = 33.;

/// How wide Commit… and the composer's Commit are at least, so the one covers the other.
const COMMIT_BUTTON_WIDTH: f32 = 72.;

struct LoadedDiff {
    key: DiffKey,
    /// In the file list's order: as picked, and in a tree, folder by folder.
    files: Arc<Vec<FileDiff>>,
    /// The files are in the tree's order, and the order they're sorted in.
    tree: bool,
    sort: FileSort,
    rows: Vec<DiffRow>,
    /// The files the rows show with every file shown, as [`Detail::shown_files`] lists them.
    shown: Vec<usize>,
    /// The one entry the rows show, with the single file diff on: a file, or the message.
    alone: Option<Entry>,
    /// Its changed lines beside the scrollbar, while it shows one file.
    marks: Option<Rc<ChangeMarks>>,
    /// What was selected when it loaded.
    selection: Option<Selection>,
}

/// A place to open the diff on its way at.
struct PendingFile {
    place: CodePlace,
    /// What the link that asked for it selected, whose diff says so if it lacks the place.
    linked: Option<Selection>,
}

impl From<CodePlace> for PendingFile {
    fn from(place: CodePlace) -> Self {
        PendingFile {
            place,
            linked: None,
        }
    }
}

/// A file's new side as diff lines, read for the patch with this fingerprint, and the
/// widest of them in columns.
type FileLines = (u64, Arc<Vec<DiffLine>>, u32);

/// What [`FileLines`] hold in place of a patch's fingerprint for a file's lines as its edit
/// left them: shown whole as they are until they're read again, so ending the edit moves
/// nothing.
const AS_TYPED: u64 = u64::MAX;

/// The files checked for the next commit changed.
pub struct ChecksChanged;

pub struct Detail {
    store: Entity<Store>,
    /// For what the window owns: focus, menus, and prompts that ask first.
    app: AppHandle,
    focus: FocusHandle,
    /// The file list's own, where ↑ and ↓ step through the files.
    files_focus: FocusHandle,
    /// A list page's list, the project's, Branches' or Operations history's, which a click
    /// on one of its rows focuses.
    side_focus: FocusHandle,
    /// The Changes and pull request tabs', which ↑ from the top of the files under them
    /// reaches, and where ← and → switch tabs.
    tabs_focus: FocusHandle,
    /// The tabs to a project's pages, which ↑ from the top of a list page's list reaches, and
    /// where ← and → go to the page on that side.
    project_tabs_focus: FocusHandle,
    /// A People card's, which a click on it takes and a click anywhere else gives back to the
    /// list it's under: its person picked shows as a focused selection only then.
    people_focus: FocusHandle,
    people_scroll: ScrollHandle,
    /// Where the diff is gliding to, a step each frame, after the keys moved its cursor.
    glide: Option<Glide>,
    /// The entry the diff was scrolled to past the cursor, which the file list marks as the
    /// current one: see [`Detail::current_entry`].
    reading: Option<Entry>,
    diff: Option<LoadedDiff>,
    diff_error: Option<SharedString>,
    loading: Option<DiffKey>,
    /// The worktree changed while its diff was loading; load again when done.
    reload_queued: bool,
    /// When the selection last changed, to tell a held arrow key from a single step.
    selected_at: Option<Instant>,
    /// Diffs shown lately, oldest first. Commits and ranges can't change, so going back to
    /// one shows it at once; the worktree reuses each unchanged file's highlighting.
    recent: Vec<(DiffKey, Arc<Vec<FileDiff>>)>,
    _load: Task<()>,
    /// When the selection last moved to a diff that isn't loaded yet; the previous one
    /// stays on screen until it is, unless that takes long enough to say so.
    waiting_since: Option<Instant>,
    _waiting: Task<()>,
    list: ListState,
    /// The width the diff's undrawn rows were last given a height at, which gpui forgets
    /// whenever the list's width changes.
    estimated_width: Rc<Cell<Option<Pixels>>>,
    /// Where the uncommitted file list and a commit's or branch's file list are scrolled.
    uncommitted_files: ListState,
    diff_files: ListState,
    /// The cursor's file the file list last scrolled to show, so it follows the cursor's
    /// moves but not its own scrolling.
    listed_cursor: std::cell::RefCell<Option<String>>,
    /// The rows of the summary the diff's pane shows for the folder the file list's keys are
    /// on, and which folder they were last drawn for, so another starts at its top.
    folder_rows: ListState,
    summarized: std::cell::RefCell<Option<String>>,
    /// The file list's row holding the cursor's file as last drawn, and the list: where ⇧F10
    /// opens its menu.
    listed_cursor_row: std::cell::RefCell<Option<(ListState, usize)>>,
    message: Entity<MessageEditor>,
    /// The branch (full ref) new commits go to; `None` creates a branch.
    commit_target: Option<FullName>,
    /// What the new branch the commit being placed goes in is named, as typed when it was
    /// picked, made only with the commit.
    new_branch_name: Option<String>,
    checks: composer::Checks,
    /// The checks of the other checkouts looked at, by project and checkout, kept for coming
    /// back to them.
    other_checks: HashMap<(Option<ProjectId>, Checkout), composer::Checks>,
    /// The `commit_target` picked in the other projects looked at.
    other_targets: HashMap<ProjectId, FullName>,
    /// Narrows the file list and the diff to the paths it matches, while `filtering`.
    file_filter: Entity<TextInput>,
    filtering: bool,
    /// Where the diff goes once the files shown whole as it was left are read, as their unchanged
    /// lines, which it may have been on, are only rows then.
    reopening: Option<page::Place>,
    /// Files shown whole, by path, with their lines once read.
    whole: HashMap<String, Option<FileLines>>,
    /// The grip under the pointer, by its file, hunk and line, and what it takes, by hunk
    /// and line index in order, outlined meanwhile.
    grip_over: Option<(find::LineId, Vec<(usize, usize)>)>,
    /// The button is down on that grip, which keeps it, and its drag, until the drag starts
    /// or the button comes up.
    grip_held: bool,
    /// A press on a line's number began what the pointer's dragging over the numbers, which
    /// picks the run of lines it passes.
    picking_lines: bool,
    /// Files whose lines are being read, one read each at a time.
    whole_reading: HashSet<String>,
    /// The file, by path, and hunk whose header asked for the whole file, and how many rows
    /// down the diff's scroll it was then, where it stays once the file's lines are in.
    whole_anchor: Option<(String, usize, usize)>,
    /// What signed each commit previewed, like "an SSH key", or `None` when nothing did or
    /// it's still being read. A commit never changes, so neither does its signature.
    signers: HashMap<backend::CommitId, Option<&'static str>>,
    /// How far each file's lines are scrolled sideways, by path: the old side's and the new
    /// side's, which a unified diff scrolls.
    sideways: HashMap<String, [ScrollHandle; 2]>,
    /// The commit message being edited, with the subscription that saves it. Looking at
    /// another commit keeps it, to pick up again on coming back.
    rewording: Option<(composer::Reworded, Entity<MessageEditor>, Subscription)>,
    /// The branch whose name is being edited in its header.
    renaming: Option<rename::Renaming>,
    /// The pull request whose title and description are being edited on its tab, in the page
    /// shown. Its page keeps what's written as it goes.
    review_edit: Option<review_edit::ReviewEdit>,
    /// The project the checks and commit target above belong to.
    checks_project: Option<ProjectId>,
    /// The checkout whose files the checks are of.
    checks_worktree: Checkout,
    /// The commit message box is open; until then there are no checkboxes.
    composing: bool,
    /// The branch the composer showed the commit going to, which mustn't change unseen.
    composing_target: Option<FullName>,
    /// Where the focus was when writing a commit message started, to go back to after.
    composer_back: Option<FocusHandle>,
    cursor: Cursor,
    /// How many times what y copied here has blinked, and what it last was and when.
    blinked: (usize, Option<(Copied, Instant)>),
    now: worktree::Now,
    /// The place to put the cursor on once the diff loading now lands, as when following the
    /// file being read into the commit an agent made of it.
    pending_file: Option<PendingFile>,
    /// The page a click on a commit of a branch that left a checkout picked, and that commit,
    /// whose merge or branch it opens once it has read its list.
    opening: Option<(ProjectTab, crate::state::SeenCommit)>,
    /// The pull request tab's column: its title, description and conversation.
    review_list: ListState,
    /// The project whose Branches page lists only the branches a brief is known of.
    branches_with_brief: Option<ProjectId>,
    brief: brief::BriefView,
    /// The pull request's side panel, which scrolls apart from its description beside it.
    review_side_scroll: ScrollHandle,
    /// How wide the pull request tab was drawn, for whether its side panel fits beside it: on
    /// a branch's page, then beside a list page's list.
    review_width: [Rc<Cell<f32>>; 2],
    /// How tall the selected target commit's message is drawn over its diff, and how wide
    /// the space beside the project page's list is, measured as they're drawn.
    target_message_height: Rc<Cell<f32>>,
    beside_width: Rc<Cell<f32>>,
    /// The Shipped pane's, which a click on it takes, so its area picked shows solid then,
    /// as the People card's does.
    shipped_focus: FocusHandle,
    /// How tall a commit's header across the page was drawn, which the diff starts below.
    header_height: Rc<Cell<f32>>,
    /// How tall the header across a project's pages was drawn, with their tabs.
    project_header_height: Rc<Cell<f32>>,
    /// What's opening or closing on the pages beside the diff.
    page_folds: Folds<PageFold>,
    /// Text selected in the diff, from where the drag started to where it is now.
    text_selection: Option<(TextPoint, TextPoint)>,
    /// The left button went down on a line and hasn't come up yet.
    selecting: bool,
    /// Where a drag that selects text went past the edge of the diff, brought back inside it,
    /// for the line there to take the selection's end.
    follow: Option<Point<Pixels>>,
    /// Text selected in the pull request's description or one of its comments: which, by the
    /// id it's drawn with, and from where the drag started to where it is now.
    note_selection: Option<(String, (TextPoint, TextPoint))>,
    /// Files folding or unfolding now, and the rows each moves, by path.
    file_folds: Folds<String>,
    fold_rows: HashMap<String, FoldRows>,
    /// The commit message being written with AI, streaming into the box, and what the box
    /// held before, which a write cut short puts back.
    writing: Option<(Task<()>, String)>,
    /// What the message box holds in place of the next commit's message while a commit's being
    /// amended.
    amend_message: Option<composer::AmendMessage>,
    /// The find bar over the diff, while it's open, and its field.
    find: Option<find::Find>,
    find_input: Entity<TextInput>,
    /// What's picked for the shown commit's conflicts, until it's applied.
    resolving: resolve::Resolving,
    /// The uncommitted file being edited in the diff, until it's saved or discarded, and
    /// the read of its sides to start from.
    file_edit: Option<edit_lines::FileEdit>,
    /// What was typed in edits that ended while a write of them was under way, each written once
    /// the write of its `read` lands.
    write_after: Vec<(ProjectId, crate::backend::edit_lines::WriteFile)>,
    _read_sides: Task<()>,
    /// Waiting for a checkout's changes to show, to open a file there.
    _open_when_shown: Option<Subscription>,
    /// Where a press on a line's text began that edits there once it's let go: a double click,
    /// or in a file being edited, a click let go where it began. With whether it was double.
    edit_press: Option<(TextPoint, bool)>,
    /// Each project page's target history, by project, kept so going back to one shows it
    /// at once, while it's read again.
    upstream: HashMap<ProjectId, upstream::Upstream>,
    upstream_scroll: ListState,
    _upstream_load: Task<()>,
    /// Each Branches page's branches, the same way, and the filter over them.
    branches: HashMap<ProjectId, branches::Branches>,
    branch_filter: Entity<TextInput>,
    /// The project page's, over its history.
    target_filter: Entity<TextInput>,
    branches_scroll: ListState,
    _branches_load: Task<()>,
    /// What's fetched of other machines' branches; see `detail/machine.rs`.
    mesh: machine::MeshFetches,
    _branch_commits_load: Task<()>,
    /// Each History page's operations, the same way, and its filter.
    history: HashMap<ProjectId, history::History>,
    history_filter: Entity<TextInput>,
    history_scroll: ListState,
    _history_load: Task<()>,
    /// What this shows, which everything else here is of. A selection reaches it as the update
    /// making it ends, or sooner with [`Detail::follow_selection`]: until then this all still
    /// agrees on the page before.
    shows: Option<Selection>,
    /// The state of the page shown, and whose it is, for keeping it as the page goes.
    page: page::PageState,
    page_key: Option<page::PageKey>,
    /// Where pages go while nothing draws them.
    kept: Entity<KeptPages>,
    /// How the file list beside the diff slides, and the files beside a list page's list.
    files_slide: Slide,
    beside_slide: Slide,
    /// How a commit's changes slide in beside the graph, and the page's width, which the graph's
    /// column narrows from and widens back to as they do.
    graph_slide: Slide,
    graph_page: f32,
    /// Draws the files once they've slid where they're going, as the diff slides over them
    /// without drawing the detail again.
    _slid: Task<()>,
    /// Since when the pointer is on the files button over the diff, and whether it's on the
    /// popover of the files the button opens, [`FILES_POPOVER_DELAY`] on, while they aren't
    /// beside the diff.
    files_hover: (Option<Instant>, bool),
    /// Whether that popover was drawn last: back on the button from it, it stays open.
    files_popover_shown: Cell<bool>,
    _files_popover_delay: Task<()>,
    /// How much of the navigator shows beside the detail, from 0 to 1, as the app last drew
    /// it. Its sliding changes the detail's width, so the detail draws again as it changes.
    pub navigator_shown: Cell<f32>,
    /// How much of the navigator's tabs show across the window's top, from 0 to 1, as they slide.
    pub tabs_shown: Cell<f32>,
    /// The window's width to lay out for, in unscaled pixels, while the window narrows to the
    /// navigator or widens back as the detail slides, so it isn't laid out again at every
    /// width; otherwise the window's own.
    pub window_width: Cell<Option<f32>>,
    /// The Graph page, which draws itself as it pans and zooms.
    graph: Entity<graph::GraphView>,
    /// For a pinned tile's page, the row of the tiling it's in and the row it shows.
    pinned: Option<(RowKey, RowKey)>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ChecksChanged> for Detail {}

impl Detail {
    /// No diff loading or queued, for the dev script's timings.
    #[cfg(feature = "dev-script")]
    pub fn is_settled(&self) -> bool {
        self.loading.is_none() && !self.reload_queued
    }

    /// The detail of what the navigator selects, or with `pinned`, of that row, for its tile
    /// in the tiling of the other, pinned beside the selected page.
    pub fn new(
        store: Entity<Store>,
        app: AppHandle,
        kept: Entity<KeptPages>,
        pinned: Option<(RowKey, RowKey)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let shows = match &pinned {
            Some((_, row)) => store.read(cx).selection_of(row),
            None => store.read(cx).selection.clone(),
        };
        let message =
            cx.new(|cx| MessageEditor::new(MessageSize::Compact, COMMIT_PLACEHOLDERS, cx));
        let branch_filter = cx.new(|cx| TextInput::new("Filter branches", false, cx));
        let target_filter = cx.new(|cx| TextInput::new("Filter commits", false, cx));
        let history_filter = cx.new(|cx| TextInput::new("Filter operations", false, cx));
        let file_filter = cx.new(|cx| TextInput::new("Filter files", false, cx));
        let find_input = cx.new(|cx| TextInput::new("Find", false, cx));
        let tab = page::tab_to_show(&kept, store.read(cx), pinned.as_ref(), shows.as_ref(), cx);
        let page_key = page::page_key(store.read(cx), pinned.as_ref(), shows.as_ref(), tab);
        let page = page::open_page(&kept, page_key.as_ref(), shows.as_ref(), cx);
        let graph = cx.new(|cx| graph::GraphView::new(store.clone(), cx));
        let subscriptions = vec![
            cx.subscribe_in(&graph, window, |this, _, event, window, cx| {
                this.graph_event(event, window, cx)
            }),
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                // A pinned page shows what it was pinned to, whatever's selected.
                StoreEvent::SelectionChanged
                | StoreEvent::Reopened
                | StoreEvent::TabAsked(_)
                | StoreEvent::FileAsked(..)
                    if this.pinned.is_some() => {}
                StoreEvent::SelectionChanged => this.follow_selection(window, cx),
                // Picked again, as by its card or the crumb naming it, a project's page goes back
                // to how it opens.
                StoreEvent::Reopened => match (this.selection(), this.project_tab()) {
                    (Some(Selection::Project(project)), ProjectTab::Overview) => {
                        this.pick_opening(&project, cx)
                    }
                    (Some(Selection::Project(_)), _) => {
                        this.show_tab(ProjectTab::Overview, window, cx)
                    }
                    _ => {}
                },
                StoreEvent::TabAsked(tab) => this.tab_asked(*tab, window, cx),
                StoreEvent::FileAsked(selection, place) => {
                    this.pending_file = Some(PendingFile {
                        place: place.clone(),
                        linked: Some(selection.clone()),
                    });
                    // Shown already, the selection doesn't change, so no diff lands to open it.
                    if this.shown_diff().and_then(|diff| diff.selection.as_ref()) == Some(selection)
                    {
                        this.open_pending_file(cx);
                    }
                }
                StoreEvent::Loaded(project) => {
                    if this.find_pinned(cx) {
                        this.shown_changed(window, cx);
                    }
                    if this.selected_project().as_ref() == Some(project) {
                        this.follow_into_commit(project, cx);
                        this.reload_page(false, cx);
                        // A reload that didn't touch the files shown, like the forge poll's,
                        // shouldn't re-highlight them. Files of another checkout, left on
                        // screen until this project loaded, aren't the ones shown.
                        let key = this.key_for_selection(cx);
                        let unchanged = (this.worktree_diff())
                            .filter(|diff| key.as_ref() == Some(&diff.key))
                            .and_then(|diff| diff.key.uncommitted(this.store.read(cx)))
                            .is_some_and(|files| this.shows_worktree(files));
                        if !unchanged {
                            this.refresh_diff(false, cx);
                        }
                    }
                }
                // What's picked to commit to follows a branch an agent renames.
                StoreEvent::Renamed(project, from, to) => {
                    this.branch_renamed(project, from.as_ref(), to, cx);
                    this.page_renamed(project, from.as_ref(), to, window, cx)
                }
                StoreEvent::MeshRead(project) => {
                    if let Some(Selection::Mesh(p, ..)) = this.selection()
                        && p == *project
                    {
                        this.load_machines(&p, false, cx);
                        this.refresh_diff(false, cx);
                        cx.notify();
                    }
                }
                StoreEvent::Pending(project) => {
                    if this.selected_project().as_ref() == Some(project) {
                        this.rebuild_rows(cx);
                        this.skip_taken(cx);
                    }
                }
                // The page draws in the home tile's room.
                StoreEvent::TilingChanged => cx.notify(),
                StoreEvent::CodeAsked(row, place) => this.code_asked(row, place, cx),
                // The app view asks for it, and gives a terminal opened the keys.
                StoreEvent::AuthorMissing(_)
                | StoreEvent::RepositoriesInside(..)
                | StoreEvent::AiNotSetUp { .. }
                | StoreEvent::TerminalOpened
                | StoreEvent::TilePinned { .. }
                | StoreEvent::TileClosed { .. }
                | StoreEvent::TilingMoved { .. }
                | StoreEvent::MoveToward(_)
                | StoreEvent::CommitBlocked
                | StoreEvent::UpdateConflicts { .. }
                | StoreEvent::LinkOpened { .. }
                | StoreEvent::RowDragEnded => {}
            }),
            cx.subscribe_in(
                &file_filter,
                window,
                |this, _, event, window, cx| match event {
                    InputEvent::Changed => {
                        this.rebuild_rows(cx);
                        // So the diff's keys act on a file in view: the best match, which the
                        // list starts with, scrolled to, as the diff can have others above it.
                        let shown = this.shown_files();
                        let first =
                            (this.listed_files(cx).first().copied()).or(shown.first().copied());
                        // Off the message too, as what's filtered is the files.
                        if let Some(file) = first
                            && (this.cursor.at == Stop::Message
                                || !shown.contains(&this.cursor.file))
                        {
                            this.cursor = Cursor::on_file(file);
                            if let Some(row) = this.row_of(|row| *row == DiffRow::File(file)) {
                                this.scroll_to_row(row, false);
                            }
                        }
                    }
                    // On to the list as filtered, to step through it, as a side list's filter
                    // goes on to its list. The field heads the list, so the list is drawn.
                    InputEvent::Submit => window.focus(&this.files_focus, cx),
                    InputEvent::Cancel => this.close_filter(window, cx),
                },
            ),
            cx.subscribe_in(
                &find_input,
                window,
                |this, _, event, window, cx| match event {
                    InputEvent::Changed => this.run_find(true, cx),
                    InputEvent::Submit => this.step_find(1, cx),
                    InputEvent::Cancel => this.close_find(window, cx),
                },
            ),
            cx.subscribe_in(&message, window, |this, _, event, window, cx| match event {
                InputEvent::Submit if this.amending(cx).is_some() => this.amend(window, cx),
                InputEvent::Submit => this.commit(window, cx),
                // While the navigator places the commit, or what's amended, back to that.
                InputEvent::Cancel
                    if this.committing(cx).is_some() || this.amending(cx).is_some() =>
                {
                    this.back_to_placing(window, cx)
                }
                InputEvent::Cancel => this.leave_composer(window, cx),
                // The input draws itself; nothing here shows the message, and notifying
                // would redraw the diff on every keystroke.
                InputEvent::Changed => {
                    let message = this.message.read(cx).text(cx);
                    // What's being amended has a message of its own, not the next commit's.
                    if let Some(project) =
                        (this.checks_project.as_ref()).filter(|_| this.amend_message.is_none())
                    {
                        let key = (project.clone(), this.checks_worktree.clone());
                        (this.store).update(cx, |store, _| match message.trim().is_empty() {
                            true => store.drafts.remove(&key),
                            false => store.drafts.insert(key, message),
                        });
                    }
                }
            }),
            list_filter(&history_filter, Self::select_shown_operation, window, cx),
            list_filter(&branch_filter, Self::select_shown_branch, window, cx),
            list_filter(&target_filter, Self::show_newest_kept, window, cx),
            // What the side panel shows comes from the store, and the diff's files go in the
            // file list's order, as the keys step through them.
            cx.observe(&store, |this, store, cx| {
                this.show_amended_message(cx);
                // Diffs of a project no longer watched go with it, unless on screen.
                let shown = this.diff.as_ref().map(|diff| diff.key.clone());
                let gone: Vec<_> = this
                    .recent
                    .extract_if(.., |(key, _)| {
                        let watched =
                            (store.read(cx).project(&key.project)).is_some_and(|p| p.expanded);
                        !watched && shown.as_ref() != Some(key)
                    })
                    .collect();
                this.forget(gone, cx);
                let prefs = &store.read(cx).prefs;
                let order = (prefs.file_view == FileView::Tree, prefs.file_sort);
                if let Some(diff) =
                    (this.diff.as_ref()).filter(|diff| (diff.tree, diff.sort) != order)
                {
                    let key = diff.key.clone();
                    let files = this
                        .recent_files(&key)
                        .unwrap_or_else(|| diff.files.clone());
                    this.show_diff(key, files, cx);
                }
                // Picking commits, or letting them go, changes what the diff is of while the
                // selection stays where it was.
                if matches!(this.selection(), Some(Selection::Commit(..)))
                    && let (Some(diff), Some(key)) = (&this.diff, this.key_for_selection(cx))
                    && diff.key != key
                    && this.loading.as_ref() != Some(&key)
                    && matches!(
                        (&diff.key.source, &key.source),
                        (DiffSource::Range { .. } | DiffSource::Commits(_), _)
                            | (_, DiffSource::Range { .. } | DiffSource::Commits(_))
                    )
                {
                    this.refresh_diff(false, cx);
                }
                this.ask_forge(cx);
                this.ask_briefs(cx);
                cx.notify()
            }),
        ];
        let mut detail = Self {
            store,
            app,
            focus: cx.focus_handle(),
            files_focus: cx.focus_handle(),
            side_focus: cx.focus_handle(),
            tabs_focus: cx.focus_handle(),
            project_tabs_focus: cx.focus_handle(),
            people_focus: cx.focus_handle(),
            people_scroll: ScrollHandle::new(),
            glide: None,
            reading: None,
            diff: None,
            diff_error: None,
            loading: None,
            reload_queued: false,
            selected_at: None,
            recent: Vec::new(),
            _load: Task::ready(()),
            waiting_since: None,
            _waiting: Task::ready(()),
            list: ListState::new(0, ListAlignment::Top, fixed(40.)),
            estimated_width: Rc::default(),
            uncommitted_files: panes::side_list_state(),
            diff_files: panes::side_list_state(),
            listed_cursor: Default::default(),
            folder_rows: panes::side_list_state(),
            summarized: Default::default(),
            listed_cursor_row: Default::default(),
            message,
            commit_target: None,
            new_branch_name: None,
            checks: Default::default(),
            other_checks: HashMap::new(),
            other_targets: HashMap::new(),
            file_filter,
            filtering: false,
            whole: HashMap::new(),
            whole_reading: HashSet::new(),
            whole_anchor: None,
            reopening: None,
            grip_over: None,
            grip_held: false,
            picking_lines: false,
            signers: HashMap::new(),
            sideways: HashMap::new(),
            rewording: None,
            renaming: None,
            review_edit: None,
            checks_project: None,
            checks_worktree: Checkout::Main,
            composing: false,
            composing_target: None,
            composer_back: None,
            cursor: Cursor::default(),
            blinked: (0, None),
            now: worktree::Now::default(),
            pending_file: None,
            opening: None,
            review_list: ListState::new(0, ListAlignment::Top, fixed(200.)),
            brief: brief::BriefView::new(),
            branches_with_brief: None,
            review_side_scroll: ScrollHandle::new(),
            review_width: [(); 2].map(|_| Rc::new(Cell::new(pull_request::TWO_COLUMNS))),
            target_message_height: Rc::new(Cell::new(0.)),
            beside_width: Rc::new(Cell::new(0.)),
            shipped_focus: cx.focus_handle(),
            header_height: Rc::new(Cell::new(panes::COMMIT_HEADER_HEIGHT)),
            project_header_height: Rc::new(Cell::new(0.)),
            page_folds: Folds::default(),
            text_selection: None,
            selecting: false,
            follow: None,
            note_selection: None,
            file_folds: Folds::default(),
            fold_rows: HashMap::new(),
            writing: None,
            amend_message: None,
            find: None,
            find_input,
            resolving: Default::default(),
            file_edit: None,
            write_after: Vec::new(),
            _read_sides: Task::ready(()),
            _open_when_shown: None,
            edit_press: None,
            upstream: HashMap::new(),
            upstream_scroll: panes::side_list_state(),
            _upstream_load: Task::ready(()),
            branches: HashMap::new(),
            branch_filter,
            target_filter,
            branches_scroll: panes::side_list_state(),
            _branches_load: Task::ready(()),
            mesh: machine::MeshFetches::default(),
            _branch_commits_load: Task::ready(()),
            history: HashMap::new(),
            history_filter,
            history_scroll: panes::side_list_state(),
            _history_load: Task::ready(()),
            shows,
            page,
            page_key,
            kept,
            files_slide: Slide::default(),
            graph_slide: Slide::default(),
            graph_page: 0.,
            beside_slide: Slide::default(),
            _slid: Task::ready(()),
            files_hover: (None, false),
            files_popover_shown: Cell::new(false),
            _files_popover_delay: Task::ready(()),
            navigator_shown: Cell::new(1.),
            tabs_shown: Cell::new(0.),
            window_width: Cell::new(None),
            graph,
            pinned,
            _subscriptions: subscriptions,
        };
        // A wheel scroll is the user's, so the diff stops gliding where the keys sent it.
        let this = cx.weak_entity();
        detail.list.set_scroll_handler(move |_, _, cx| {
            (this.update(cx, |detail, _| {
                detail.glide = None;
                detail.reopening = None;
            }))
            .ok();
        });
        // Words being written outlast the tile that closes or the app that quits around them.
        cx.on_release(|this, _| this.keep_brief_writing()).detach();
        cx.on_app_quit(|this, cx| {
            this.keep_brief_writing();
            // Typed in a file and not saved yet.
            this.save_at_quit(cx);
            async {}
        })
        .detach();
        // So is one of a brief, and the keys aren't where it's being read any more.
        let this = cx.weak_entity();
        detail.brief.list.set_scroll_handler(move |_, _, cx| {
            (this.update(cx, |detail, cx| {
                if let Some(brief) = detail.page.brief_mut()
                    && brief.cursor.take().is_some()
                {
                    cx.notify();
                }
            }))
            .ok();
        });
        detail.enter_page(window, cx);
        match detail.pinned {
            Some(_) => {
                detail.shown_changed(window, cx);
                detail.restore_reword(window, cx);
            }
            None => detail.refresh_diff(true, cx),
        }
        detail
    }

    /// The page shown: what was selected as it last took a selection, or for a pinned page
    /// what it was pinned to.
    pub(crate) fn selection(&self) -> Option<Selection> {
        self.shows.clone()
    }

    /// The tab of the project's page shown: the first, on any other page.
    pub(crate) fn project_tab(&self) -> ProjectTab {
        self.page.project_tab().unwrap_or_default()
    }

    /// The project whose page shows `tab`, while one does.
    fn on_tab(&self, tab: ProjectTab) -> Option<ProjectId> {
        match self.selection() {
            Some(Selection::Project(project)) if self.project_tab() == tab => Some(project),
            _ => None,
        }
    }

    /// Show `tab` of the project's page shown, as it was left or as it opens. Keys on the tabs,
    /// or outside this page, stay where they are. Elsewhere on it what had them may be gone with
    /// the tab it was on, so they go to what the tab shows, without opening anything there.
    pub(crate) fn show_tab(
        &mut self,
        tab: ProjectTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if tab == self.project_tab() || !matches!(self.selection(), Some(Selection::Project(_))) {
            return;
        }
        let left_at = self.place_left();
        self.note_places(cx);
        self.reset_view();
        self.show_page_on(tab, left_at, window, cx);
        self.page_shown(cx);
        self.count_tab(cx);
        let on_page = window
            .focused(cx)
            .is_some_and(|focused| self.owns_focus(&focused));
        if on_page && !self.project_tabs_focused(window) {
            let pane = self.key_pane_entered(KeyMove::Beside(Side::Right), cx);
            window.focus(self.pane_focus(pane.unwrap_or(KeyPane::Diff)), cx);
        }
        cx.notify();
    }

    /// Show `tab`, asked for of the project being selected: coming to its page it opens there,
    /// and on it already, the tab goes back to how it opens.
    fn tab_asked(&mut self, tab: ProjectTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.shows != self.store.read(cx).selection {
            self.shown_changed_on(Some(tab), window, cx);
            self.count_tab(cx);
        } else if tab == self.project_tab() {
            self.reopen_tab(cx);
        } else {
            self.show_tab(tab, window, cx);
        }
    }

    /// For the metrics: the home tile's page came to a tab.
    fn count_tab(&self, cx: &App) {
        if self.pinned.is_none()
            && let Some(project) = self.selected_project()
        {
            self.store.read(cx).count_tab(&project, self.project_tab());
        }
    }

    /// ← or → on the project's tabs: the tab on that side, if there is one. Says whether there
    /// was.
    pub(crate) fn step_project_tab(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let beside = self.project_tab().beside(forward);
        if let Some(tab) = beside {
            self.show_tab(tab, window, cx);
        }
        beside.is_some()
    }

    /// Back to how the tab shown opens, as when it's asked for again or the crumb naming it is
    /// clicked.
    fn reopen_tab(&mut self, cx: &mut Context<Self>) {
        match self.project_tab() {
            ProjectTab::Overview => {
                if let Some(project) = self.on_tab(ProjectTab::Overview) {
                    self.pick_opening(&project, cx)
                }
            }
            ProjectTab::Branches => {
                self.close_opened_branch(cx);
            }
            ProjectTab::History => self.pick_newest_operation(cx),
            ProjectTab::Graph => self.graph.update(cx, |graph, cx| graph.go_to_opening(cx)),
        }
    }

    /// Whether commit `id`'s page shows it together with the others picked: not a pinned page,
    /// which is that commit's alone.
    fn shows_picks(&self, project: &ProjectId, id: CommitId, cx: &App) -> bool {
        self.pinned.is_none() && self.store.read(cx).among_picks(project, id)
    }

    /// Whether it has a page to show: something's selected, or what it's pinned to is there.
    pub fn shows_page(&self) -> bool {
        self.selection().is_some()
    }

    /// The row a pinned page shows, found again, as a commit is once an agent rewrote it.
    /// Whether it changed.
    fn find_pinned(&mut self, cx: &App) -> bool {
        let Some((_, row)) = &self.pinned else {
            return false;
        };
        let now = self.store.read(cx).selection_of(row).or(self.shows.clone());
        let changed = now != self.shows;
        self.shows = now;
        changed
    }

    /// Take what's selected, if this hasn't yet. The event saying so comes as the update making
    /// the selection ends, so what acts on the page it just selected asks for it here first.
    pub(crate) fn follow_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pinned.is_none() && self.shows != self.store.read(cx).selection {
            self.shown_changed(window, cx);
        }
    }

    /// Show what's selected now, or for a pinned page what it was pinned to, once it's found.
    fn shown_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shown_changed_on(None, window, cx)
    }

    /// [`Detail::shown_changed`], on `tab` of a project's page, or else the one it was left on.
    fn shown_changed_on(
        &mut self,
        tab: Option<ProjectTab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let left_at = self.place_left();
        self.note_places(cx);
        if self.pinned.is_none() {
            self.shows = self.store.read(cx).selection.clone();
        }
        // A commit an operation rewrote is still the one being looked at.
        let followed = (self.key_for_selection(cx)).is_some_and(|key| self.follows(&key, cx));
        if !followed {
            self.reset_view();
        }
        self.show_page(left_at, tab, window, cx);
        // A project's page comes back on the tab it was left on.
        if tab.is_none() && self.project_tab() != ProjectTab::Overview {
            self.count_tab(cx);
        }
        let project = self.selected_project();
        let checkout = self.selected_checkout().map(|(_, worktree)| worktree);
        if self.checks_project != project {
            // The branch picked to commit to waits for coming back, as checks do.
            let left = self.checks_project.clone();
            if let Some((left, target)) = left.zip(self.commit_target.take()) {
                self.other_targets.insert(left, target);
            }
            self.commit_target =
                (project.as_ref()).and_then(|next| self.other_targets.remove(next));
            self.switch_checkout(project, checkout.unwrap_or_default(), cx);
        } else if let Some(worktree) = checkout
            && worktree != self.checks_worktree
        {
            // Checks are of one checkout's files, which another's share paths with.
            self.switch_checkout(project, worktree, cx);
        }
        self.page_shown(cx);
    }

    /// Load what the page just shown needs.
    fn page_shown(&mut self, cx: &mut Context<Self>) {
        if let Some(project) = self.on_tab(ProjectTab::Overview) {
            self.resume_project_page(&project);
        }
        self.reload_page(true, cx);
        self.refresh_diff(true, cx);
        self.open_waiting(false, cx);
        self.ask_briefs(cx);
    }

    /// Where the diff of the page shown is, unless what's loaded is still an earlier page's.
    fn place_left(&self) -> Option<page::Place> {
        let diff = (self.diff.as_ref()).filter(|diff| diff.selection == self.shows)?;
        let sideways = (self.sideways.iter())
            .map(|(path, [old, new])| (path.clone(), [old.offset(), new.offset()]))
            .filter(|(_, offsets)| offsets.iter().any(|offset| *offset != Point::default()))
            .collect();
        Some(page::Place::of(
            diff,
            self.cursor,
            self.list.logical_scroll_top(),
            self.whole.keys().cloned().collect(),
            sideways,
        ))
    }

    /// Show the files whole and scrolled sideways as the diff just loaded was left.
    fn restore_files(&mut self, left_at: &page::Place, cx: &mut Context<Self>) {
        for (path, offsets) in &left_at.sideways {
            if let Some(handles) = self.sideways.get(path) {
                for (handle, offset) in handles.iter().zip(offsets) {
                    handle.set_offset(*offset);
                }
            }
        }
        let files = self.diff.as_ref().map(|diff| diff.files.clone());
        for (ix, file) in files.iter().flat_map(|files| files.iter().enumerate()) {
            if left_at.whole.contains(&file.file.path) {
                self.whole.insert(file.file.path.clone(), None);
                self.load_whole(ix, cx);
            }
        }
    }

    /// Go back to where the diff was left, once the last of its files shown whole is read.
    fn finish_reopening(&mut self) {
        if !self.whole_reading.is_empty() {
            return;
        }
        let (Some(left_at), Some(diff)) = (self.reopening.take(), &self.diff) else {
            return;
        };
        let top = left_at.offset_in(&diff.rows, &diff.files, diff.alone.is_some());
        self.list.scroll_to(top);
    }

    /// Go on with the page of a branch renamed. The selection followed it first, which put the
    /// page away under the old name, so one's waiting under this page's name now.
    fn page_renamed(
        &mut self,
        project: &ProjectId,
        from: Option<&FullName>,
        to: &FullName,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.kept
            .update(cx, |kept, _| kept.follow_rename(project, from, to));
        let Some(key) = self.page_key.clone() else {
            return;
        };
        if let Some(page) = self.kept.update(cx, |kept, _| kept.take(&key)) {
            self.page = page;
            self.enter_page(window, cx);
            self.refresh_diff(true, cx);
        }
    }

    /// Take the state of the page shown now, as it was left or as it opens, keeping the one
    /// left at `left_at`. A commit rewritten is the same page.
    fn show_page(
        &mut self,
        left_at: Option<page::Place>,
        tab: Option<ProjectTab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = tab.unwrap_or_else(|| {
            page::tab_to_show(
                &self.kept,
                self.store.read(cx),
                self.pinned.as_ref(),
                self.shows.as_ref(),
                cx,
            )
        });
        self.show_page_on(tab, left_at, window, cx);
    }

    /// Take the state of the page shown now on `tab`, keeping the one left at `left_at`.
    fn show_page_on(
        &mut self,
        tab: ProjectTab,
        left_at: Option<page::Place>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = page::page_key(
            self.store.read(cx),
            self.pinned.as_ref(),
            self.shows.as_ref(),
            tab,
        );
        if key == self.page_key {
            return;
        }
        self.put_page_away(left_at, cx);
        self.page = page::open_page(&self.kept, key.as_ref(), self.shows.as_ref(), cx);
        self.page_key = key;
        self.enter_page(window, cx);
        // Commits picked together are one diff, which what was picked of one alone isn't of.
        if self.pinned.is_none()
            && !self.store.read(cx).picked.is_empty()
            && let Some(files) = self.page.files_mut()
        {
            files.file_picks.clear();
            files.line_picks = None;
        }
        // Another page's files are as they are, with nothing to slide from.
        self.files_slide = Slide::default();
        self.beside_slide = Slide::default();
    }

    /// Show what the page's state says of what lives in this view rather than in the state: the
    /// fields and editors it was left with, and how far its pull request was scrolled.
    fn enter_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A pinned tile's detail hands back what it read for the list it left.
        if let Some(project) = self.shows.as_ref().map(|shows| shows.project().clone()) {
            match &mut self.page {
                page::PageState::Branches { list, .. } => {
                    if let Some(list) = list.take() {
                        self.branches.insert(project, list);
                    }
                }
                page::PageState::History { list, .. } => {
                    if let Some(list) = list.take() {
                        self.history.insert(project, list.resumed());
                    }
                }
                page::PageState::Project { list, .. } => {
                    if let Some(list) = list.take() {
                        self.upstream.insert(project, list);
                    }
                }
                page::PageState::Changes { .. }
                | page::PageState::Diff { .. }
                | page::PageState::Graph { .. }
                | page::PageState::Overview(_)
                | page::PageState::Blank => {}
            }
        }
        let (filter, find) = (self.page.files_mut()).map_or((None, None), |files| {
            (files.filter.take(), files.find.take())
        });
        self.filtering = filter.is_some();
        let filter = filter.unwrap_or_default();
        if self.file_filter.read(cx).text() != filter {
            (self.file_filter).update(cx, |input, cx| input.restore(filter, cx));
        }
        self.restore_find(find, cx);
        let (edit, side) = (self.page.review_mut()).map_or((None, Point::default()), |review| {
            (review.edit.take(), review.side)
        });
        self.review_side_scroll.set_offset(side);
        self.restore_review_edit(edit, window, cx);
        if let Some((input, _)) = self.side_widgets() {
            let input = input.clone();
            let filter = (self.page.side()).map_or(String::new(), |side| side.filter.clone());
            if input.read(cx).text() != filter {
                input.update(cx, |input, cx| input.restore(filter, cx));
            }
        }
        let graph_of = self.on_tab(ProjectTab::Graph);
        self.graph
            .update(cx, |graph, cx| graph.show_project(graph_of, cx));
        if let page::PageState::Graph { graph: left, .. } = &self.page {
            let left = left.clone();
            self.graph.update(cx, |graph, cx| graph.restore(left, cx));
        }
    }

    /// The field filtering the list of the list page shown, and the list.
    fn side_widgets(&self) -> Option<(&Entity<TextInput>, &ListState)> {
        match &self.page {
            page::PageState::Branches { .. } => Some((&self.branch_filter, &self.branches_scroll)),
            page::PageState::History { .. } => Some((&self.history_filter, &self.history_scroll)),
            page::PageState::Project { .. } => Some((&self.target_filter, &self.upstream_scroll)),
            page::PageState::Changes { .. }
            | page::PageState::Diff { .. }
            | page::PageState::Graph { .. }
            | page::PageState::Overview(_)
            | page::PageState::Blank => None,
        }
    }

    /// Put list `scroll` of `count` rows where the page left it, as it's first drawn again.
    fn restore_side_top(&self, scroll: &ListState, count: usize, cx: &Context<Self>) {
        if let Some(top) = self.page.side().and_then(|side| side.top.take()) {
            panes::resize_side_list(scroll, count, Theme::get(cx));
            scroll.scroll_to(top);
        }
    }

    /// The list has been sent somewhere, so it stays there rather than going where it was left.
    fn forget_side_top(&self) {
        if let Some(side) = self.page.side() {
            side.top.set(None);
        }
    }

    fn people_open(&self) -> bool {
        self.page.side().is_some_and(|side| side.people_open)
    }

    fn set_people_open(&mut self, open: bool) {
        if let Some(side) = self.page.side_mut() {
            side.people_open = open;
        }
    }

    /// Note how far what's in this view rather than the page's state was scrolled, and what its
    /// list's filter says.
    fn note_places(&mut self, cx: &App) {
        let widgets = self.side_widgets();
        let list = widgets.map(|(_, list)| list.logical_scroll_top());
        let filter = widgets.map(|(input, _)| input.read(cx).text().to_owned());
        if let Some(side) = self.page.side_mut() {
            // Not drawn since the page came back, its list is still to go where it was left.
            let waiting = side.top.take();
            side.top.set(waiting.or(list));
            side.filter = filter.unwrap_or_default();
        }
        let (top, side) = (
            self.review_list.logical_scroll_top(),
            self.review_side_scroll.offset(),
        );
        if self.review_list.item_count() > 0
            && let Some(review) = self.page.review_mut()
        {
            review.top.set(Some(top));
            review.side = side;
        }
        if self.brief.list.item_count() > 0
            && let Some(brief) = self.page.brief()
        {
            brief.top.set(Some(self.brief.list.logical_scroll_top()));
        }
    }

    /// Show or hide what's past the title of the message.
    fn toggle_message(&mut self, cx: &mut Context<Self>) {
        let open = self.message_open(cx);
        if let Some(files) = self.page.files_mut() {
            files.message_open = Some(!open);
        }
        cx.notify();
    }

    /// Whether the message of what the page shows is open past its title.
    fn message_open(&self, cx: &App) -> bool {
        (self.page).message_open(self.store.read(cx).prefs.full_messages)
    }

    /// Have the message of what's shown next start as Settings has messages start.
    fn close_message(&mut self) {
        if let Some(files) = self.page.files_mut() {
            files.message_open = None;
        }
    }

    /// Keep the page's state for when it's shown again, as this stops showing it.
    pub(crate) fn keep_page(&mut self, cx: &mut Context<Self>) {
        let left_at = self.place_left();
        self.note_places(cx);
        self.put_page_away(left_at, cx);
    }

    /// Keep the page, left at `left_at`, or where it was before if its diff never came back.
    fn put_page_away(&mut self, left_at: Option<page::Place>, cx: &mut Context<Self>) {
        let mut page = std::mem::replace(&mut self.page, page::PageState::Blank);
        if let Some(files) = page.files_mut() {
            files.place = left_at.or(files.place.take());
            let filter = self.file_filter.read(cx).text().to_owned();
            files.filter = self.filtering.then_some(filter);
            files.find = self.find_left(cx);
        }
        let edit = self.review_edit_left(cx);
        if let Some(review) = page.review_mut() {
            review.edit = edit;
        }
        if let page::PageState::Graph { graph: left, .. } = &mut page {
            *left = self.graph.read(cx).state();
        }
        if self.pinned.is_some() {
            self.reword_left(&mut page, cx);
            // What's read goes with this detail, so it's kept with the page.
            let project = self.shows.as_ref().map(|shows| shows.project().clone());
            match (&mut page, project) {
                (page::PageState::Branches { list, .. }, Some(project)) => {
                    *list = self.branches.remove(&project);
                }
                (page::PageState::History { list, .. }, Some(project)) => {
                    *list = self.history.remove(&project);
                }
                (page::PageState::Project { list, .. }, Some(project)) => {
                    *list = self.upstream.remove(&project);
                }
                _ => {}
            }
        }
        if let Some(key) = self.page_key.take() {
            self.kept.update(cx, |kept, _| kept.keep(key, page));
        }
    }

    fn selected_project(&self) -> Option<ProjectId> {
        self.selection().map(|s| s.project().clone())
    }

    /// The pull request of what a page has picked: the branch opened on the Branches page, or
    /// the target's merge.
    pub(crate) fn page_pull_request(&self) -> Option<String> {
        if let Some(project) = self.on_tab(ProjectTab::Branches) {
            return (self.selected_other_branch(&project)?.review.as_ref()?.url).clone();
        }
        self.target_pull_request(&self.on_tab(ProjectTab::Overview)?)
    }

    /// The menu `items` makes from this view, open at `at` once the update running now is over.
    fn open_menu(
        &self,
        at: Point<Pixels>,
        items: impl FnOnce(&mut Self, &mut Context<Self>) -> Vec<MenuItem> + 'static,
        cx: &mut Context<Self>,
    ) {
        self.app.open_menu(&cx.entity().downgrade(), at, items, cx);
    }

    /// The checkout whose uncommitted changes are shown, which the composer commits.
    pub fn selected_checkout(&self) -> Option<(ProjectId, Checkout)> {
        match self.selection()? {
            Selection::Uncommitted(project, checkout) => Some((project, checkout)),
            _ => None,
        }
    }

    /// The commit `project` is editing, if `but resolve` has one checked out.
    fn editing<'a>(&self, project: &ProjectId, cx: &'a App) -> Option<&'a Editing> {
        self.store.read(cx).workspace(project)?.editing.as_ref()
    }

    fn key_for_selection(&self, cx: &App) -> Option<DiffKey> {
        let selection = self.selection()?;
        let store = self.store.read(cx);
        let source = match &selection {
            Selection::Project(project) => match self.project_tab() {
                ProjectTab::Overview if self.editing(project, cx).is_some() => return None,
                ProjectTab::Overview => self.upstream_source(project, cx)?,
                ProjectTab::History => self.history_source(project)?,
                ProjectTab::Branches => self.branches_source(project)?,
                ProjectTab::Graph => DiffSource::Commit(self.graph_commit()?),
            },
            Selection::Checkout(..) | Selection::Pending(..) => return None,
            Selection::Uncommitted(_, Checkout::Main) => DiffSource::Worktree,
            Selection::Uncommitted(_, Checkout::Linked(name)) => DiffSource::Linked(name.clone()),
            Selection::Mesh(project, id, pick) => self.machine_source(project, id, pick)?,
            // Commits picked with it in one run show their changes together, and with others
            // between them, each one's in turn.
            Selection::Commit(project, id) if self.shows_picks(project, *id, cx) => {
                match store.picked_run(project) {
                    Some((newest, oldest)) => DiffSource::Range { oldest, newest },
                    None => DiffSource::Commits(
                        store
                            .with_picks(project, *id)
                            .iter()
                            .rev()
                            .copied()
                            .collect(),
                    ),
                }
            }
            Selection::Commit(_, id) => DiffSource::Commit(*id),
            Selection::Upstream(_, _, id) => DiffSource::Commit(*id),
            // What a pull would bring in, each of the remote's commits in turn: those a local
            // commit matches are left out of them, so a range from the oldest could bring them
            // back.
            Selection::Section(project, Section::Upstream(ref_name)) => {
                let branch = store.workspace(project)?.find_branch(Some(ref_name))?;
                DiffSource::Commits(branch.incoming.iter().rev().map(|c| c.id).collect())
            }
            Selection::Section(..) | Selection::Terminal(..) => return None,
            Selection::Branch(project, ref_name) => {
                let branch = store.workspace(project)?.find_branch(ref_name.as_ref())?;
                let newest = branch.commits.first()?.id;
                let oldest = branch.commits.last()?.id;
                DiffSource::Range { oldest, newest }
            }
            // Its top branch's newest commit back to its bottom branch's oldest, skipping
            // branches with none yet.
            Selection::Stack(project, bottom) => {
                let branches = &store.workspace(project)?.lane(bottom.as_ref())?.branches;
                let newest = branches
                    .iter()
                    .find_map(|branch| branch.commits.first())?
                    .id;
                let oldest = (branches.iter().rev())
                    .find_map(|branch| branch.commits.last())?
                    .id;
                DiffSource::Range { oldest, newest }
            }
        };
        Some(DiffKey {
            project: selection.project().clone(),
            source,
        })
    }

    /// Load what the selected page lists beside the diff: on selecting it (`force`), and
    /// when its project reloads, if what it lists may have changed.
    fn reload_page(&mut self, force: bool, cx: &mut Context<Self>) {
        match self.selection() {
            Some(Selection::Project(project)) => match self.project_tab() {
                ProjectTab::History => self.load_history(&project, false, cx),
                ProjectTab::Branches => self.load_branches(&project, force, cx),
                ProjectTab::Overview => self.load_upstream(&project, false, force, cx),
                ProjectTab::Graph => {}
            },
            Some(Selection::Mesh(project, ..)) => self.load_machines(&project, force, cx),
            Some(Selection::Checkout(..) | Selection::Uncommitted(..)) => self.load_now(force, cx),
            _ => {}
        }
    }

    /// Forget how the last diff was looked at: its text selection, cursor and folds moving, and
    /// how far the pull request was scrolled. For a new selection, or another item picked on the
    /// same page.
    fn reset_view(&mut self) {
        self.reopening = None;
        self.text_selection = None;
        self.note_selection = None;
        self.cursor = Cursor::default();
        self.reading = None;
        self.file_folds.stop_all();
        self.fold_rows.clear();
        self.review_list.reset(0);
        self.review_side_scroll.set_offset(Default::default());
        self.brief.list.reset(0);
        self.brief.reading = None;
        self.keep_brief_writing();
    }

    /// Show the diff of what was just picked on the selected page, from its top.
    fn show_picked(&mut self, cx: &mut Context<Self>) {
        self.reset_view();
        if let Some(files) = self.page.files_mut() {
            files.folded.clear();
            files.place = None;
            files.file_picks.clear();
            files.line_picks = None;
            files.last_pane = None;
        }
        // Another pick's pull request starts at its top, and its row is where the list is.
        if let Some(review) = self.page.review() {
            review.top.set(None);
        }
        // Another pick of the same branch still has its brief.
        if self.brief_anchor(cx) != self.brief.asked
            && let Some(brief) = self.page.brief_mut()
        {
            *brief = Default::default();
        }
        self.forget_side_top();
        self.refresh_diff(true, cx);
        // A pick on the page changes no selection, which the store's observer would ask on.
        self.ask_forge(cx);
        self.ask_briefs(cx);
        cx.notify();
    }

    /// Load the diff for the current selection, unless it's the one shown and can't have
    /// changed, as only a worktree diff can. `selection_changed` loads it even then, replaces
    /// a load in flight rather than queuing a reload behind it, and waits out a held arrow key.
    fn refresh_diff(&mut self, selection_changed: bool, cx: &mut Context<Self>) {
        let key = self.key_for_selection(cx);
        let current = self.diff.as_ref().map(|diff| &diff.key);
        let stale = match &key {
            None => true,
            Some(key) => current != Some(key) || key.source.live(),
        };
        if !stale && !selection_changed {
            self.open_pending_file(cx);
            return;
        }
        let Some(key) = key else {
            self.diff = None;
            self.loading = None;
            cx.notify();
            return;
        };
        if !selection_changed && self.loading.as_ref() == Some(&key) {
            // Only a worktree diff can change while it loads. A reload queued for another diff
            // would load nothing, as that diff is shown by then, and would skip the second pass
            // that highlights the rest of its files.
            self.reload_queued = key.source.live();
            return;
        }
        self.reload_queued = false;
        if current != Some(&key) {
            self.diff_error = None;
            // The previous diff stays until this one lands, usually a frame or two later,
            // rather than flashing an empty pane; a slow load says so. As when another
            // commit lands on the branch shown, not only when the selection changes.
            self.waiting_since = Some(Instant::now());
            self._waiting = cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SHOW_LOADING).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            });
        }
        let Some(path) = self.store.read(cx).path(&key.project) else {
            return self.wait_for_diff(&key, cx);
        };
        let worktree = key.uncommitted(self.store.read(cx)).map(<[_]>::to_vec);
        if key.source.live() && worktree.is_none() {
            // The workspace is still loading and lists the files itself; diffing now would run
            // a second status on the same repository, slowing both. Its `Loaded` brings us back.
            return self.wait_for_diff(&key, cx);
        }
        self.loading = Some(key.clone());
        let recent = self.recent_files(&key);
        let scope_lines = self.store.read(cx).prefs.scope_lines();
        if let Some(files) = recent.clone()
            && !key.source.live()
            && (files.iter()).all(|file| file.annotated && file.scope_lines == scope_lines)
        {
            self.loading = None;
            self.show_diff(key, files, cx);
            cx.notify();
            return;
        }
        // Else the files on screen: a file whose patch is the same, as between a branch of one
        // commit and that commit, is reused already highlighted.
        let previous = recent.or_else(|| self.diff.as_ref().map(|diff| diff.files.clone()));
        // A single step loads at once. Selections arriving faster than this, from a held
        // arrow key, wait it out: each replaces the task before its blocking load, which
        // nothing can stop once it starts, so only the commit landed on is loaded. Worktree
        // reloads follow the file watcher, which already batches bursts of writes.
        const SETTLE: Duration = Duration::from_millis(40);
        let delay = if selection_changed
            && self
                .selected_at
                .replace(Instant::now())
                .is_some_and(|at| at.elapsed() < SETTLE)
        {
            SETTLE
        } else {
            Duration::ZERO
        };
        self.spawn_load(key, path, worktree, previous, delay, cx);
    }

    /// Read what signed commit `id`, for its header, once.
    fn read_signer(&mut self, id: backend::CommitId, path: PathBuf, cx: &mut Context<Self>) {
        if self.signers.contains_key(&id) {
            return;
        }
        self.signers.insert(id, None);
        cx.spawn(async move |this, cx| {
            let signer = backend::run(move || backend::commit_signer(&path, id)).await;
            let signer = signer
                .inspect_err(|err| tracing::warn!("{err:#}"))
                .ok()
                .flatten();
            if signer.is_some() {
                this.update(cx, |this, cx| {
                    this.signers.insert(id, signer);
                    cx.notify()
                })
                .ok();
            }
        })
        .detach();
    }

    /// Load `key`'s diff after `delay`: first with only the files likely on screen
    /// highlighted, to show it soon, then with all of them.
    fn spawn_load(
        &mut self,
        key: DiffKey,
        path: PathBuf,
        worktree: Option<Vec<FileChange>>,
        previous: Option<Arc<Vec<FileDiff>>>,
        delay: Duration,
        cx: &mut Context<Self>,
    ) {
        let source = key.source.clone();
        let scope_lines = self.store.read(cx).prefs.scope_lines();
        self._load = cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            // Who signed it is read for the commit landed on, not each one a held key passes.
            if let DiffSource::Commit(id) = source {
                (this.update(cx, |this, cx| this.read_signer(id, path.clone(), cx))).ok();
            }
            // About a screen's worth: the rest follow highlighted a moment later.
            const FIRST: usize = 6;
            let result = backend::run({
                let (path, source, worktree) = (path.clone(), source.clone(), worktree.clone());
                move || {
                    backend::load_diff(&path, source, worktree, previous, Some(FIRST), scope_lines)
                }
            })
            .await;
            let shown = this
                .update(cx, |this, cx| {
                    if this.loading.as_ref() != Some(&key) {
                        return None;
                    }
                    this.loading = None;
                    let mut shown = None;
                    match result {
                        Ok(files) => {
                            let partial = files.iter().any(|file| !file.annotated);
                            this.show_diff(key.clone(), Arc::new(files), cx);
                            // The files as shown: sorted into the list's order, they're a copy,
                            // which the full load checks it still replaces.
                            if partial {
                                shown = this.diff.as_ref().map(|diff| diff.files.clone());
                            }
                        }
                        Err(err) => {
                            this.diff_error = Some(format!("{err:#}").into());
                            // Nothing may act on the files of what was shown before.
                            if this.diff.as_ref().is_some_and(|diff| diff.key != key) {
                                this.diff = None;
                            }
                        }
                    }
                    cx.notify();
                    if std::mem::take(&mut this.reload_queued) {
                        this.refresh_diff(false, cx);
                        return None;
                    }
                    shown
                })
                .ok()
                .flatten();
            let Some(shown) = shown else {
                return;
            };
            let previous = Some(shown.clone());
            let result = backend::run(move || {
                backend::load_diff(&path, source, worktree, previous, None, scope_lines)
            })
            .await;
            this.update(cx, |this, cx| {
                // Unless something newer landed in the meantime.
                let current = this.diff.as_ref().map(|diff| &diff.files);
                if this.loading.is_none()
                    && current.is_some_and(|files| Arc::ptr_eq(files, &shown))
                    && let Ok(files) = result
                {
                    this.show_diff(key, Arc::new(files), cx);
                    cx.notify();
                }
            })
            .ok();
        });
    }

    /// Let go of diffs no longer kept, and of the images only they show: gpui keeps an
    /// image on the GPU until told to drop it.
    fn forget(&mut self, gone: Vec<(DiffKey, Arc<Vec<FileDiff>>)>, cx: &mut Context<Self>) {
        let pictures = |diffs: &[(DiffKey, Arc<Vec<FileDiff>>)]| -> Vec<Arc<RenderImage>> {
            (diffs.iter().flat_map(|(_, files)| files.iter()))
                .flat_map(|file| file.pictures().map(|picture| picture.image.clone()))
                .collect()
        };
        let gone = pictures(&gone);
        if gone.is_empty() {
            return;
        }
        let kept: HashSet<_> = pictures(&self.recent)
            .iter()
            .map(|image| image.id)
            .collect();
        let dropped: Vec<_> = (gone.into_iter())
            .filter(|image| !kept.contains(&image.id))
            .collect();
        // Once no window is being updated, as drop_image skips one that is.
        if !dropped.is_empty() {
            cx.defer(move |cx| {
                for image in dropped {
                    cx.drop_image(image, None);
                }
            });
        }
    }

    /// The files `key` had when it was last shown, if that was lately.
    fn recent_files(&self, key: &DiffKey) -> Option<Arc<Vec<FileDiff>>> {
        (self.recent.iter())
            .find(|(recent, _)| recent == key)
            .map(|(_, files)| files.clone())
    }

    /// Whether `key` is of the commit the loaded diff is of, as an operation just rewrote
    /// it: the same diff to look at, kept where it was.
    fn follows(&self, key: &DiffKey, cx: &App) -> bool {
        let Some(diff) = &self.diff else {
            return false;
        };
        match (&diff.key.source, &key.source) {
            (DiffSource::Commit(from), DiffSource::Commit(to)) => {
                diff.key.project == key.project && self.store.read(cx).followed(*from, *to)
            }
            _ => false,
        }
    }

    /// Show a loaded diff, keeping the scroll position and checks when it's a reload.
    fn show_diff(&mut self, key: DiffKey, files: Arc<Vec<FileDiff>>, cx: &mut Context<Self>) {
        // A branch's or stack's diff is of its commits' range, which an agent rewriting or
        // adding one changes: still selected, it's the same diff to look at.
        let selection = self.selection();
        let reloaded = (self.diff.as_ref()).is_some_and(|diff| {
            matches!(
                (&diff.key.source, &key.source),
                (DiffSource::Range { .. }, DiffSource::Range { .. })
            ) && matches!(
                selection,
                Some(Selection::Branch(..) | Selection::Stack(..))
            ) && diff.selection == selection
        });
        let same = self.diff.as_ref().is_some_and(|diff| diff.key == key)
            || self.follows(&key, cx)
            || reloaded;
        if !same {
            self.whole.clear();
            self.sideways.clear();
        }
        for file in files.iter() {
            self.sideways.entry(file.file.path.clone()).or_default();
        }
        // At most this many, holding about this many lines in all (some 80 MB highlighted,
        // at some 400 bytes a line, which an image's pixels count as too), the oldest going
        // first and the one shown staying. Enough that one huge diff doesn't push out the
        // rest.
        const RECENT: usize = 16;
        const RECENT_LINES: usize = 200_000;
        let mut gone: Vec<_> = self
            .recent
            .extract_if(.., |(recent, _)| *recent == key)
            .collect();
        self.recent.push((key.clone(), files.clone()));
        let lines = |recent: &[(DiffKey, Arc<Vec<FileDiff>>)]| -> usize {
            let files = recent.iter().flat_map(|(_, files)| files.iter());
            (files.map(|file| {
                let lines: usize = file.hunks().iter().map(|hunk| hunk.lines.len()).sum();
                let sizes = file.pictures().map(|picture| picture.image.size(0));
                let bytes: usize = sizes
                    .map(|size| (size.width.0 * size.height.0 * 4) as usize)
                    .sum();
                lines + bytes / 400
            }))
            .sum()
        };
        while self.recent.len() > RECENT
            || (self.recent.len() > 1 && lines(&self.recent) > RECENT_LINES)
        {
            gone.push(self.recent.remove(0));
        }
        self.forget(gone, cx);
        // The keys step through the files as the list shows them: in the order picked, and
        // in a tree, folder by folder. Copied only when that's not how they came.
        let prefs = &self.store.read(cx).prefs;
        let (tree, sort) = (prefs.file_view == FileView::Tree, prefs.file_sort);
        let order = self.diff_file_order(cx);
        let files = match files.is_sorted_by(|a, b| order(a, b).is_le()) {
            true => files,
            false => {
                let mut sorted = files.to_vec();
                sorted.sort_by(order);
                Arc::new(sorted)
            }
        };
        let files = self.live_files(&key, files, cx);
        // Before the rows, which can show the cursor's file alone. The file scrolled to is
        // found again in the new files once they're drawn. A page shown again goes back to
        // where it was left.
        // Another commit opens on its message where it's listed, as its header came first.
        let left_at = (self.page.files_mut()).and_then(|files| files.place.take());
        if let Some(left_at) = &left_at {
            self.cursor = left_at.cursor.find(&files);
        } else if same {
            self.cursor = self.cursor_in(&files);
        } else if self.lists_message(&key, cx) {
            self.cursor = Cursor::on_message();
        }
        self.reading = None;
        let (rows, shown, alone) = self.build_rows(&key, &files, false, cx);
        let marks = matches!(alone, Some(Entry::File(_)))
            .then(|| self.change_marks(&rows, &files, cx))
            .flatten();
        let top = match &left_at {
            Some(left_at) => Some(left_at.offset_in(&rows, &files, alone.is_some())),
            None => same.then(|| self.keep_place(&rows, &files, alone.is_some())),
        };
        // The rows go, and with them the one the diff was gliding to: on a reload, as when the
        // rest of its files come in highlighted or an agent writes, it glides on to where that
        // row is now, a match Find went to as much as the cursor's hunk.
        let glide_to = (self.glide.take())
            .filter(|_| same)
            .and_then(|glide| self.row_in(glide.to as usize, &rows, &files));
        // Most rows are a line tall: rows not drawn yet count as one, so the scrollbar's thumb
        // is about the right size before they are.
        (self.list).reset_with_uniform_height(rows.len(), code_line(cx));
        if let Some(top) = top {
            self.list.scroll_to(top);
        }
        if self.checks_project.as_ref() == Some(&key.project)
            && key.checkout().as_ref() == Some(&self.checks_worktree)
        {
            self.checks.reconcile(&files);
        }
        if self.diff.as_ref().is_none_or(|diff| diff.rows != rows) {
            self.text_selection = None;
        }
        let stale: Vec<usize> = (files.iter().enumerate())
            .filter(|(_, file)| {
                matches!(self.whole.get(&file.file.path),
                    Some(Some((fingerprint, ..))) if *fingerprint != file.fingerprint)
            })
            .map(|(ix, _)| ix)
            .collect();
        if let Some(state) = self.page.files_mut() {
            if !state.file_picks.is_empty() {
                let paths: HashSet<&str> = files.iter().map(|f| f.file.path.as_str()).collect();
                (state.file_picks)
                    .follow(|path| paths.contains(path.as_str()).then(|| path.clone()));
            }
            if !same && left_at.is_none() {
                state.line_picks = None;
            }
        }
        self.diff = Some(LoadedDiff {
            key,
            files,
            tree,
            sort,
            rows,
            shown,
            alone,
            marks,
            selection,
        });
        // Before rows change again, which carries it on in turn.
        if let Some((row, _)) = glide_to {
            self.scroll_to_row(row, true);
        }
        self.settle_line_picks();
        self.refresh_grip();
        if self.prune_resolving(cx) {
            self.rebuild_rows(cx);
        }
        for file in stale {
            self.load_whole(file, cx);
        }
        // Another diff's matches count from its first.
        let from_first = !same && left_at.is_none();
        if let Some(left_at) = left_at {
            self.restore_files(&left_at, cx);
            self.reopening = (!left_at.whole.is_empty()).then_some(left_at);
        }
        // A brief's matches are in its words, whatever diff lands.
        self.run_find(from_first && !self.showing_brief(), cx);
        self.diff_error = None;
        self.open_pending_file(cx);
        self.link_brief_names();
    }

    /// Open the diff shown at the place asked for, as far as it has it, and stop asking. A
    /// link's says so when the diff of what it selected lacks the file or the line.
    fn open_pending_file(&mut self, cx: &mut Context<Self>) {
        let Some(PendingFile { place, linked }) = self.pending_file.take() else {
            return;
        };
        let found = self.find_code_place(&place);
        if let Some(found) = found {
            self.reveal_code_place(found, cx);
        }
        let diff_of = self.shown_diff().and_then(|diff| diff.selection.as_ref());
        if linked.is_none() || linked.as_ref() != diff_of {
            return;
        }
        let missing = match (&place, found) {
            (_, None) => "The link's file isn't in those changes now",
            (CodePlace::File { line: Some(_), .. }, Some((_, None, _))) => {
                "The link's line isn't in the file's changes now"
            }
            _ => return,
        };
        (self.store).update(cx, |store, cx| store.snack_warning(missing, cx));
    }

    /// `place`'s file, hunk and line in the diff shown, as far as it says and that has them.
    fn find_code_place(&self, place: &CodePlace) -> Option<(usize, Option<usize>, Option<usize>)> {
        let loaded = self.shown_diff()?;
        let (CodePlace::File { path, .. } | CodePlace::Line { path, .. }) = place;
        let (file_ix, file) =
            (loaded.files.iter().enumerate()).find(|(_, file)| file.file.path == *path)?;
        Some(match place {
            CodePlace::File { line, .. } => {
                let hunk = line.and_then(|line| {
                    (file.hunks().iter())
                        .position(|hunk| hunk.lines.iter().any(|at| at.new == Some(line)))
                });
                (file_ix, hunk, None)
            }
            CodePlace::Line { hunk, line, .. } => (file_ix, Some(*hunk), Some(*line)),
        })
    }

    /// Scroll to what [`Detail::find_code_place`] found, with the cursor on its hunk if it
    /// found one, and that hunk's line in view if it found that too.
    fn reveal_code_place(
        &mut self,
        (file_ix, hunk, line): (usize, Option<usize>, Option<usize>),
        cx: &mut Context<Self>,
    ) {
        self.scroll_to_file(file_ix, false, cx);
        let Some(hunk) = hunk else {
            return;
        };
        self.show_hunk(file_ix, hunk, cx);
        let Some(line) = line else {
            return;
        };
        let row = self.row_of(|row| match *row {
            DiffRow::Line {
                file,
                hunk: of,
                line: at,
            } => (file, of, at) == (file_ix, hunk, line),
            DiffRow::Pair {
                file,
                hunk: of,
                left,
                right,
            } => (file, of) == (file_ix, hunk) && (left == Some(line) || right == Some(line)),
            _ => false,
        });
        if let Some(row) = row {
            // With a few lines above it, for what it's in.
            self.scroll_to_row(row.saturating_sub(4), false);
        }
    }

    /// What dragging the uncommitted changes of `project`'s checkout `worktree`
    /// carries: the checked files and hunks when they're shown here, otherwise all of them, but
    /// only what's [`Selected::movable`].
    pub fn files_to_move(
        &self,
        project: &ProjectId,
        worktree: Checkout,
        cx: &App,
    ) -> Vec<Selected> {
        let files = match self.selected_checkout() == Some((project.clone(), worktree.clone())) {
            true => self.selected_changes(cx),
            false => (self.store.read(cx).workspace(project))
                .and_then(|ws| ws.uncommitted_in(&worktree))
                .map_or_else(Vec::new, |files| {
                    files.iter().map(Selected::whole).collect()
                }),
        };
        files.into_iter().filter(Selected::movable).collect()
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Focus the file list, or the diff while the list is hidden. On the Branches, Operations
    /// history and project pages, focus their list instead, as it takes the arrow keys itself.
    pub fn focus_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let side_list = self.list_page();
        // Not the file list under a pull request's tab, which draws none.
        let files = !side_list && self.files_shown(cx) && !self.over_changes(cx);
        // So has what the project's page opens on, what merged lately, and the Branches page's
        // overview: their lists take them on their first row, as the first step down would.
        if self.showing_shipped(cx) {
            self.step_upstream(1, cx);
        } else if self.showing_branches_overview() {
            self.step_branches(1, cx);
        }
        let handle = match (side_list, files) {
            (true, _) => &self.side_focus,
            (false, true) => &self.files_focus,
            (false, false) => &self.focus,
        };
        window.focus(handle, cx);
        cx.notify();
    }

    /// Whether `handle` is one of the detail's own, the diff's or a list's, rather than a
    /// text field's.
    pub fn owns_focus(&self, handle: &FocusHandle) -> bool {
        [
            &self.focus,
            &self.files_focus,
            &self.side_focus,
            &self.tabs_focus,
            &self.project_tabs_focus,
            &self.people_focus,
            &self.shipped_focus,
        ]
        .contains(&handle)
    }

    pub fn files_focused(&self, window: &Window) -> bool {
        self.files_focus.is_focused(window)
    }

    /// Whether a Branches, Operations history or project page's list has the keys, which
    /// step it rather than the diff beside it.
    pub fn side_list_focused(&self, window: &Window) -> bool {
        self.side_focus.is_focused(window)
    }

    /// The row the cursor is on: on a hunk, the first of its changes, else the box above them,
    /// as when a pending operation takes them; its file's header where its hunk isn't drawn, as
    /// when folded.
    fn cursor_row(&self) -> Option<usize> {
        let Cursor { file, at } = self.cursor;
        if at == Stop::Message {
            return self.row_of(|row| *row == DiffRow::Message);
        }
        let changes = at
            .hunk()
            .and_then(|hunk| self.row_of(|row| self.changes_at(*row) == Some((file, hunk))));
        let gap = || {
            let hunk = at.near()?;
            self.row_of(|row| *row == DiffRow::Hunk { file, hunk })
        };
        (changes.or_else(gap)).or_else(|| self.row_of(|row| *row == DiffRow::File(file)))
    }

    /// The entry the file list marks: the cursor's, or once the diff is scrolled past it, the
    /// one at the diff's top, the file its sticky header names, as in but.dev.
    fn current_entry(&self) -> Entry {
        self.reading.unwrap_or(self.cursor.entry())
    }

    /// What acts on a whole file acts on: the current entry's file, none on the message. What
    /// acts on a hunk still acts on the cursor's.
    fn current_file(&self) -> Option<usize> {
        match self.current_entry() {
            Entry::File(file) => Some(file),
            Entry::Message => None,
        }
    }

    /// Mark the file the diff is scrolled to as the current one while the cursor is out of
    /// sight. Scrolling leaves the cursor where it is, so its hunk is still selected when the
    /// diff comes back to it. Not while the diff glides to the cursor, past other files.
    pub(super) fn follow_scroll(&mut self, cx: &mut Context<Self>) {
        let reading = match self.glide.is_none() && !self.cursor_in_sight(cx) {
            true => self
                .top_entry()
                .filter(|entry| *entry != self.cursor.entry()),
            false => None,
        };
        if self.reading != reading {
            self.reading = reading;
            cx.notify();
        }
    }

    fn top_entry(&self) -> Option<Entry> {
        let top = self.list.logical_scroll_top().item_ix;
        let rows = self.diff.as_ref()?.rows.get(top..)?;
        match rows.first()? {
            DiffRow::Message => Some(Entry::Message),
            _ => rows.iter().find_map(|row| row.file()).map(Entry::File),
        }
    }

    /// Whether any of what the cursor's on shows in the diff: the rows [`Detail::extent`]
    /// gives, or its row where they aren't drawn. Rows not drawn since they were rebuilt count
    /// as tall as they're guessed to be.
    fn cursor_in_sight(&self, cx: &App) -> bool {
        let Some((first, last)) =
            (self.extent(self.cursor)).or_else(|| self.cursor_row().map(|row| (row, row)))
        else {
            return false;
        };
        let top = self.list.logical_scroll_top();
        if last < top.item_ix {
            return false;
        }
        let height = self.list.viewport_bounds().size.height;
        let mut down = -top.offset_in_item;
        for row in top.item_ix..first {
            down += self.row_height(row, cx);
            if down >= height {
                return false;
            }
        }
        true
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }

    /// What right-clicking what has the keys here offers, and where its row is while it shows:
    /// the file under the cursor in the file list or the diff, else what a project page's
    /// Overview, Branches or Operations history tab has selected in its list.
    pub(crate) fn selected_menu(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<(Vec<MenuItem>, Option<Bounds<Pixels>>)> {
        if let Some(items) = self.focused_file_items(window, cx) {
            let row = match self.files_focus.is_focused(window) {
                true => (self.listed_cursor_row.borrow().as_ref())
                    .and_then(|(list, row)| crate::ui::shown_row(list, *row)),
                false => (self.cursor_row()).and_then(|row| crate::ui::shown_row(&self.list, row)),
            };
            return Some((items, row));
        }
        match self.selection()? {
            Selection::Project(project) => match self.project_tab() {
                ProjectTab::Branches => self.branches_page_menu(&project, cx),
                ProjectTab::History => self.operation_page_menu(&project, cx),
                ProjectTab::Overview => self.target_page_menu(&project, cx),
                ProjectTab::Graph => None,
            },
            _ => None,
        }
    }

    /// The rows `files` show: files folded by hand show only their header, and uncommitted
    /// files and hunks, or files of the commit shown, that a pending operation takes away are
    /// left out until it's done. `unfolded`, as they would with every file open and one line a
    /// row, as Find searches them. The commit's message heads them where the diff lists it. With
    /// them, the files shown and, with the single file diff on, the one entry the rows are of.
    fn build_rows(
        &self,
        key: &DiffKey,
        files: &[FileDiff],
        unfolded: bool,
        cx: &App,
    ) -> (Vec<DiffRow>, Vec<usize>, Option<Entry>) {
        let store = self.store.read(cx);
        let terms = self.filter_terms(cx);
        let leaving = key.leaving(store);
        // Hunks, and lines of them, a pending operation takes out of their file.
        let commit = match key.source {
            DiffSource::Commit(id) => store.key_of(&key.project, id),
            _ => None,
        };
        let parts = match (key.checkout(), &commit) {
            (Some(checkout), _) => store.parts_leaving(&key.project, &checkout, None),
            (None, Some(commit)) => {
                store.parts_leaving(&key.project, &Checkout::Main, Some(commit))
            }
            (None, None) => HashMap::new(),
        };
        let parts_of = |file: &FileDiff| parts.get(file.file.path.as_str());
        // A file folding stays open till it's shut.
        let folded = |file: &FileDiff| {
            let path = &file.file.path;
            let by_hand = (self.page.files()).is_some_and(|files| files.folded.contains(path));
            !unfolded && by_hand && !self.file_folds.closing(path)
        };
        let hidden =
            |file: &FileDiff| leaving(&file.file.path) || !keeps_path(&terms, &file.file.path);
        let shown: Vec<usize> = (files.iter().enumerate())
            .filter(|(_, file)| !hidden(file))
            .map(|(ix, _)| ix)
            .collect();
        let message = self.lists_message(key, cx);
        let alone = self.alone(message, &shown, cx);
        let alone_path = match alone {
            Some(Entry::File(ix)) => files.get(ix).map(|file| &file.file.path),
            Some(Entry::Message) | None => None,
        };
        let rows = diff::rows(
            files,
            folded,
            |file| self.drawing_of(file).is_some(),
            |file| {
                hidden(file)
                    || alone == Some(Entry::Message)
                    || alone_path.is_some_and(|path| *path != file.file.path)
            },
            |file, hunk| parts_of(file).is_some_and(|keys| keys.contains(&hunk.key)),
            |file, hunk, line| {
                parts_of(file).is_some_and(|keys| keys.iter().any(|key| key.takes(hunk, line)))
            },
            // A file being edited shows whole as typed, from its own lines.
            |file| {
                (!self.edits_file(file))
                    .then(|| self.whole_lines(file))
                    .flatten()
                    .map(|(_, lines, _)| lines.len() as u32)
            },
            store.prefs.split && !unfolded,
            |file, conflict| self.conflict_view(&file.file.path, conflict),
        );
        let rows = match unfolded || !self.file_folds.any() {
            true => rows,
            false => {
                let moving = |file: usize| {
                    let path = &files.get(file)?.file.path;
                    self.file_folds.openness(path)?;
                    Some(self.fold_rows.get(path).map_or(0, |rows| rows.left_out))
                };
                diff::moving_rows(rows, moving)
            }
        };
        // Apart from the first file as the files are from each other.
        let rows = match (message, alone) {
            (true, None) if !rows.is_empty() => [DiffRow::Message, DiffRow::Gap]
                .into_iter()
                .chain(rows)
                .collect(),
            (true, None | Some(Entry::Message)) => vec![DiffRow::Message],
            _ => rows,
        };
        (rows, shown, alone)
    }

    /// Whose messages the diff of `key` lists first, with them in the diff in place of the page's
    /// header: one commit's, not several picked together, also one only a remote has, or a
    /// branch's commits', or those only a remote has, as a branch's; and the commit or branch
    /// a list page shows over its diff.
    fn listed_messages(&self, key: &DiffKey, cx: &App) -> Option<Messages> {
        let store = self.store.read(cx);
        match (self.shows.as_ref()?, &key.source) {
            (Selection::Commit(project, id), DiffSource::Commit(_))
                if !store.prefs.shows(Part::CommitHeader) && !store.among_picks(project, *id) =>
            {
                Some(Messages::Commit(project.clone(), *id))
            }
            (Selection::Upstream(project, ref_name, id), DiffSource::Commit(_))
                if !store.prefs.shows(Part::CommitHeader) =>
            {
                Some(Messages::UpstreamCommit(
                    project.clone(),
                    ref_name.clone(),
                    *id,
                ))
            }
            (Selection::Branch(project, ref_name), DiffSource::Range { .. })
                if !store.prefs.shows(Part::BranchHeader) =>
            {
                Some(Messages::Branch(project.clone(), ref_name.clone()))
            }
            (Selection::Project(project), DiffSource::Commit(_))
                if self.project_tab() != ProjectTab::History
                    && !self.list_header_shown(project, cx) =>
            {
                Some(Messages::TargetCommit(project.clone()))
            }
            (Selection::Project(project), DiffSource::Branch { .. })
                if self.project_tab() == ProjectTab::Branches
                    && !self.list_header_shown(project, cx) =>
            {
                Some(Messages::OtherBranch(project.clone()))
            }
            (Selection::Section(project, Section::Upstream(ref_name)), DiffSource::Commits(_))
                if !store.prefs.shows(Part::BranchHeader) =>
            {
                Some(Messages::Upstream(project.clone(), ref_name.clone()))
            }
            _ => None,
        }
    }

    fn lists_message(&self, key: &DiffKey, cx: &App) -> bool {
        self.listed_messages(key, cx).is_some()
    }

    fn loaded_messages(&self, cx: &App) -> Option<Messages> {
        self.listed_messages(&self.diff.as_ref()?.key, cx)
    }

    pub(super) fn message_listed(&self, cx: &App) -> bool {
        self.loaded_messages(cx).is_some()
    }

    /// Put the cursor on the commit's message and scroll to it, gliding there with `glide`.
    pub(super) fn scroll_to_message(&mut self, glide: bool, cx: &mut Context<Self>) {
        self.leave_folder();
        self.cursor = Cursor::on_message();
        self.follow_alone(cx);
        if let Some(row) = self.row_of(|row| *row == DiffRow::Message) {
            self.scroll_to_row(row, glide);
        }
        cx.notify();
    }

    /// The one entry the single file diff shows: the commit's `message` while it's listed
    /// and the cursor's on it, else the file of `shown` the cursor's in, or when that's
    /// hidden, the next shown after it, else the last, as `skip_taken` moves the cursor, else
    /// the message. None on a list page with no files beside its list, as Operations history:
    /// nothing there would list the other files.
    fn alone(&self, message: bool, shown: &[usize], cx: &App) -> Option<Entry> {
        if !self.store.read(cx).single_file_diff || (self.list_page() && !self.has_files_beside(cx))
        {
            return None;
        }
        let file = match self.cursor.entry() {
            Entry::Message if message => None,
            _ => (shown.get(shown.partition_point(|&file| file < self.cursor.file)))
                .or(shown.last())
                .copied(),
        };
        file.map(Entry::File)
            .or_else(|| message.then_some(Entry::Message))
    }

    /// The changed lines of `rows` of `files`, sized as the diff draws its rows, for the
    /// marks beside the scrollbar. None when nothing changed, as in a binary file.
    fn change_marks(
        &self,
        rows: &[DiffRow],
        files: &[FileDiff],
        cx: &App,
    ) -> Option<Rc<ChangeMarks>> {
        let line = Theme::get(cx).code_line;
        let change = |file: usize, hunk: usize, line: usize| match files
            .get(file)?
            .hunks()
            .get(hunk)?
            .lines
            .get(line)?
            .kind
        {
            LineKind::Added => Some(Change::Added),
            LineKind::Removed => Some(Change::Removed),
            LineKind::Context => None,
        };
        let mut marks = ChangeMarks::default();
        for row in rows {
            let (height, changes) = match *row {
                // Its header stays above the marks, which run beside its lines.
                DiffRow::Message | DiffRow::File(_) => (0., [None, None]),
                // As `row_height` guesses them.
                DiffRow::Hunk { file, hunk } => {
                    match (files.get(file)).is_some_and(|file| self.boxless(file, hunk)) {
                        true => (0., [None, None]),
                        false => (40., [None, None]),
                    }
                }
                DiffRow::Gap => (10., [None, None]),
                DiffRow::Line {
                    file,
                    hunk,
                    line: ix,
                } => (line, [change(file, hunk, ix), None]),
                DiffRow::Pair {
                    file,
                    hunk,
                    left,
                    right,
                } => {
                    let side = |ix: Option<usize>| change(file, hunk, ix?);
                    (line, [side(left), side(right)])
                }
                _ => (line, [None, None]),
            };
            marks.push(height, changes);
        }
        (!marks.is_empty()).then(|| Rc::new(marks))
    }

    /// Show the cursor's entry alone once the cursor goes to another, or every file once
    /// Settings turns the single file diff off, and list the commit's message or stop as
    /// Settings lists messages in the diff or not.
    pub(super) fn follow_alone(&mut self, cx: &mut Context<Self>) {
        let message = self.message_listed(cx);
        if !message && self.cursor.at == Stop::Message {
            self.cursor = Cursor::default();
            self.reading = None;
        }
        let Some(diff) = &self.diff else {
            return;
        };
        // With every file shown, the message heads them or not; alone, it is what's shown.
        let heads = diff.rows.first() == Some(&DiffRow::Message);
        let stale = diff.alone.is_none() && heads != message;
        if stale || diff.alone != self.alone(message, &diff.shown, cx) {
            self.rebuild_rows(cx);
        }
    }

    /// What the open file filter keeps paths by: each term's letters, in order, somewhere in
    /// them.
    fn filter_terms(&self, cx: &App) -> Vec<String> {
        if !self.filtering {
            return Vec::new();
        }
        // As typed: the matchers fold case a letter at a time, and folding the whole word first
        // can make two letters of one, as İ becomes i and a dot.
        (self.file_filter.read(cx).text().split_whitespace())
            .map(str::to_owned)
            .collect()
    }

    fn open_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The project, Branches and History pages filter their lists, as the navigator does,
        // unless the files beside the list have the keys.
        let page = match (self.selection(), self.project_tab()) {
            _ if self.files_focused(window) => None,
            (Some(Selection::Project(_)), ProjectTab::Branches) => Some(self.branch_filter.clone()),
            (Some(Selection::Project(_)), ProjectTab::History) => Some(self.history_filter.clone()),
            (Some(Selection::Project(_)), ProjectTab::Overview) => Some(self.target_filter.clone()),
            _ => None,
        };
        if let Some(input) = page {
            input.update(cx, |input, cx| input.select_all_text(cx));
            return window.focus(&input.focus_handle(cx), cx);
        }
        self.open_file_filter(window, cx)
    }

    /// Open the field heading the file list, beside the diff or beside a list page's list.
    fn open_file_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filtering = true;
        // Its field heads the file list, so a hidden list comes back with it.
        self.hide_files(self.list_page(), false, cx);
        self.file_filter
            .update(cx, |input, cx| input.select_all_text(cx));
        window.focus(&self.file_filter.focus_handle(cx), cx);
        self.rebuild_rows(cx);
    }

    /// Close the file filter if it's open, saying whether it was.
    pub fn close_open_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let open = self.filtering;
        if open {
            self.close_filter(window, cx);
        }
        open
    }

    fn close_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filtering = false;
        // The keys the field had go to the list it heads, or to the diff when that's going too.
        // Elsewhere they stay where they are.
        if self.file_filter.focus_handle(cx).is_focused(window) {
            let to = match self.files_shown(cx) {
                true => &self.files_focus,
                false => &self.focus,
            };
            window.focus(to, cx);
        }
        self.file_filter
            .update(cx, |input, cx| input.set_text("", cx));
        self.rebuild_rows(cx);
    }

    /// Wait for `key`'s diff, which can't load yet. A load still running is of what was
    /// selected before, which mustn't land under it, and nothing may act on the files of another
    /// diff shown meanwhile.
    fn wait_for_diff(&mut self, key: &DiffKey, cx: &mut Context<Self>) {
        self.loading = None;
        if self.diff.as_ref().is_some_and(|diff| diff.key != *key) {
            self.diff = None;
            cx.notify();
        }
    }

    /// The diff to draw: the loaded one, or while the next loads the previous one, until
    /// the wait is long enough to show it's loading instead.
    fn shown_diff(&self) -> Option<&LoadedDiff> {
        let diff = self.diff.as_ref()?;
        let waiting = self.loading.as_ref().is_some_and(|key| *key != diff.key);
        (!waiting || !self.slow_to_load()).then_some(diff)
    }

    /// Whether the diff waited for has taken long enough to say it's loading.
    fn slow_to_load(&self) -> bool {
        (self.waiting_since).is_some_and(|since| since.elapsed() >= SHOW_LOADING)
    }

    /// Scroll the diff `by` its height, down or, negative, up. Down from its end, the app goes
    /// on to the next unread commit, so Space reads a run of commits from the first to the last.
    /// A file at a time, it stops at the file's end: the next commit would skip the other files,
    /// which the file list goes to.
    pub fn page(&mut self, by: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.showing_brief() {
            let height = self.brief.list.viewport_bounds().size.height;
            self.brief.list.scroll_by(height * by);
            return cx.notify();
        }
        let at_end = -self.list.scroll_px_offset_for_scrollbar().y
            >= self.list.max_offset_for_scrollbar().y - crate::ui::fixed(1.);
        if by > 0. && at_end {
            if self.single_file_shown() {
                return;
            }
            return self.app.update_later(window, cx, |app, window, cx| {
                (app.navigator).update(cx, |navigator, cx| navigator.read_on(window, cx))
            });
        }
        self.glide = None;
        let height = self.list.viewport_bounds().size.height;
        self.list.scroll_by(height * by);
        cx.notify();
    }

    /// Scroll the diff `lines` code lines, down or, negative, up, leaving the cursor and the
    /// keys where they are, as ⇧J and ⇧K read on from the file list.
    fn scroll_lines(&mut self, lines: f32, cx: &mut Context<Self>) {
        self.scroll_diff_lines(lines, cx);
        cx.notify();
    }

    /// Scroll the diff as [`Self::scroll_lines`] does, without drawing the detail again: only
    /// the diff's pane, which its caller redraws, shows where the diff is.
    pub(super) fn scroll_diff_lines(&mut self, lines: f32, cx: &App) {
        self.glide = None;
        // Within the diff: past its end the list would scroll on into nothing.
        let at = -self.list.scroll_px_offset_for_scrollbar().y;
        let max = self.list.max_offset_for_scrollbar().y.max(Pixels::ZERO);
        let by = (code_line(cx) * lines).max(-at).min(max - at);
        self.list.scroll_by(by);
    }

    /// Where the view goes in `rows` of `files` to keep showing what's at its top now: the
    /// same row, or its file's header when that row went away. Rows of one file `alone`
    /// that's not the one at the top start at their own top.
    fn keep_place(&self, rows: &[DiffRow], files: &[FileDiff], alone: bool) -> ListOffset {
        let top = self.list.logical_scroll_top();
        let Some(diff) = &self.diff else {
            return top;
        };
        let row = page::RowPlace::of(diff, top.item_ix);
        page::offset_in(top, row.as_ref(), rows, files, alone)
    }

    /// Where row `at` of the rows shown is in `rows` of `files`: the same row, and true, or its
    /// file's header when that row went away, and false.
    fn row_in(&self, at: usize, rows: &[DiffRow], files: &[FileDiff]) -> Option<(usize, bool)> {
        page::RowPlace::of(self.diff.as_ref()?, at)?.find(rows, files)
    }

    /// Rebuild the rows when folding changed, keeping the scroll position.
    fn rebuild_rows(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            cx.notify();
            return;
        };
        let (rows, shown, alone) = self.build_rows(&diff.key, &diff.files, false, cx);
        let marks = matches!(alone, Some(Entry::File(_)))
            .then(|| self.change_marks(&rows, &diff.files, cx))
            .flatten();
        if rows != diff.rows {
            self.text_selection = None;
            let scroll_to = self.keep_place(&rows, &diff.files, alone.is_some());
            // A glide goes on to where its row is now, as when a file shown whole is read
            // while the keys or a click send the diff somewhere.
            let glide_to = (self.glide.take())
                .and_then(|glide| self.row_in(glide.to as usize, &rows, &diff.files));
            (self.list).reset_with_uniform_height(rows.len(), code_line(cx));
            self.list.scroll_to(scroll_to);
            if let Some(diff) = &mut self.diff {
                diff.rows = rows;
            }
            if let Some((row, _)) = glide_to {
                self.scroll_to_row(row, true);
            }
            // What Find counts follows what's shown.
            self.run_find(false, cx);
        }
        if let Some(diff) = &mut self.diff {
            (diff.shown, diff.alone, diff.marks) = (shown, alone, marks);
        }
        cx.notify();
    }

    fn is_folded(&self, file: usize) -> bool {
        self.diff.as_ref().is_some_and(|diff| {
            let next = diff
                .rows
                .iter()
                .position(|row| matches!(row, DiffRow::File(ix) if *ix == file));
            next.is_some_and(|row| diff::ends_card(&diff.rows, row))
        })
    }

    /// Show diffs side by side, or one above the other.
    pub fn toggle_split(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.set_diff_prefs(|prefs| prefs.split = !prefs.split, cx)
        });
        self.rebuild_rows(cx);
    }

    /// Wrap long lines, or stop. Rows change height, so the list measures them again.
    pub fn toggle_wrap(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.set_diff_prefs(|prefs| prefs.wrap = !prefs.wrap, cx)
        });
        self.remeasure(cx);
    }

    /// Change how much of the code around each hunk diffs read and show, and load the diff
    /// on screen again with it.
    pub fn set_semantic_context(
        &mut self,
        update: impl FnOnce(&mut Prefs),
        cx: &mut Context<Self>,
    ) {
        self.store
            .update(cx, |store, cx| store.set_prefs(update, cx));
        self.refresh_diff(true, cx);
    }

    /// Draw changed lines with their green and red ground, or without.
    fn toggle_plain_lines(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.set_diff_prefs(|prefs| prefs.plain_lines = !prefs.plain_lines, cx)
        });
        cx.notify();
    }

    pub fn set_code_size(&mut self, size: crate::theme::CodeSize, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.code_size = size, cx)
        });
        cx.set_global(Theme::get(cx).clone().with_code(size));
        self.remeasure(cx);
        cx.refresh_windows();
    }

    /// Have the diff's rows measured again, as their heights changed, keeping its place.
    pub(crate) fn remeasure(&mut self, cx: &mut Context<Self>) {
        let top = self.list.logical_scroll_top();
        let count = self.diff.as_ref().map_or(0, |diff| diff.rows.len());
        (self.list).reset_with_uniform_height(count, code_line(cx));
        self.glide = None;
        self.list.scroll_to(top);
        self.review_list.remeasure();
        self.brief.list.remeasure();
        let theme = Theme::get(cx);
        for side in [
            &self.uncommitted_files,
            &self.diff_files,
            &self.upstream_scroll,
            &self.branches_scroll,
            &self.history_scroll,
        ] {
            panes::resize_side_list(side, side.item_count(), theme);
        }
        cx.notify();
    }

    fn row_of(&self, wanted: impl Fn(&DiffRow) -> bool) -> Option<usize> {
        self.diff.as_ref()?.rows.iter().position(wanted)
    }

    /// Step `delta` entries through the ones shown, the commit's message and the files, and
    /// with `folders` a tree's folders too, letting picks go.
    fn move_file(&mut self, delta: isize, folders: bool, cx: &mut Context<Self>) {
        let Some(stop) = self.file_step(delta, folders, cx) else {
            return;
        };
        let cursor = self.cursor;
        if let Some(files) = self.page.files_mut() {
            files.file_picks.clear();
        }
        match stop {
            ListStop::Entry(Entry::File(file)) => self.scroll_to_file(file, true, cx),
            ListStop::Entry(Entry::Message) => self.scroll_to_message(true, cx),
            ListStop::Folder(dir) => {
                if let Some(files) = self.page.files_mut() {
                    files.cursor_dir = Some(page::CursorDir {
                        dir,
                        at: cursor,
                        file: None,
                    });
                }
                cx.notify();
            }
        }
    }

    /// The stop `delta` stops from the current one, through the ones the file list shows: the
    /// commit's message where it's listed, then the files, and with `folders` a tree's folders.
    fn file_step(&self, delta: isize, folders: bool, cx: &App) -> Option<ListStop> {
        let none = HashSet::new();
        let collapsed = (self.page.files()).map_or(&none, |files| &files.collapsed_dirs);
        let message = self
            .message_listed(cx)
            .then_some(ListStop::Entry(Entry::Message));
        let shown: Vec<ListStop> = (message.into_iter())
            .chain(self.list_stops(collapsed, folders, cx))
            .collect();
        if shown.is_empty() {
            return None;
        }
        // Found by place, as the filter's ranking can list the files out of the diff's order.
        // From a file that isn't shown, the next shown one counts as a step.
        let current = self.current_stop(folders, cx);
        let at = match shown.iter().position(|stop| *stop == current) {
            Some(at) => at as isize,
            None => {
                let entry = self.current_entry();
                let at = (shown.iter())
                    .position(|stop| matches!(stop, ListStop::Entry(e) if *e >= entry))
                    .unwrap_or(shown.len()) as isize;
                at - isize::from(delta >= 0)
            }
        };
        (shown.get((at + delta).clamp(0, shown.len() as isize - 1) as usize)).cloned()
    }

    /// What the file list's keys are on: the folder they stepped onto, else the current entry.
    fn current_stop(&self, folders: bool, cx: &App) -> ListStop {
        match self.cursor_folder(cx).filter(|_| folders) {
            Some(dir) => ListStop::Folder(dir.to_owned()),
            None => ListStop::Entry(self.current_entry()),
        }
    }

    /// Whether the current stop is the first the file list shows, with none over it to step to.
    fn on_first_entry(&self, folders: bool, cx: &App) -> bool {
        let current = self.current_stop(folders, cx);
        (self.file_step(-1, folders, cx)).is_none_or(|stop| stop == current)
    }

    /// Whether the current stop is the last the file list shows, with none under it to step to.
    fn on_last_entry(&self, folders: bool, cx: &App) -> bool {
        let current = self.current_stop(folders, cx);
        (self.file_step(1, folders, cx)).is_none_or(|stop| stop == current)
    }

    /// Whether the file list's keys stop on its folders: it has them, shows a tree, and
    /// Settings asks for it.
    fn folder_stops(&self, window: &Window, cx: &App) -> bool {
        let prefs = &self.store.read(cx).prefs;
        prefs.folder_stops
            && prefs.file_view == FileView::Tree
            && self.files_focus.is_focused(window)
    }

    /// The tree's folder the file list's keys stepped onto, while it's the current one: the
    /// diff's cursor hasn't moved since, and the diff still has a file in it.
    pub(super) fn cursor_folder(&self, cx: &App) -> Option<&str> {
        let prefs = &self.store.read(cx).prefs;
        if !prefs.folder_stops || prefs.file_view != FileView::Tree {
            return None;
        }
        let page::CursorDir { dir, at, .. } = self.page.files()?.cursor_dir.as_ref()?;
        let holds = (self.diff.as_ref()?.files.iter()).any(|file| in_dir(&file.file.path, dir));
        (*at == self.cursor && holds).then_some(dir.as_str())
    }

    /// The files the diff shows in folder `dir`, at any depth, in the file list's order.
    pub(super) fn folder_files(&self, dir: &str, cx: &App) -> Vec<usize> {
        let Some(diff) = &self.diff else {
            return Vec::new();
        };
        (self.files_in_list_order(&HashSet::new(), cx).into_iter())
            .filter(|ix| (diff.files.get(*ix)).is_some_and(|file| in_dir(&file.file.path, dir)))
            .collect()
    }

    /// Fold the tree's folder `dir` shut, open it, or with `None` the other of the two.
    pub(super) fn fold_dir(&mut self, dir: String, fold: Option<bool>, cx: &mut Context<Self>) {
        let Some(files) = self.page.files_mut() else {
            return;
        };
        let shut = files.collapsed_dirs.contains(&dir);
        let close = fold.unwrap_or(!shut);
        if close == shut {
            return;
        }
        match close {
            true => files.collapsed_dirs.insert(dir.clone()),
            false => files.collapsed_dirs.remove(&dir),
        };
        self.page_folds.start(PageFold::Dir(dir), !close, cx);
        cx.notify();
    }

    /// Let go of the folder the file list's keys stepped onto, as the keys or a click go to
    /// an entry.
    fn leave_folder(&mut self) {
        if let Some(files) = self.page.files_mut() {
            files.cursor_dir = None;
        }
    }

    /// The files the file list shows, in its order: not those a filter or a pending
    /// operation hides, nor, in the tree, those of a directory folded shut. While the filter
    /// is open, the best matches first, as it ranks them.
    fn listed_files(&self, cx: &App) -> Vec<usize> {
        let none = HashSet::new();
        let collapsed = (self.page.files()).map_or(&none, |files| &files.collapsed_dirs);
        self.files_in_list_order(collapsed, cx)
    }

    /// The files the diff shows, in the file list's order, with the directories in
    /// `collapsed` folded shut in a tree.
    fn files_in_list_order(&self, collapsed: &HashSet<String>, cx: &App) -> Vec<usize> {
        (self.list_stops(collapsed, false, cx).into_iter())
            .filter_map(|stop| match stop {
                ListStop::Entry(Entry::File(file)) => Some(file),
                _ => None,
            })
            .collect()
    }

    /// The files the diff shows, in the file list's order, with the directories in
    /// `collapsed` folded shut in a tree, and with `folders` the tree's folders among them.
    fn list_stops(&self, collapsed: &HashSet<String>, folders: bool, cx: &App) -> Vec<ListStop> {
        let files = |shown: Vec<usize>| -> Vec<ListStop> {
            (shown.into_iter())
                .map(|file| ListStop::Entry(Entry::File(file)))
                .collect()
        };
        let shown = self.shown_files();
        let Some(diff) = &self.diff else {
            return files(shown);
        };
        let shown: Vec<(usize, &str)> = (shown.into_iter())
            .filter_map(|ix| Some((ix, diff.files.get(ix)?.file.path.as_str())))
            .collect();
        let (shown, paths): (Vec<usize>, Vec<&str>) =
            (ranked(&self.filter_terms(cx), shown, |(_, path)| *path).into_iter())
                .map(|(file, _)| file)
                .unzip();
        if self.store.read(cx).prefs.file_view == FileView::List {
            return files(shown);
        }
        (tree_stops(&paths, collapsed, folders).into_iter())
            .filter_map(|stop| match stop {
                ListStop::Entry(Entry::File(pos)) => {
                    Some(ListStop::Entry(Entry::File(*shown.get(pos)?)))
                }
                stop => Some(stop),
            })
            .collect()
    }

    pub(crate) fn move_hunk(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.showing_brief() {
            return self.step_brief(forward, window, cx);
        }
        if self.on_folder_summary(window, cx) {
            return self.step_summary(if forward { 1 } else { -1 }, window, cx);
        }
        let past_end = if forward { Side::Bottom } else { Side::Top };
        // The keys go on to hunks that show, not a cursor out of sight. From a branch's pull
        // request, its changes. Nothing else that hides the diff shows its hunks.
        if self.showing_review(cx) {
            if let Some(review) = self.page.review_mut() {
                review.tab = PageTab::Changes;
            }
            cx.notify();
        }
        if self.diff_hidden(cx) {
            if self.focus.is_focused(window) {
                self.leave(past_end, window, cx);
            }
            return;
        }
        let Some(diff) = &self.diff else {
            return;
        };
        // From the cursor, even scrolled out of sight: the diff glides back to where it went.
        let at = self.cursor_row().unwrap_or(0);
        let stop = next_stop(&diff.rows, at, forward, |row, entry| {
            self.stop_at(row, entry)
        });
        if let Some((cursor, _)) = stop {
            self.cursor = cursor;
            self.reveal_cursor(cx);
            cx.notify();
        }
        // Past its ends, the diff's keys go on to what's over or under it, as the file list's do.
        if stop.is_none() && self.focus.is_focused(window) {
            return self.leave(past_end, window, cx);
        }
        // From the file list too: the diff takes the keys, so the hunk or header shows solid
        // and the keys after act on it.
        if stop.is_some() || self.cursor.at != Stop::File {
            window.focus(&self.focus, cx);
        }
    }

    /// Move the cursor past what it's on, to the next of its kind: from a file's header to the
    /// next file's, from a hunk to the next hunk, in its file or after, as checking one goes on.
    pub(super) fn move_past(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let from = self.cursor;
        let next_of_kind = |cursor: &Cursor| match from.at {
            Stop::Message | Stop::File => {
                cursor.at == Stop::File && cursor.in_file() != from.in_file()
            }
            Stop::Hunk(_) | Stop::Gap(_) => {
                matches!(cursor.at, Stop::Hunk(_))
                    && (cursor.file, cursor.at.near()) != (from.file, from.at.near())
            }
        };
        let mut at = self.cursor_row().unwrap_or(0);
        let next = std::iter::from_fn(|| {
            let (cursor, row) =
                next_stop(&diff.rows, at, true, |row, entry| self.stop_at(row, entry))?;
            at = row;
            Some(cursor)
        })
        .find(next_of_kind);
        if let Some(cursor) = next {
            self.cursor = cursor;
            self.reveal_cursor(cx);
            cx.notify();
        }
    }

    /// Where the cursor's file and hunk sit in freshly loaded `files`, so a reload never
    /// leaves it pointing at a different file or a hunk that's gone.
    fn cursor_in(&self, files: &[FileDiff]) -> Cursor {
        let shown = self.diff.as_ref().map_or(&[][..], |diff| &diff.files[..]);
        page::CursorPlace::of(self.cursor, shown).find(files)
    }

    /// Move the cursor off a file or hunk a pending operation just took away, onto the next
    /// one shown, so the next key acts on what's on screen.
    fn skip_taken(&mut self, cx: &App) {
        let Some(diff) = &self.diff else {
            return;
        };
        let Some(path) = (self.cursor.in_file())
            .and_then(|file| diff.files.get(file))
            .map(|f| &f.file.path)
        else {
            return;
        };
        // The box above a hunk goes with it.
        let at = (self.cursor.file, self.cursor.at.near());
        // The files and hunks shown, in order, as the cursor can be on them.
        let places: Vec<(usize, Option<usize>)> = (diff.rows.iter())
            .filter_map(|row| match *row {
                DiffRow::File(file) => Some((file, None)),
                DiffRow::Hunk { file, hunk } => Some((file, Some(hunk))),
                _ => None,
            })
            .collect();
        let file_shown = places.iter().any(|(file, _)| *file == at.0);
        // Not what the filter or a fold leaves out.
        let taken = match file_shown {
            false => keeps_path(&self.filter_terms(cx), path),
            true => at.1.is_some() && !places.contains(&at) && !self.is_folded(at.0),
        };
        if !taken {
            return;
        }
        // The next file shown, for a file taken, or the next hunk, for a hunk; failing that,
        // the last before it.
        let places: Vec<_> = (places.into_iter())
            .filter(|(_, hunk)| hunk.is_some() == file_shown)
            .collect();
        if let Some(&(file, hunk)) = places.iter().find(|place| **place > at).or(places.last()) {
            self.cursor = Cursor {
                file,
                at: hunk.map_or(Stop::File, Stop::Hunk),
            };
        }
    }

    /// The order the diff has its files in, as the list shows them: in the order picked, and in
    /// a tree, folder by folder.
    fn diff_file_order(
        &self,
        cx: &App,
    ) -> impl Fn(&FileDiff, &FileDiff) -> std::cmp::Ordering + use<> {
        let prefs = &self.store.read(cx).prefs;
        let (tree, sort) = (prefs.file_view == FileView::Tree, prefs.file_sort);
        move |a: &FileDiff, b: &FileDiff| match tree {
            true => file_tree::tree_order(sort, &a.file, &b.file),
            false => file_tree::file_order(sort, &a.file, &b.file),
        }
    }

    fn worktree_diff(&self) -> Option<&LoadedDiff> {
        (self.diff.as_ref()).filter(|diff| diff.key.checkout().is_some())
    }

    /// Whether the loaded diff is the worktree's and shows exactly `files` as they are now.
    /// A file opened to edit with no changes yet isn't among them, though the diff shows it.
    fn shows_worktree(&self, files: &[FileChange]) -> bool {
        (self.worktree_diff()).is_some_and(|diff| {
            let shown: Vec<&FileChange> = (diff.files.iter())
                .map(|shown| &shown.file)
                .filter(|shown| {
                    !self.opened_unchanged(shown)
                        || files.iter().any(|now| now.change.path == shown.change.path)
                })
                .collect();
            same_files(shown.into_iter(), files)
        })
    }

    /// Open the file under the cursor in the editor, at its hunk.
    fn open_in_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let Some(file) = self.cursor.in_file().and_then(|file| diff.files.get(file)) else {
            return;
        };
        let line = (self.cursor.at.near())
            .and_then(|ix| file.hunks().get(ix))
            .map(|h| h.key.0.new_start);
        let (project, path) = (diff.key.project.clone(), file.file.path.clone());
        let worktree = diff.key.files_checkout();
        let position = window.mouse_position();
        self.app.update_later(window, cx, move |app, _, cx| {
            app.open_in_editor(&project, worktree, Some(path), line, position, cx)
        });
    }

    /// Keep committing to `project`'s branch `from` (a full ref) now that it's renamed `to`,
    /// whether it's the project shown or one to come back to.
    pub fn branch_renamed(
        &mut self,
        project: &ProjectId,
        from: Option<&FullName>,
        to: &FullName,
        cx: &mut Context<Self>,
    ) {
        let target = match self.checks_project.as_ref() == Some(project) {
            true => self.commit_target.as_mut(),
            false => self.other_targets.get_mut(project),
        };
        if let Some(target) = target.filter(|target| Some(&**target) == from) {
            *target = to.to_owned();
            cx.notify();
        }
    }

    /// Whether the uncommitted changes' commit footer, or the composer that takes its place,
    /// is at the bottom of the page, where a snackbar would cover its buttons.
    pub fn shows_footer(&self, cx: &App) -> bool {
        let store = self.store.read(cx);
        // Drawn with nothing uncommitted too, as Commit… and the composer still are.
        !self.hides_files(cx)
            && self.selected_checkout().is_some_and(|(project, worktree)| {
                (store.workspace(&project))
                    .and_then(|ws| ws.uncommitted_in(&worktree))
                    .is_some()
            })
    }

    pub fn composing(&self) -> bool {
        self.composing
    }

    /// Whether the keys are in the composer's message.
    pub fn message_focused(&self, window: &Window, cx: &App) -> bool {
        self.message.read(cx).contains_focus(window, cx)
    }

    /// Whether the composer is open on `project`'s `checkout`, whose changes then go by
    /// what's checked.
    pub fn composing_in(&self, project: &ProjectId, checkout: &Checkout) -> bool {
        self.composing
            && self.checks_project.as_ref() == Some(project)
            && self.checks_worktree == *checkout
    }

    /// Whether the diff's cursor is on a hunk that shows, rather than a file's header, the box
    /// above a hunk or a hunk scrolled away.
    pub fn on_hunk(&self, cx: &App) -> bool {
        self.cursor.at.hunk().is_some() && self.cursor_in_sight(cx)
    }

    /// Whether there's a file shown for the diff's keys to act on, not one the filter hides.
    pub fn has_files(&self) -> bool {
        (self.shown_diff())
            .is_some_and(|diff| diff.rows.iter().any(|r| matches!(r, DiffRow::File(_))))
    }

    /// The box the composer's message is written in.
    pub(crate) fn message_editor(&self) -> Entity<MessageEditor> {
        self.message.clone()
    }

    /// Commit to `branch` of `project` next: a branch just made is where its commits go, as
    /// are those started from a branch.
    pub fn branch_created(
        &mut self,
        project: &ProjectId,
        branch: FullName,
        cx: &mut Context<Self>,
    ) {
        if self.checks_project.as_ref() == Some(project) {
            self.place_on(&branch, cx);
            self.commit_target = Some(branch);
            cx.notify();
        }
    }

    /// How wide the file list is drawn while it's shown, beside the navigator as much as that
    /// shows.
    pub(crate) fn files_width(&self, window: &Window, cx: &App) -> f32 {
        self.column_at_rest(window, cx) + self.graph_sliding_by(clock::now(), window, cx)
    }

    /// How much wider than at rest the graph's column is at `now`, as a commit's changes slide
    /// in beside it or away: the diff slides that far past its place, at its own width.
    pub fn graph_sliding_by(&self, now: Instant, window: &Window, cx: &App) -> f32 {
        let open = (self.graph_slide.openness(now)).filter(|_| self.showing_graph());
        open.map_or(0., |open| {
            (self.graph_page - self.column_at_rest(window, cx)) * (1. - open)
        })
    }

    fn column_at_rest(&self, window: &Window, cx: &App) -> f32 {
        let window = self.laid_out_width(window, cx);
        let navigator = self.navigator_shown.get();
        // Fitted beside the diff in the home tile, as if the tiles pinned beside it narrowed
        // the window.
        let store = self.store.read(cx);
        let navigator_width = store.width(Pane::Navigator) * navigator;
        let pinned = (window - navigator_width) * (1. - self.tile_area(cx).width);
        let list = self.column_pane();
        store.fitted_width(list, list, window - pinned, navigator, 1.)
    }

    /// The pane the page's column is: the graph's, or the file list's.
    fn column_pane(&self) -> Pane {
        match self.showing_graph() {
            true => Pane::Graph,
            false => Pane::Files,
        }
    }

    /// How far down its tile the home tile's page starts: under the tile's bar, while its row
    /// has others. A pinned page's bar is above its detail.
    pub fn bar_inset(&self, cx: &App) -> f32 {
        let tiled = self.pinned.is_none() && self.store.read(cx).drawn_home().is_some();
        if tiled { crate::ui::TILE_BAR } else { 0. }
    }

    /// How far the tiles' room sits in from the detail's left: as from its other edges, but
    /// beside the navigator, whose column holds the whole gap to them, so their cards start
    /// right at the detail's edge. It goes from one to the other as the navigator slides, so
    /// the cards never touch the window's edge or jump, and the card the detail draws stays
    /// with the tiles drawn over it in a frame that doesn't draw the detail again.
    pub fn room_left(&self) -> f32 {
        crate::ui::TILE_MARGIN * (1. - 2. * self.navigator_shown.get())
    }

    /// How tall the location bar is: gone with the navigator across the top, which says where
    /// things are, and going as the tabs come down, so what's under it stays put.
    pub fn location_bar_height(&self) -> f32 {
        crate::ui::HEADER_HEIGHT * (1. - self.tabs_shown.get())
    }

    /// How far the tiles' room sits from the bottom of the location bar: up into it as far as
    /// the cards sit in, so they start where it ends, as the navigator's under its own bar,
    /// its controls as far above them as tiles are apart.
    pub fn room_top(&self) -> f32 {
        -crate::ui::TILE_MARGIN
    }

    /// Where the page goes in the room below the header: its tile's area, as it's drawn in
    /// this frame. A pinned page's as it is at rest, which it keeps as it slides open or shut.
    pub fn tile_area(&self, cx: &App) -> Area {
        let store = self.store.read(cx);
        let area = match &self.pinned {
            Some((_, row)) => store.page_area(&Tile::Pinned(row.clone())),
            None => store.drawn_home(),
        };
        area.unwrap_or(Area::WHOLE)
    }

    /// How wide the navigator is drawn while it's open, beside the file list as much as that
    /// shows at `now`. To the point, as AppKit sizes the window, so the window narrowed to it
    /// fits it.
    pub(crate) fn navigator_width(&self, now: Instant, window: &Window, cx: &App) -> f32 {
        let window = self.laid_out_width(window, cx);
        let files = self.files_share(now, cx);
        let list = self.column_pane();
        let width = (self.store.read(cx)).fitted_width(Pane::Navigator, list, window, 1., files);
        let scale = Theme::get(cx).scale;
        (width * scale).round() / scale
    }

    /// The window's width the panes are laid out for, in unscaled pixels.
    fn laid_out_width(&self, window: &Window, cx: &App) -> f32 {
        (self.window_width.get())
            .unwrap_or_else(|| f32::from(window.viewport_size().width) / Theme::get(cx).scale)
    }

    /// Where the diff's cursor is, as an editor's status bar says the line: the place of the
    /// current file among the ones shown, as the file list orders them, and the cursor's hunk
    /// while it's in it. The sticky header names the file.
    pub fn cursor_place(&self, cx: &App) -> Option<String> {
        if self.cursor_folder(cx).is_some() {
            return None;
        }
        let diff = self.shown_diff()?;
        let file = diff.files.get(self.current_file()?)?;
        // The list's order is worked out only while the filter ranks the files, as this is drawn
        // with every frame: otherwise the list has them as the diff does, which loads them in
        // its order. Folders folded shut count too, as the diff shows their files.
        let shown = match self.filter_terms(cx).is_empty() {
            true => self.shown_files(),
            false => self.files_in_list_order(&HashSet::new(), cx),
        };
        let at = shown
            .iter()
            .position(|file| Some(*file) == self.current_file())?
            + 1;
        let mut place = format!("File {at} of {}", shown.len());
        let hunks = file.hunks().len();
        let hunk = (self.cursor.at.hunk()).filter(|hunk| *hunk < hunks && self.reading.is_none());
        if let Some(hunk) = hunk {
            place.push_str(&format!(" · hunk {} of {hunks}", hunk + 1));
        }
        Some(place)
    }

    /// The path of the current file, in any diff: see [`Detail::current_file`].
    pub fn cursor_path(&self) -> Option<String> {
        self.diff_path(self.current_file()?)
    }

    /// The path of the current file, unless the diff it's in is hidden under a pull
    /// request's tab, which draws no files, or under the summary of the folder the file
    /// list's keys are on.
    pub fn shown_cursor_path(&self, cx: &App) -> Option<String> {
        if self.over_changes(cx) || self.cursor_folder(cx).is_some() {
            return None;
        }
        self.cursor_path()
    }

    /// What copying takes from here: the selected text, else the hunk the cursor is on or the
    /// current file's whole diff, as git prints them, or the file's path when it has no lines,
    /// as a binary file hasn't. What it copied blinks, as the TUI highlights it. Under a pull
    /// request's tab, the text selected in its description or comments.
    pub fn copy(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<String> {
        if self.showing_review(cx) {
            return self.selected_note_text(cx);
        }
        if self.showing_brief() {
            return self.selected_brief_text();
        }
        let in_list = self.files_focus.is_focused(window);
        let (text, copied) = match self.selected_text() {
            Some(text) => (text, Copied::Text),
            None => {
                let text = (self.cursor_patch(in_list, cx)).or_else(|| self.shown_cursor_path(cx));
                (text?, Copied::Cursor)
            }
        };
        self.blink_copied(copied, cx);
        Some(text)
    }

    /// The hunk the cursor is on or the current file's whole diff, as copying takes it with no
    /// text selected, and blink it: a menu's Copy hunk, which leaves the selection be. The
    /// file's whole diff `in_list`, as the file list had the keys when the menu opened.
    pub fn copy_patch(&mut self, in_list: bool, cx: &mut Context<Self>) -> Option<String> {
        let patch = self.cursor_patch(in_list, cx)?;
        self.blink_copied(Copied::Cursor, cx);
        Some(patch)
    }

    fn blink_copied(&mut self, copied: Copied, cx: &mut Context<Self>) {
        self.blinked = (self.blinked.0 + 1, Some((copied, Instant::now())));
        cx.notify();
    }

    /// The current file while it's shown and has lines to copy, and the hunk the cursor is on
    /// in it, or on the box above, as the composer's checks take it: none `in_list`, where the
    /// file list's rows are whole files, nor while the diff is scrolled to another file, as
    /// [`Self::cursor_place`] says.
    fn cursor_hunk(&self, in_list: bool, cx: &App) -> Option<(&FileDiff, Option<HunkKey>)> {
        self.shown_cursor_path(cx)?;
        let file = self.diff.as_ref()?.files.get(self.current_file()?)?;
        let hunk = (self.cursor.at.near()).filter(|_| !in_list && self.reading.is_none());
        let key = hunk.and_then(|hunk| Some(file.hunks().get(hunk)?.key));
        file.has_patch().then_some((file, key))
    }

    /// The hunk the cursor is on as git prints it, else the current file's whole diff.
    fn cursor_patch(&self, in_list: bool, cx: &App) -> Option<String> {
        let (file, hunk) = self.cursor_hunk(in_list, cx)?;
        file.patch(hunk)
    }

    /// The diffs ⇧Y offers of the current file, the first what copying takes: the hunk the
    /// cursor is on, then the whole file's.
    pub fn patch_copies(&self, window: &Window, cx: &App) -> Vec<(&'static str, String)> {
        let in_list = self.files_focus.is_focused(window);
        let Some((file, hunk)) = self.cursor_hunk(in_list, cx) else {
            return Vec::new();
        };
        let hunk = hunk.and_then(|key| file.patch(Some(key)));
        (hunk.map(|patch| ("Hunk", patch)).into_iter())
            .chain(file.patch(None).map(|patch| ("Diff", patch)))
            .collect()
    }

    /// Which blink `copied` is in while it blinks: see [`crate::ui::blink_once`].
    fn blink(&self, copied: Copied) -> Option<usize> {
        let (n, last) = self.blinked;
        last.filter(|&(what, at)| what == copied && at.elapsed() < crate::ui::BLINK)
            .map(|_| n)
    }

    /// The indexes of the files the diff shows, in ascending order for the binary searches
    /// over them (Find's among them): not those a filter or a pending operation hides. Every
    /// one, though the single file diff shows them one at a time.
    fn shown_files(&self) -> Vec<usize> {
        self.diff
            .iter()
            .flat_map(|diff| diff.shown.clone())
            .collect()
    }

    /// Fold file `file_ix` to its header, or unfold it. One whose header is in sight moves
    /// there, its lines going up into the header or coming down from it.
    pub fn toggle_fold(&mut self, file_ix: usize, cx: &mut Context<Self>) {
        let Some(path) = self.diff_path(file_ix) else {
            return;
        };
        let Some(files) = self.page.files_mut() else {
            return;
        };
        let open = files.folded.remove(&path);
        if !open {
            files.folded.insert(path.clone());
        }
        let view = self.list.viewport_bounds();
        let header = self.row_of(|row| *row == DiffRow::File(file_ix));
        let shows = (header.and_then(|row| self.list.bounds_for_item(row)))
            .is_some_and(|row| row.bottom() > view.top() && row.top() < view.bottom());
        // Turning back, it keeps the rows it left out; from rest, it closes from where its
        // lines show, under its header stuck at the diff's top too.
        let moving = (self.fold_rows.get(&path))
            .filter(|_| self.file_folds.openness(&path).is_some())
            .map(|rows| rows.left_out);
        let start = match (moving, open) {
            (Some(left_out), _) => shows.then_some((left_out, None)),
            (None, true) => shows.then_some((0, None)),
            (None, false) => self.close_from(file_ix, cx),
        };
        self.cursor = Cursor::on_file(file_ix);
        let Some((left_out, stuck)) = start else {
            self.file_folds.stop(&path);
            self.fold_rows.remove(&path);
            return self.rebuild_rows(cx);
        };
        self.file_folds.start(path.clone(), open, cx);
        if moving.is_none() {
            self.fold_rows.remove(&path);
        }
        // Its lines go as tall as they're drawn, and come as tall as they'll be: from rest, past
        // those it leaves out, which a file already moving has left out.
        if !open {
            let passed = if moving.is_some() { 0 } else { left_out };
            let rows = FoldRows {
                left_out,
                ..self.moving_rows_of(file_ix, passed, cx)
            };
            self.fold_rows.insert(path.clone(), rows);
        }
        self.rebuild_rows(cx);
        if open {
            let rows = FoldRows {
                left_out,
                ..self.moving_rows_of(file_ix, 0, cx)
            };
            self.fold_rows.insert(path, rows);
        }
        // Closing under its header stuck at the top, the header takes the stuck one's place,
        // over the lines that stay where they are, and comes down whole as they go.
        if let (Some(above), Some(header)) = (stuck, header) {
            let at = ListOffset {
                item_ix: header,
                offset_in_item: above,
            };
            self.list.scroll_to(at);
            if above > Pixels::ZERO {
                self.scroll_to_row(header, true);
            }
        }
    }

    /// ⇧← folds the current file and ⇧→ unfolds it, as the navigator folds. Not in a diff that
    /// isn't drawn.
    /// Fold the folder the file list's keys are on, else the current file, shut or open.
    fn fold_cursor(&mut self, fold: bool, cx: &mut Context<Self>) {
        match self.cursor_folder(cx) {
            Some(dir) => self.fold_dir(dir.to_owned(), Some(fold), cx),
            None => self.fold_cursor_file(fold, cx),
        }
    }

    fn fold_cursor_file(&mut self, fold: bool, cx: &mut Context<Self>) {
        let Some(file) = self.current_file() else {
            return;
        };
        let folded = (self.diff_path(file).zip(self.page.files()))
            .map(|(path, files)| files.folded.contains(&path));
        if folded == Some(!fold) && !self.diff_hidden(cx) {
            self.toggle_fold(file, cx);
        }
    }

    /// Bring diff row `row` into view: left where it is when it's on screen, else glided to
    /// just below the top. gpui's own reveal goes by estimated heights, and past rows not
    /// drawn yet, like a file header or an image, it stops short. While the diff glides, where
    /// the row would end up can't be told yet, so it glides on to it instead.
    fn reveal_row(&mut self, row: usize) {
        let view = self.list.viewport_bounds();
        let shown = (self.list.bounds_for_item(row))
            .is_some_and(|item| item.top() >= view.top() && item.bottom() <= view.bottom());
        if !shown || self.glide.is_some() {
            self.scroll_to_row(row.saturating_sub(2), true);
        }
    }

    /// Scroll the diff so `row` starts at its top, gliding there with `glide`, in place of
    /// wherever it was gliding to. With the diff's end on screen, a row below the top can't
    /// come any higher, so the diff stays put: gpui would take such a scroll back only as it
    /// lays the list out, after the sticky header was worked out from it, and the header went
    /// for a frame.
    fn scroll_to_row(&mut self, row: usize, glide: bool) {
        self.glide = None;
        let view = self.list.viewport_bounds();
        let last = (self.diff.as_ref()).map_or(0, |diff| diff.rows.len().saturating_sub(1));
        // Rows on screen were laid out with the list, so where they are isn't an estimate.
        let on_screen = |ix| {
            (self.list.bounds_for_item(ix))
                .is_some_and(|item| item.top() >= view.top() && item.bottom() <= view.bottom())
        };
        if !(on_screen(row) && on_screen(last)) {
            let to = ListOffset {
                item_ix: row,
                offset_in_item: Pixels::ZERO,
            };
            match glide {
                true => {
                    self.glide = Some(Glide {
                        to: row as f32,
                        last: Instant::now(),
                        reveal: false,
                    })
                }
                false => self.list.scroll_to(to),
            }
        }
    }

    /// Glide to bring as much into view as fits of what the cursor's on, as the TUI's details
    /// scroll to a section. In view already, the diff stops where it is: a glide to somewhere
    /// else, as to the next file, would take it out of view.
    fn reveal_cursor(&mut self, cx: &App) {
        self.glide = (self.reveal_target(cx)).map(|to| Glide {
            to,
            last: Instant::now(),
            reveal: true,
        });
    }

    /// Where the diff goes from where it is now to bring what the cursor's on into view: the
    /// rows [`Detail::extent`] gives, scrolled as [`reveal_by`] says. None within a pixel of
    /// there.
    pub(super) fn reveal_target(&self, cx: &App) -> Option<f32> {
        let (first, last) = self.extent(self.cursor)?;
        let at = self.scrolled_to();
        let top = self.distance(at, first as f32, cx);
        let bottom = self.distance(at, (last + 1) as f32, cx);
        // Under a file's header, its sticky header hides the diff's top.
        let clear = match self.cursor.at {
            Stop::Message | Stop::File => Pixels::ZERO,
            Stop::Gap(_) | Stop::Hunk(_) => self.file_header_height(cx),
        };
        // A little of what follows shows below it, rather than it ending against the edge.
        let room = code_line(cx) * 2.;
        let by = reveal_by(
            top,
            bottom,
            clear,
            room,
            self.list.viewport_bounds().size.height,
        );
        (by.abs() >= fixed(1.)).then(|| self.place_after(at, by, cx))
    }

    /// The place the diff shows from: its top row and how far down it, by its height.
    fn scrolled_to(&self) -> f32 {
        let top = self.list.logical_scroll_top();
        let into = (self.list.bounds_for_item(top.item_ix))
            .filter(|item| item.size.height > Pixels::ZERO)
            .map_or(0., |item| top.offset_in_item / item.size.height);
        top.item_ix as f32 + into
    }

    /// Where the list scrolls to for place `at`, a row and how far down it by its height.
    fn list_offset(&self, at: f32, cx: &App) -> ListOffset {
        let row = at.floor() as usize;
        ListOffset {
            item_ix: row,
            offset_in_item: self.row_height(row, cx) * at.fract(),
        }
    }

    /// How far place `to` is below place `from`, or above it when negative, each a row and
    /// how far down it by its height.
    fn distance(&self, from: f32, to: f32, cx: &App) -> Pixels {
        let (upper, lower) = (from.min(to), from.max(to));
        let (first, last) = (upper.floor() as usize, lower.floor() as usize);
        let rows = (first..last).fold(Pixels::ZERO, |sum, ix| sum + self.row_height(ix, cx));
        let span = rows - self.row_height(first, cx) * upper.fract()
            + self.row_height(last, cx) * lower.fract();
        if to < from { -span } else { span }
    }

    /// The place `by` below place `from`, or above it when negative, short of the diff's ends.
    fn place_after(&self, from: f32, by: Pixels, cx: &App) -> f32 {
        let last = (self.diff.as_ref()).map_or(0, |diff| diff.rows.len().saturating_sub(1));
        let mut row = (from.floor() as usize).min(last);
        // How far below the top of `row` the place is.
        let mut down = by + self.row_height(row, cx) * from.fract();
        while down < Pixels::ZERO && row > 0 {
            row -= 1;
            down += self.row_height(row, cx);
        }
        while row < last && down >= self.row_height(row, cx) {
            down -= self.row_height(row, cx);
            row += 1;
        }
        let tall = self.row_height(row, cx);
        let into = match tall > Pixels::ZERO {
            true => (down / tall).clamp(0., 1.),
            false => 0.,
        };
        row as f32 + into
    }

    /// How tall diff row `ix` is: as drawn, or for one gpui hasn't drawn or doesn't say, as
    /// above the diff's top, a guess by what it shows.
    fn row_height(&self, ix: usize, cx: &App) -> Pixels {
        if let Some(item) = self.list.bounds_for_item(ix) {
            return item.size.height;
        }
        let theme = Theme::get(cx);
        let Some(loaded) = &self.diff else {
            return code_line(cx);
        };
        match loaded.rows.get(ix) {
            // About a title, a few lines and its byline, until it's drawn.
            Some(DiffRow::Message) => theme.scaled(160.),
            Some(DiffRow::File(_)) => self.file_header_height(cx),
            // A box 28 tall with 6 above and below it, or nothing where the lines run on.
            Some(&DiffRow::Hunk { file, hunk }) => {
                match (loaded.files.get(file)).is_some_and(|file| self.boxless(file, hunk)) {
                    true => Pixels::ZERO,
                    false => theme.scaled(40.),
                }
            }
            Some(DiffRow::Gap) => theme.scaled(10.),
            _ => code_line(cx),
        }
    }

    /// Put the cursor on hunk `hunk` of file `file_ix`, in view.
    fn show_hunk(&mut self, file_ix: usize, hunk: usize, cx: &mut Context<Self>) {
        self.cursor = Cursor::on_hunk(file_ix, hunk);
        self.reveal_cursor(cx);
        cx.notify();
    }

    /// Put the cursor on file `file_ix`'s header and scroll it to the top, gliding there with
    /// `glide`, as a key or a click goes from file to file. A diff that's just loaded, or a
    /// filter being typed, goes there at once.
    fn scroll_to_file(&mut self, file_ix: usize, glide: bool, cx: &mut Context<Self>) {
        self.leave_folder();
        self.cursor = Cursor::on_file(file_ix);
        self.follow_alone(cx);
        if let Some(row) = self.row_of(|row| matches!(row, DiffRow::File(ix) if *ix == file_ix)) {
            self.scroll_to_row(row, glide);
            cx.notify();
        }
    }

    /// How far below the breadcrumbs the diff starts: under the page's header, for the pages
    /// the render's `header` gives one.
    pub fn view_header_height(&self, cx: &App) -> f32 {
        match self.selection() {
            Some(Selection::Branch(..) | Selection::Stack(..)) if !self.branch_header_shown(cx) => {
                0.
            }
            Some(Selection::Commit(project, id)) if !self.commit_header_shown(&project, id, cx) => {
                0.
            }
            Some(Selection::Upstream(..)) if !self.upstream_header_shown(cx) => 0.,
            Some(
                Selection::Commit(..)
                | Selection::Upstream(..)
                | Selection::Branch(..)
                | Selection::Stack(..),
            ) => self.header_height.get(),
            Some(Selection::Project(project)) => {
                // A list page's commit or branch's header, or a picked commit's message; the
                // selected operation's header under the project's.
                let shows = match self.project_tab() {
                    ProjectTab::Overview => self.showing_target_commit(&project, cx),
                    ProjectTab::Branches => self.selected_other_branch(&project).is_some(),
                    ProjectTab::History | ProjectTab::Graph => false,
                };
                let message = match shows && self.list_header_shown(&project, cx) {
                    true => self.target_message_height.get(),
                    false => 0.,
                };
                let height = self.project_header_height.get();
                match self.project_tab() {
                    ProjectTab::Overview | ProjectTab::Branches => {
                        height + message + self.list_tabs_height(cx)
                    }
                    ProjectTab::History if self.selected_operation(&project).is_some() => {
                        height + self.header_height.get()
                    }
                    ProjectTab::History | ProjectTab::Graph => height,
                }
            }
            Some(Selection::Checkout(..)) => self.header_height.get(),
            Some(Selection::Uncommitted(..)) if !self.changes_header_shown(cx) => 0.,
            Some(Selection::Uncommitted(project, _)) if self.editing(&project, cx).is_none() => {
                self.header_height.get()
            }
            _ => 0.,
        }
    }

    /// Whether the column beside the diff shows. On a page whose column is its list
    /// (Branches, Operations history, the project's) it always does: without the list,
    /// nothing says what the diff is of. Elsewhere the files show unless `f` hid them, however
    /// narrow the window. A checkout's overview shows beside them, or alone with them hidden,
    /// as its diff does.
    pub fn files_shown(&self, cx: &App) -> bool {
        !self.overview_only() && (self.list_page() || !self.hides_files(cx))
    }

    /// Whether the single file diff is on. The files then stay beside the diff with no toggle,
    /// as the file list is how to go to another.
    pub(crate) fn single_file_diff(&self, cx: &App) -> bool {
        self.store.read(cx).single_file_diff
    }

    /// Whether `f` hid the file list beside the diff, and it isn't pinned there.
    pub(super) fn hides_files(&self, cx: &App) -> bool {
        (self.page.files()).is_some_and(|files| files.files_hidden) && !self.single_file_diff(cx)
    }

    /// Whether the files beside a list page's list are hidden, as they are at first, and
    /// aren't pinned there.
    pub(super) fn hides_beside_files(&self, cx: &App) -> bool {
        (self.page.files()).is_some_and(|files| files.beside_files_hidden)
            && !self.single_file_diff(cx)
    }

    /// How much of the column beside the diff shows at `now`, from 0 to 1, as `f` slides the
    /// file list. A list page's list always shows.
    pub fn files_share(&self, now: Instant, cx: &App) -> f32 {
        match (self.overview_only(), self.list_page()) {
            (true, _) => 0.,
            (false, true) => 1.,
            (false, false) => self.files_slide.share(!self.hides_files(cx), now),
        }
    }

    /// Whether the column beside the diff is drawn at `now`: shown, or still sliding shut.
    fn files_drawn(&self, now: Instant, cx: &App) -> bool {
        self.files_shown(cx) || (!self.overview_only() && self.files_slide.closing(now))
    }

    /// How far right of the detail's left edge the diff starts at `now`: past the column beside
    /// it and a list page's files beside its list, each as much as it shows as it slides.
    pub fn diff_left(&self, now: Instant, window: &Window, cx: &App) -> f32 {
        let files = self.files_width(window, cx) * self.files_share(now, cx);
        let beside = (self.has_files_beside(cx)).then(|| {
            let shown = self.beside_slide.share(!self.hides_beside_files(cx), now);
            self.beside_files_width(cx) * shown
        });
        files + beside.unwrap_or(0.)
    }

    /// Whether the file list, or the files beside a list page's list, slide at `now`.
    pub fn sliding(&self, now: Instant) -> bool {
        self.files_slide.openness(now).is_some()
            || self.beside_slide.openness(now).is_some()
            || self.graph_slide.openness(now).is_some()
    }

    /// Whether what's selected has no changes to list or diff, as a branch with no commits, a
    /// checkout with none uncommitted or a list page with nothing opened, so neither the file
    /// list nor the diff has anything for the keys. Not while its diff loads, which may.
    pub(crate) fn without_changes(&self, cx: &App) -> bool {
        match self.key_for_selection(cx) {
            None => true,
            Some(key) => {
                (self.diff.as_ref()).is_some_and(|diff| diff.key == key && diff.files.is_empty())
            }
        }
    }

    /// Whether the keys are on the diff while it has nothing for them, as they are on a
    /// checkout with no changes once the navigator has gone.
    pub(crate) fn keys_stranded(&self, window: &Window, cx: &App) -> bool {
        self.focus.is_focused(window) && self.without_changes(cx) && !self.showing_graph()
    }

    /// Whether the page is an overview with no column beside it: a worktree's, Inactive and
    /// Network worktrees', as the navigator lists what they sum up, the Graph's with no
    /// commit's changes beside it, and a commit's still being made.
    pub fn overview_only(&self) -> bool {
        (self.showing_graph() && !self.graph_beside())
            || matches!(
                self.selection(),
                Some(
                    Selection::Checkout(..)
                        | Selection::Pending(..)
                        | Selection::Section(_, Section::Inactive | Section::Mesh)
                        | Selection::Terminal(..)
                )
            )
    }

    /// Whether the files `f` shows or hides show: those beside the diff or a checkout's
    /// overview, or on a list page those beside its list. `None` where `f` does nothing.
    pub(crate) fn files_toggled(&self, cx: &App) -> Option<bool> {
        let list_page = self.list_page();
        if self.overview_only()
            || self.single_file_diff(cx)
            || (list_page && !self.has_files_beside(cx))
        {
            return None;
        }
        let files = self.page.files()?;
        Some(match list_page {
            true => !files.beside_files_hidden,
            false => !files.files_hidden,
        })
    }

    /// Show or hide the file list beside the diff or a checkout's overview, or a list page's
    /// files beside its list, as `f` does.
    pub(crate) fn toggle_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hidden) = self.files_toggled(cx) else {
            return;
        };
        self.hide_files(self.list_page(), hidden, cx);
        // Keys go on to the diff once the list they went to is gone, and its filter goes with
        // it, so no file stays out of the diff unseen.
        if hidden {
            if self.files_focus.is_focused(window) {
                window.focus(&self.focus, cx);
            }
            self.close_open_filter(window, cx);
        }
    }

    /// Hide or show the file list beside the diff, or with `beside` the files beside a list
    /// page's list, sliding them there.
    pub(super) fn hide_files(&mut self, beside: bool, hidden: bool, cx: &mut Context<Self>) {
        let Some(files) = self.page.files_mut() else {
            return;
        };
        let (flag, slide) = match beside {
            true => (&mut files.beside_files_hidden, &mut self.beside_slide),
            false => (&mut files.files_hidden, &mut self.files_slide),
        };
        if *flag == hidden {
            return;
        }
        *flag = hidden;
        slide.start(!hidden, cx);
        self.draw_through_slide(cx);
    }

    /// Draw the slide just started, and once more after it's over, as its last frame may be
    /// drawn while it's still moving.
    fn draw_through_slide(&mut self, cx: &mut Context<Self>) {
        self._slid = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FOLD).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        });
        cx.notify();
    }

    /// What a project's page draws beside its list, or in place of it, on the tab shown.
    fn project_beside(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        match self.project_tab() {
            ProjectTab::Overview if self.showing_target_commit(project, cx) => {
                let message = (self.shown_target_commit(project, cx))
                    .filter(|_| self.list_header_shown(project, cx))
                    .map(|commit| self.render_target_message(commit, theme, cx));
                Some(self.list_beside(project, message, theme, window, cx))
            }
            // A target several local branches track is picked where what shipped shows.
            ProjectTab::Overview
                if let Some(choice) = (self.store.read(cx).project(project))
                    .and_then(|entry| entry.target_choice.clone()) =>
            {
                Some(self.target_chooser(project, &choice, theme, cx))
            }
            ProjectTab::Overview if self.showing_shipped(cx) => {
                Some(self.render_shipped(project, window, cx))
            }
            ProjectTab::Overview => None,
            // An opened branch: its header, its files and its pull request, as a merge on the
            // project's page has them, and a commit of it picked shows its message, as a merge's
            // does.
            ProjectTab::Branches if self.showing_branches_overview() => {
                Some(self.render_branches_overview(project, theme, cx))
            }
            ProjectTab::Branches => {
                let header = match self.picked_branch_commit(project) {
                    _ if !self.list_header_shown(project, cx) => None,
                    Some(commit) => Some(self.render_target_message(commit, theme, cx)),
                    None => self.render_other_branch_header(project, theme, cx),
                };
                Some(self.list_beside(project, header, theme, window, cx))
            }
            // What the selected operation did, over the files it changed.
            ProjectTab::History => self.render_operation_header(project, theme, cx),
            ProjectTab::Graph if self.graph_beside() => {
                Some(self.list_beside(project, None, theme, window, cx))
            }
            ProjectTab::Graph => Some(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.graph_view())
                    .into_any_element(),
            ),
        }
    }

    /// The graph, drawn only when it notifies.
    fn graph_view(&self) -> AnyElement {
        (self.graph.clone())
            .cached(gpui::StyleRefinement::default().size_full())
            .into_any_element()
    }

    fn showing_graph(&self) -> bool {
        self.on_tab(ProjectTab::Graph).is_some()
    }

    /// Step the graph's pinned commit to `side`, or the keys past the graph with none there or
    /// while they're on the project's tabs, inside the graph's key context.
    fn step_graph(&mut self, side: Side, window: &mut Window, cx: &mut Context<Self>) {
        let on_graph = self.focus.is_focused(window) || self.side_focus.is_focused(window);
        if !(on_graph && self.graph.update(cx, |graph, cx| graph.step(side, cx))) {
            self.leave(side, window, cx);
        }
    }

    /// Whether the page is a list beside a diff: a project's tab, the Graph's while a commit's
    /// changes are beside it, with the graph as its list.
    pub(crate) fn list_page(&self) -> bool {
        matches!(self.selection(), Some(Selection::Project(_)))
            && (self.project_tab() != ProjectTab::Graph || self.graph_beside())
    }
}

impl Focusable for Detail {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Detail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.page_folds.frame();
        if self.page_folds.any() {
            window.request_animation_frame();
        }
        let theme = Theme::get(cx).clone();
        let selection = self.selection();
        let body: AnyElement = match &selection {
            None if self.store.read(cx).projects.is_empty() => welcome(&self.store, &theme, cx),
            None => empty_state(
                None,
                "Nothing selected",
                "Pick a project, branch or commit on the left",
                &theme,
                cx,
            ),
            Some(selection) => {
                let side = match selection {
                    Selection::Uncommitted(project, checkout) => {
                        self.render_uncommitted_side(project, checkout, &theme, window, cx)
                    }
                    Selection::Commit(project, id) => {
                        self.render_commit_side(project, *id, &theme, window, cx)
                    }
                    Selection::Branch(project, ref_name) => {
                        self.render_branch_side(project, ref_name.as_ref(), &theme, window, cx)
                    }
                    Selection::Stack(project, bottom) => {
                        self.render_stack_side(project, bottom.as_ref(), &theme, window, cx)
                    }
                    Selection::Upstream(..) | Selection::Section(_, Section::Upstream(_)) => {
                        self.render_diff_file_list(&theme, window, cx)
                    }
                    // No column: the overview takes the page.
                    Selection::Checkout(..)
                    | Selection::Pending(..)
                    | Selection::Section(_, Section::Inactive | Section::Mesh)
                    | Selection::Terminal(..) => div().into_any_element(),
                    Selection::Mesh(project, id, pick) => {
                        self.render_mesh_side(project, id, pick, &theme, window, cx)
                    }
                    Selection::Project(project) => match self.project_tab() {
                        ProjectTab::Branches => {
                            self.render_branches_side(project, &theme, window, cx)
                        }
                        ProjectTab::History => {
                            self.render_history_side(project, &theme, window, cx)
                        }
                        // With a commit pinned, the graph is the list, and the keys step it.
                        ProjectTab::Graph if self.graph_beside() => v_flex()
                            .size_full()
                            .track_focus(&self.side_focus)
                            .key_context("Graph")
                            .child(self.graph_view())
                            .into_any_element(),
                        // No column: the graph takes the page.
                        ProjectTab::Graph => div().into_any_element(),
                        ProjectTab::Overview => match self.editing(project, cx) {
                            Some(editing) => empty_state(
                                None,
                                commit_title(&editing.title),
                                format!(
                                    "Commit {} is checked out for editing. Its files are under \
                                     Editing a commit, with Finish and Cancel.",
                                    editing.reference
                                ),
                                &theme,
                                cx,
                            ),
                            None => self.render_upstream_side(project, &theme, window, cx),
                        },
                    },
                };
                let width = self.files_width(window, cx);
                // A page's header runs across it, over its files and its diff, as but.dev's
                // does. `view_header_height` lists the same pages.
                let header = match selection {
                    Selection::Commit(project, id) if self.shows_picks(project, *id, cx) => {
                        Some(self.render_picks_header(project, *id, &theme, cx))
                    }
                    Selection::Commit(project, id)
                        if !self.commit_header_shown(project, *id, cx) =>
                    {
                        None
                    }
                    Selection::Commit(project, id) => {
                        Some(self.render_commit_header(project, *id, &theme, cx))
                    }
                    Selection::Branch(..) | Selection::Stack(..)
                        if !self.branch_header_shown(cx) =>
                    {
                        None
                    }
                    Selection::Branch(project, ref_name) => Some(self.render_branch_header(
                        project,
                        ref_name.as_ref(),
                        &theme,
                        window,
                        cx,
                    )),
                    Selection::Stack(project, bottom) => {
                        Some(self.render_stack_header(project, bottom.as_ref(), &theme, cx))
                    }
                    Selection::Upstream(..) if !self.upstream_header_shown(cx) => None,
                    Selection::Upstream(project, ref_name, id) => Some(
                        self.render_upstream_header(project, ref_name, *id, &theme, window, cx),
                    ),
                    Selection::Project(_) => {
                        Some(self.render_project_header(selection, &theme, window, cx))
                    }
                    Selection::Checkout(project, checkout) => {
                        Some(self.render_checkout_header(project, checkout, false, &theme, cx))
                    }
                    Selection::Uncommitted(..) if !self.changes_header_shown(cx) => None,
                    Selection::Uncommitted(project, checkout)
                        if self.editing(project, cx).is_none() =>
                    {
                        Some(self.render_checkout_header(project, checkout, true, &theme, cx))
                    }
                    _ => None,
                };
                // Beside a list, the selected branch or commit's tabs when it has a pull
                // request, over the pull request or the diff.
                let beside = match selection {
                    Selection::Project(project) => self.project_beside(project, &theme, window, cx),
                    Selection::Section(project, Section::Inactive) => {
                        Some(self.render_inactive_overview(project, &theme, cx))
                    }
                    Selection::Section(project, Section::Mesh) => {
                        Some(self.render_machines_overview(project, &theme, cx))
                    }
                    Selection::Checkout(project, checkout) => {
                        Some(self.render_now(project, checkout, cx))
                    }
                    // Its page is the commit's, once it's made.
                    Selection::Pending(project, op, _) => {
                        let title = self.store.read(cx).pending_commit(project, *op);
                        Some(empty_state(
                            None,
                            commit_title(&title.map(|(title, _)| title).unwrap_or_default()),
                            "Its page opens once it's made",
                            &theme,
                            cx,
                        ))
                    }
                    _ => None,
                };
                let (files_shown, list_page) = (self.files_shown(cx), self.list_page());
                let column_pane = self.column_pane();
                // A list that went while it had the keys hands them to the diff, as `f` does.
                let gone = (!self.filtering
                    && self.file_filter.focus_handle(cx).is_focused(window))
                    || (self.find.is_none() && self.find_input.focus_handle(cx).is_focused(window));
                if gone
                    || (!files_shown && self.files_focus.is_focused(window))
                    || (!list_page && self.side_focus.is_focused(window))
                {
                    let focus = self.focus.clone();
                    window.defer(cx, move |window, cx| window.focus(&focus, cx));
                }
                // The file list sliding beside an overview is cut to as much as shows, drawn again
                // as it goes. Under the diff it stays as it is, and the diff slides over it.
                let now = clock::now();
                let cut =
                    (self.files_slide.openness(now)).filter(|_| !list_page && self.diff_hidden(cx));
                if cut.is_some() || self.graph_slide.openness(now).is_some() {
                    window.request_animation_frame();
                }
                let panes = match selection {
                    Selection::Branch(project, _) | Selection::Upstream(project, ..)
                        if self.showing_review(cx) =>
                    {
                        self.render_review_tab(project, &theme, cx)
                    }
                    Selection::Branch(..) if self.showing_brief() => {
                        self.render_brief_tab(&theme, window, cx)
                    }
                    _ => h_flex()
                        .w_full()
                        .flex_1()
                        .min_h_0()
                        .items_start()
                        .on_drag_move(resize_pane(column_pane, 0., self.store.clone()))
                        .when(self.files_drawn(now, cx), |el| {
                            let column = v_flex()
                                .w(dp(width))
                                .flex_none()
                                .relative()
                                .h_full()
                                .border_r_1()
                                .border_color(theme.border_3)
                                // A list page's list takes the arrows itself, and the files
                                // beside it hold their focus there.
                                .when(!list_page, |el| {
                                    el.track_focus(&self.files_focus).key_context(
                                        match self.composing {
                                            true => "Files Composer",
                                            false => "Files",
                                        },
                                    )
                                })
                                .child(side)
                                .child(resize_handle(column_pane, self.store.clone(), &theme));
                            let column = inert(column, !files_shown);
                            el.child(match cut {
                                Some(open) => div()
                                    .flex_none()
                                    .relative()
                                    .h_full()
                                    .w(dp(width * open))
                                    .overflow_hidden()
                                    .child(column)
                                    .child(edge(theme.border_3).right_0()),
                                None => column,
                            })
                        })
                        // `DiffPane` draws the diff over this space.
                        .child(beside.unwrap_or_else(|| div().flex_1().into_any_element()))
                        .into_any_element(),
                };
                v_flex()
                    .size_full()
                    .children(header)
                    .child(panes)
                    .into_any_element()
            }
        };
        let context = self.key_context(cx);
        v_flex()
            .id("detail")
            .key_context(context)
            .map(|el| self.branches_page_actions(el, window, cx))
            .map(|el| self.history_actions(el, cx))
            .map(|el| self.upstream_page_actions(el, cx))
            .map(|el| self.change_actions(el, cx))
            .map(|el| self.brief_actions(el, cx))
            .when(self.showing_graph(), |el| {
                el.on_action(cx.listener(|this, _: &graph::StepUp, window, cx| {
                    this.step_graph(Side::Top, window, cx)
                }))
                .on_action(cx.listener(|this, _: &graph::StepDown, window, cx| {
                    this.step_graph(Side::Bottom, window, cx)
                }))
                .on_action(cx.listener(|this, _: &graph::StepLeft, window, cx| {
                    this.step_graph(Side::Left, window, cx)
                }))
                .on_action(cx.listener(
                    |this, _: &graph::StepRight, window, cx| {
                        this.step_graph(Side::Right, window, cx)
                    },
                ))
            })
            // Always: what it takes is asked as it runs, as the cursor can come into sight with
            // no draw of this view, and where there's nothing it mustn't go on to the row.
            .on_action(cx.listener(|this, _: &Discard, window, cx| this.discard_here(window, cx)))
            // The file under the cursor, while the file list or the diff has the keys.
            .when_some(
                self.focused_file_paths(window, cx),
                |el, (relative, absolute)| {
                    let revealed = std::path::PathBuf::from(&absolute);
                    let el = clipboard::copy_on::<CopyRelativePath>(el, &self.store, relative);
                    clipboard::copy_on::<CopyAbsolutePath>(el, &self.store, absolute).on_action(
                        cx.listener(move |_, _: &crate::navigator::Reveal, _, cx| {
                            cx.reveal_path(&revealed)
                        }),
                    )
                },
            )
            .when(self.shows_conflicts(), |el| resolve::conflict_keys(el, cx))
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NextFile, window, cx| {
                let folders = this.folder_stops(window, cx);
                match this.on_last_entry(folders, cx) {
                    true => this.leave(Side::Bottom, window, cx),
                    false => this.move_file(1, folders, cx),
                }
            }))
            // With none over it, the file list's keys go on from it, and the diff's as ↑ does, up
            // its hunks and out of its top.
            .on_action(cx.listener(|this, _: &PreviousFile, window, cx| {
                let folders = this.folder_stops(window, cx);
                let first = this.on_first_entry(folders, cx);
                if first && this.files_focus.is_focused(window) {
                    this.leave(Side::Top, window, cx)
                } else if first && this.focus.is_focused(window) {
                    this.move_hunk(false, window, cx)
                } else {
                    this.move_file(-1, folders, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &FirstFile, window, cx| {
                match this.on_folder_summary(window, cx) {
                    true => this.step_summary(isize::MIN / 2, window, cx),
                    false => this.move_file(isize::MIN / 2, false, cx),
                }
            }))
            .on_action(cx.listener(|this, _: &LastFile, window, cx| {
                match this.on_folder_summary(window, cx) {
                    true => this.step_summary(isize::MAX / 2, window, cx),
                    false => this.move_file(isize::MAX / 2, false, cx),
                }
            }))
            .on_action(cx.listener(|this, _: &PickNextFile, _, cx| this.pick_step(1, cx)))
            .on_action(cx.listener(|this, _: &PickPreviousFile, _, cx| this.pick_step(-1, cx)))
            .on_action(cx.listener(|this, _: &PickAllFiles, _, cx| this.pick_all_files(cx)))
            .on_action(
                cx.listener(|this, _: &NextHunk, window, cx| this.move_hunk(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PreviousHunk, window, cx| this.move_hunk(false, window, cx)),
            )
            .on_action(cx.listener(|this, _: &FoldFile, _, cx| this.fold_cursor(true, cx)))
            .on_action(cx.listener(|this, _: &UnfoldFile, _, cx| this.fold_cursor(false, cx)))
            // On its header, or its row in the file list, or scrolled to past the cursor, not
            // from inside one of its hunks.
            .on_action(cx.listener(|this, _: &ToggleFileFold, window, cx| {
                if this.on_folder_summary(window, cx) {
                    return this.open_summary_file(cx);
                }
                if let Some(dir) = this.cursor_folder(cx) {
                    return this.fold_dir(dir.to_owned(), None, cx);
                }
                let in_hunk = this.cursor.at != Stop::File && this.reading.is_none();
                if in_hunk && !this.files_focused(window) {
                    return;
                }
                let file = this.cursor_path();
                let folded = (file.zip(this.page.files()))
                    .is_some_and(|(path, files)| files.folded.contains(&path));
                this.fold_cursor_file(!folded, cx)
            }))
            .on_action(cx.listener(|this, _: &PageDown, window, cx| this.page(PAGE, window, cx)))
            .on_action(cx.listener(|this, _: &PageUp, window, cx| this.page(-PAGE, window, cx)))
            .on_action(cx.listener(|this, _: &HalfPageDown, window, cx| this.page(0.5, window, cx)))
            .on_action(cx.listener(|this, _: &HalfPageUp, window, cx| this.page(-0.5, window, cx)))
            .on_action(
                cx.listener(|this, _: &ScrollDown, _, cx| this.scroll_lines(SCROLL_LINES, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ScrollUp, _, cx| this.scroll_lines(-SCROLL_LINES, cx)),
            )
            .on_action(cx.listener(|this, _: &SwitchTab, window, cx| {
                if !this.switch_tab(true, window, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &PreviousTab, window, cx| {
                if !this.switch_tab(false, window, cx) {
                    cx.propagate();
                }
            }))
            // A project's page steps through its own tabs, round again past either end. Other
            // pages have none.
            .on_action(cx.listener(|this, _: &NextProjectPage, window, cx| {
                this.show_tab(this.project_tab().round(true), window, cx)
            }))
            .on_action(cx.listener(|this, _: &PreviousProjectPage, window, cx| {
                this.show_tab(this.project_tab().round(false), window, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowBrief, window, cx| this.show_brief(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleWrap, _, cx| this.toggle_wrap(cx)))
            .on_action(
                cx.listener(|this, _: &EditLines, window, cx| this.edit_cursor_hunk(window, cx)),
            )
            .on_action(cx.listener(|this, _: &SaveEdits, _, cx| this.save_now(cx)))
            .on_action(cx.listener(|this, _: &FinishEdit, window, cx| this.leave_edit(window, cx)))
            .on_action(cx.listener(|this, _: &TogglePlainLines, _, cx| this.toggle_plain_lines(cx)))
            .on_action(cx.listener(|this, _: &ToggleSplit, _, cx| this.toggle_split(cx)))
            .on_action(
                cx.listener(|this, _: &FilterFiles, window, cx| this.open_filter(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleFiles, window, cx| this.toggle_files(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &find::FindInDiff, window, cx| this.open_find(window, cx)),
            )
            .on_action(cx.listener(|this, _: &find::FindNext, _, cx| this.step_find(1, cx)))
            .on_action(cx.listener(|this, _: &find::FindPrevious, _, cx| this.step_find(-1, cx)))
            .on_action(
                cx.listener(|this, _: &crate::open_in::OpenInEditor, window, cx| {
                    if this.cursor_folder(cx).is_none() {
                        this.open_in_editor(window, cx)
                    }
                }),
            )
            .when(self.composing, |el| {
                el.on_action(cx.listener(|this, _: &ToggleCheck, window, cx| {
                    if this.cursor_folder(cx).is_none() {
                        this.toggle_cursor_check(window, cx)
                    }
                }))
                .on_action(cx.listener(|this, _: &CheckAll, _, cx| this.set_all_checked(true, cx)))
                .on_action(
                    cx.listener(|this, _: &UncheckAll, _, cx| this.set_all_checked(false, cx)),
                )
                .on_action(
                    cx.listener(|this, _: &WriteMessage, window, cx| {
                        this.focus_message(window, cx)
                    }),
                )
            })
            .size_full()
            // The ground the home tile's card is on, as the navigator's cards are; a pinned
            // page is inside its card.
            .bg(match self.pinned {
                Some(_) => theme.bg_1,
                None => theme.bg_2,
            })
            // A pinned page goes in its tile, under the location bar across the home tile's.
            .when(
                self.pinned.is_none() && self.location_bar_height() > 0.,
                |el| el.child(self.render_header(selection.as_ref(), &theme, window, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(match self.pinned {
                        Some(_) => div().absolute().inset_0().child(body),
                        // On its card, in from the window's edges as tiles are from each other.
                        None => crate::ui::inset_by(div(), crate::ui::TILE_MARGIN)
                            .left(dp(self.room_left()))
                            .top(dp(self.room_top()))
                            .child({
                                let tile = placed(self.tile_area(cx))
                                    .child(crate::ui::tile_card(&theme))
                                    .child(
                                        crate::ui::tile_content().child(
                                            (div().absolute().inset_0())
                                                .top(dp(self.bar_inset(cx)))
                                                .child(body),
                                        ),
                                    );
                                // Dragged by its bar or zoomed, over the tiles beside it.
                                match self.store.read(cx).lifted_tile() == Some(&Tile::Home) {
                                    true => crate::ui::fold::on_top(tile).into_any_element(),
                                    false => tile.into_any_element(),
                                }
                            }),
                    }),
            )
    }
}

/// Where the file list's keys stop in a tree of `paths` with the directories in `collapsed`
/// folded shut, in its order: the files it lists, by their position in `paths`, and with
/// `folders` its folders. A row the tree merges, as `src/backend`, is one folder, and folds
/// by its own path.
fn tree_stops(paths: &[&str], collapsed: &HashSet<String>, folders: bool) -> Vec<ListStop> {
    (file_tree::tree_rows(paths, collapsed).into_iter())
        .filter_map(|row| match row {
            file_tree::TreeRow::File { pos, .. } => Some(ListStop::Entry(Entry::File(pos))),
            file_tree::TreeRow::Dir { path, .. } => folders.then_some(ListStop::Folder(path)),
        })
        .collect()
}

/// Whether `path` is in folder `dir`, at any depth.
fn in_dir(path: &str, dir: &str) -> bool {
    (path.strip_prefix(dir)).is_some_and(|rest| rest.starts_with('/'))
}

/// Whether `path` has the letters of every one of `terms` in it, in order, as the palette
/// matches. Without what [`score`] works out to underline, as it's asked of every file a frame.
fn keeps_path(terms: &[String], path: &str) -> bool {
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    terms.iter().all(|term| {
        let mut letters = path.chars().map(lower);
        term.chars()
            .map(lower)
            .all(|wanted| letters.any(|c| c == wanted))
    })
}

/// How well `terms` match `path`, and the byte ranges of it they matched, to underline. A term
/// is looked for in the file's name first, where it counts for more than in its folders, and
/// either way ranks as [`score`] ranks the palette's choices. `None` when `path` isn't kept.
fn path_match(terms: &[String], path: &str) -> Option<(i32, Vec<Range<usize>>)> {
    let (name, _) = diff::split_path(path);
    let name_at = path.len() - name.len();
    let (mut total, mut found) = (0, Vec::new());
    for term in terms {
        let (points, ranges) = match score(name, term) {
            Some((points, ranges)) => (
                points + IN_NAME,
                (ranges.into_iter())
                    .map(|range| range.start + name_at..range.end + name_at)
                    .collect(),
            ),
            None => score(path, term)?,
        };
        total += points;
        found.extend(ranges);
    }
    Some((total, merged(found)))
}

/// What a term found in a file's name counts for over one found only in its folders: more than
/// [`score`] gives any match, so a file whose name has it comes first.
const IN_NAME: i32 = 4000;

/// `items` as the open file filter lists them, each with what it found in its `path`: those it
/// keeps, the best matches first, as [`path_match`] ranks them, and the rest in the order they
/// came.
fn ranked<T>(
    terms: &[String],
    items: Vec<T>,
    path: impl Fn(&T) -> &str,
) -> Vec<(T, Vec<Range<usize>>)> {
    let mut ranked: Vec<(T, i32, Vec<Range<usize>>)> = (items.into_iter())
        .filter_map(|item| {
            let (points, found) = path_match(terms, path(&item))?;
            Some((item, points, found))
        })
        .collect();
    ranked.sort_by_key(|(_, points, _)| std::cmp::Reverse(*points));
    (ranked.into_iter())
        .map(|(item, _, found)| (item, found))
        .collect()
}

/// `ranges` in order, those that overlap or touch joined, as [`crate::ui::matched`] takes them.
fn merged(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if last.end >= range.start => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// What the app shows before it has a project: what it's for and the two ways in, then the
/// CLI for agents, with the docs, GitButler's Discord and the usage notice at the bottom, out
/// of the way.
fn welcome(store: &Entity<Store>, theme: &Theme, cx: &App) -> AnyElement {
    let link = |id: &'static str, label: &'static str, icon: Icon, action: &dyn gpui::Action| {
        button(id, label)
            .icon(icon)
            .style(ButtonStyle::Ghost)
            .small()
            .dispatch(action)
    };
    v_flex()
        .size_full()
        .items_center()
        .gap(dp(24.))
        .child(
            v_flex()
                .flex_1()
                // So a short pane shrinks the drawing rather than clip it.
                .min_h_0()
                .items_center()
                .justify_center()
                .gap(dp(10.))
                .child(illustration(Illustration::Waving, theme, cx).mb(dp(14.)))
                .child(
                    div()
                        .text_size(dp(18.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text_1)
                        .child("Welcome to GitButler"),
                )
                .child(
                    div()
                        .max_w(dp(420.))
                        .text_center()
                        .text_size(dp(13.))
                        .text_color(theme.text_2)
                        .child(
                            "Add a repository your coding agents work in. What they change shows \
                             up here as it happens, to review, commit and stack into branches.",
                        ),
                )
                .child(
                    h_flex()
                        .mt(dp(6.))
                        .gap(dp(8.))
                        .child(
                            button("welcome-add", "Add a repository…")
                                .style(ButtonStyle::Pop)
                                .tooltip("Pick a folder with a Git repository")
                                .dispatch(&AddProject),
                        )
                        .child(
                            button("welcome-clone", "Clone a repository…")
                                .dispatch(&crate::projects::CloneRepository),
                        ),
                )
                .children(
                    crate::settings::welcome_card(store, theme, cx).map(|card| card.mt(dp(28.))),
                ),
        )
        .child(
            v_flex()
                .items_center()
                .gap(dp(6.))
                .pb(dp(16.))
                .child(
                    h_flex()
                        .gap(dp(4.))
                        .child(link(
                            "welcome-docs",
                            "Docs",
                            Icon::Docs,
                            &crate::help::OpenDocs,
                        ))
                        .child(link(
                            "welcome-discord",
                            "Discord",
                            Icon::Discord,
                            &crate::help::OpenDiscord,
                        )),
                )
                .child(usage_notice(theme)),
        )
        .into_any_element()
}

/// The welcome's last line, as VS Code's welcome page ends: that GitButler collects usage
/// data, with its privacy policy and the switches that turn it off a click away.
fn usage_notice(theme: &Theme) -> AnyElement {
    let mut text = String::from("GitButler collects usage data. Read our ");
    let link = |words: &str, text: &mut String| {
        let at = text.len();
        text.push_str(words);
        at..text.len()
    };
    let policy = link("privacy policy", &mut text);
    text.push_str(" and learn how to ");
    let opt_out = link("opt out", &mut text);
    text.push('.');
    let linked = gpui::HighlightStyle {
        underline: Some(gpui::UnderlineStyle {
            thickness: fixed(1.),
            ..Default::default()
        }),
        ..Default::default()
    };
    let styled = gpui::StyledText::new(text)
        .with_highlights([(policy.clone(), linked), (opt_out.clone(), linked)]);
    // One line the column centers, not `text_center`: GPUI maps a click on centered text as if
    // the line were left-aligned, so the links would miss.
    div()
        .text_size(dp(12.))
        .text_color(theme.text_3)
        .child(gpui::InteractiveText::new("usage-notice", styled).on_click(
            vec![policy, opt_out],
            |link, window, cx| match link {
                0 => cx.open_url("https://gitbutler.com/privacy"),
                _ => window.dispatch_action(Box::new(crate::settings::OpenSettings), cx),
            },
        ))
        .into_any_element()
}

fn hunk_keys(file: &FileDiff) -> Vec<HunkKey> {
    file.hunks().iter().map(|hunk| hunk.key).collect()
}

/// How far to scroll the diff to bring rows running from `top` to `bottom`, down from the
/// view's top, as far into view as they fit, as the TUI's details scroll to a section. The view
/// is `height` tall, above `clear` the sticky header hides it, and `room` is left below them.
/// Taller than what's left, or starting above it, they go to its top; ending below it, their
/// end comes up to `room` above its bottom; in view already, they stay put.
fn reveal_by(top: Pixels, bottom: Pixels, clear: Pixels, room: Pixels, height: Pixels) -> Pixels {
    let end = height - room;
    if bottom - top > end - clear || top < clear {
        top - clear
    } else if bottom > end {
        bottom - end
    } else {
        Pixels::ZERO
    }
}

/// Where the hunk keys go from row `from` of `rows`, `forward` or back: the next row `stop`
/// says the cursor stops on, and that row. Headers are stops too, so the keys go back up to one
/// they left and down to the next file's rather than past it.
fn next_stop(
    rows: &[DiffRow],
    from: usize,
    forward: bool,
    stop: impl Fn(usize, DiffRow) -> Option<Stop>,
) -> Option<(Cursor, usize)> {
    let at = |row: usize| {
        let entry = *rows.get(row)?;
        let cursor = match stop(row, entry)? {
            Stop::Message => Cursor::on_message(),
            at => Cursor {
                file: entry.file()?,
                at,
            },
        };
        Some((cursor, row))
    };
    match forward {
        true => (from + 1..rows.len()).find_map(at),
        false => (0..from.min(rows.len())).rev().find_map(at),
    }
}

/// Where hunk `ix` of a file's `old` hunks is among its hunks `now`. A hunk keeps its lines
/// of the old side while hunks before it come and go and shift its new side. When it went
/// too, say discarded, it's what moved up into its place.
fn follow_hunk(old: &[HunkKey], ix: usize, now: &[HunkKey]) -> Option<usize> {
    let lines = |key: &HunkKey| (key.0.old_start, key.0.old_lines);
    (old.get(ix)
        .and_then(|key| now.iter().position(|k| lines(k) == lines(key))))
    .or_else(|| Some(ix.min(now.len().checked_sub(1)?)))
}

/// The subscription to a side list's filter: typing keeps the list's selection among what it
/// shows, Enter goes on to the list as filtered, and Esc empties it, then goes back to the
/// list, as the navigator's filter does.
fn list_filter(
    input: &Entity<TextInput>,
    select_shown: fn(&mut Detail, &mut Context<Detail>),
    window: &mut Window,
    cx: &mut Context<Detail>,
) -> Subscription {
    cx.subscribe_in(
        input,
        window,
        move |this, input, event, window, cx| match event {
            InputEvent::Changed => {
                select_shown(this, cx);
                cx.notify()
            }
            InputEvent::Cancel if input.read(cx).text().is_empty() => {
                window.focus(&this.side_focus, cx)
            }
            InputEvent::Cancel => input.update(cx, |input, cx| input.set_text("", cx)),
            InputEvent::Submit => window.focus(&this.side_focus, cx),
        },
    )
}

/// A line of code's height, at the interface scale, for rows not drawn yet.
fn code_line(cx: &App) -> Pixels {
    let theme = Theme::get(cx);
    theme.scaled(theme.code_line)
}

/// Whether `shown`, a diff's files in the order its list draws them, are `files` as they are
/// now, in the order the workspace lists them: a tree puts folders first, and status puts
/// conflicted files first.
fn same_files<'a>(
    shown: impl ExactSizeIterator<Item = &'a FileChange>,
    files: &[FileChange],
) -> bool {
    if shown.len() != files.len() {
        return false;
    }
    let shown: HashMap<&str, &FileChange> = shown.map(|file| (file.path.as_str(), file)).collect();
    files.iter().all(|now| {
        shown.get(now.path.as_str()).is_some_and(|shown| {
            (shown.modified, shown.size) == (now.modified, now.size)
                && shown.base() == now.base()
                && shown.conflicted == now.conflicted
        })
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use gpui::Pixels;

    use super::{
        Cursor, Entry, ListStop, Stop, follow_hunk, keeps_path, next_stop, path_match, ranked,
        reveal_by, same_files, tree_stops,
    };
    use crate::backend::FileChange;
    use crate::ui::diff::DiffRow;

    #[test]
    fn the_file_filter_keeps_paths_with_each_terms_letters_in_order() {
        let terms =
            |text: &str| -> Vec<String> { text.split_whitespace().map(Into::into).collect() };
        let path = "crates/but-gpui/src/settings/metrics.rs";
        for (filter, path, kept, why) in [
            ("", path, true, "an empty filter keeps everything"),
            (
                "setmet",
                path,
                true,
                "letters needn't be next to each other",
            ),
            ("METRICS", path, true, "case doesn't matter"),
            ("rs gpui", path, true, "terms needn't come in order"),
            (
                "über",
                "docs/Über.md",
                true,
                "case doesn't matter beyond ASCII",
            ),
            ("tem", "src/mod.rs", false, "letters must come in order"),
            ("mod xyz", "src/mod.rs", false, "every term must match"),
        ] {
            assert_eq!(keeps_path(&terms(filter), path), kept, "{why}");
            assert_eq!(
                path_match(&terms(filter), path).is_some(),
                kept,
                "a row the filter keeps has its letters to underline: {why}"
            );
        }
    }

    #[test]
    fn the_file_filter_underlines_each_terms_letters_once_in_order() {
        let terms =
            |text: &str| -> Vec<String> { text.split_whitespace().map(Into::into).collect() };
        let underlined = |path: &'static str, filter| -> Option<Vec<&str>> {
            let (_, found) = path_match(&terms(filter), path)?;
            found.into_iter().map(|range| path.get(range)).collect()
        };
        assert_eq!(
            underlined("src/settings/metrics.rs", "metrics"),
            Some(vec!["metrics"]),
            "a name typed in full underlines that name"
        );
        assert_eq!(
            underlined("src/detail/mod.rs", "dmod"),
            Some(vec!["d", "mod"]),
            "scattered letters underline each, neighbours as one"
        );
        assert_eq!(
            underlined("src/detail/mod.rs", "mod.rs mo"),
            Some(vec!["mod.rs"]),
            "terms finding the same letters underline them once"
        );
        assert_eq!(
            underlined("src/detail/mod.rs", "rs src"),
            Some(vec!["src", "rs"]),
            "terms typed out of order underline in the path's order"
        );
        assert_eq!(
            underlined("src/mod.rs", "xyz"),
            None,
            "a path not kept underlines nothing"
        );
        assert_eq!(
            (path_match(&terms("mod"), "src/model/mod.rs"))
                .and_then(|(_, found)| Some(found.first()?.start)),
            Some(10),
            "letters in the file's name are underlined there, not in a folder before it"
        );
    }

    #[test]
    fn the_file_filter_lists_files_named_for_it_first() {
        let terms =
            |text: &str| -> Vec<String> { text.split_whitespace().map(Into::into).collect() };
        let paths = vec![
            "assets/keymaps/default.json",
            "src/main.rs",
            "src/key_map.rs",
            "src/keymap.rs",
        ];
        let listed = |filter| -> Vec<&str> {
            (ranked(&terms(filter), paths.clone(), |path| *path).into_iter())
                .map(|(path, _)| path)
                .collect()
        };
        assert_eq!(
            listed("keymap"),
            [
                "src/keymap.rs",
                "src/key_map.rs",
                "assets/keymaps/default.json"
            ],
            "a name with all of it together, then one with its letters apart, then a folder's"
        );
        assert_eq!(listed(""), paths, "with nothing typed, the order picked");
        assert_eq!(
            listed("src"),
            ["src/main.rs", "src/key_map.rs", "src/keymap.rs"],
            "as well matched, in the order picked"
        );
    }

    #[test]
    fn a_diff_in_tree_order_shows_the_files_status_lists_by_path() {
        let files = [FileChange::bare("src/a.rs"), FileChange::bare("src/b/x.rs")];
        let tree = [FileChange::bare("src/b/x.rs"), FileChange::bare("src/a.rs")];
        assert!(
            same_files(tree.iter(), &files),
            "a tree draws the folder before the file beside it, which sorts first by path"
        );
        let written = FileChange {
            modified: Some(1),
            ..FileChange::bare("src/a.rs")
        };
        assert!(
            !same_files(tree.iter(), &[written, FileChange::bare("src/b/x.rs")]),
            "a file written since the diff was read isn't shown as it is"
        );
        assert!(
            !same_files(tree.iter().take(1), &files),
            "nor is a file the diff doesn't have"
        );
    }
    use crate::backend::HunkKey;

    #[test]
    fn the_keys_skip_only_files_the_tree_folds_away() {
        let collapsed =
            |dirs: &[&str]| -> HashSet<String> { dirs.iter().map(|dir| dir.to_string()).collect() };
        let file = |pos| ListStop::Entry(Entry::File(pos));
        assert_eq!(
            tree_stops(&["src/backend/x.rs"], &collapsed(&["src"]), false),
            [file(0)],
            "`src` holds only `backend`, so the tree has one `src/backend` row, not folded"
        );
        assert_eq!(
            tree_stops(&["src/backend/x.rs"], &collapsed(&["src/backend"]), false),
            [],
            "the merged row folds by its own path"
        );
        assert_eq!(
            tree_stops(
                &["src/backend/x.rs", "src/y.rs", "z.rs"],
                &collapsed(&["src"]),
                false
            ),
            [file(2)],
            "`src` has a file of its own here, so it's a row of its own, folding all under it"
        );
    }

    #[test]
    fn the_keys_stop_on_folders_when_asked() {
        let none = HashSet::new();
        let paths = ["src/backend/x.rs", "src/y.rs", "z.rs"];
        let file = |pos| ListStop::Entry(Entry::File(pos));
        let folder = |path: &str| ListStop::Folder(path.to_owned());
        assert_eq!(
            tree_stops(&paths, &none, true),
            [
                folder("src"),
                folder("src/backend"),
                file(0),
                file(1),
                file(2)
            ],
            "each folder comes before what's in it, as the tree lists them"
        );
        let shut = HashSet::from(["src".to_owned()]);
        assert_eq!(
            tree_stops(&paths, &shut, true),
            [folder("src"), file(2)],
            "a folded folder is still a stop, though what's in it isn't"
        );
        assert_eq!(
            tree_stops(&["src/backend/x.rs"], &none, true),
            [folder("src/backend"), file(0)],
            "a merged row is one stop"
        );
    }

    fn key(old_start: u32, new_start: u32) -> HunkKey {
        HunkKey(but_core::HunkHeader {
            old_start,
            old_lines: 2,
            new_start,
            new_lines: 3,
        })
    }

    #[test]
    fn the_cursor_stays_on_its_hunk_as_hunks_around_it_go() {
        let old = [key(1, 1), key(10, 11), key(20, 22)];
        assert_eq!(
            follow_hunk(&old, 1, &[key(10, 10), key(20, 21)]),
            Some(0),
            "the first hunk was discarded, shifting the new side of the one under the cursor"
        );
        assert_eq!(
            follow_hunk(&old, 1, &[key(1, 1), key(20, 21)]),
            Some(1),
            "the hunk under the cursor was discarded: the next one moved up into its place"
        );
        assert_eq!(
            follow_hunk(&old, 2, &[key(1, 1)]),
            Some(0),
            "the last hunk went: the one before it"
        );
        assert_eq!(follow_hunk(&old, 0, &[]), None, "no hunks left");
    }

    #[test]
    fn the_hunk_keys_stop_on_file_headers_and_hunk_boxes_too() {
        let line = |file, hunk, line| DiffRow::Line { file, hunk, line };
        let rows = [
            DiffRow::File(0),
            DiffRow::Hunk { file: 0, hunk: 0 },
            line(0, 0, 0),
            line(0, 0, 1),
            line(0, 0, 2),
            DiffRow::Hunk { file: 0, hunk: 1 },
            line(0, 1, 1),
            DiffRow::File(1),
            DiffRow::Hunk { file: 1, hunk: 0 },
            line(1, 0, 1),
        ];
        // Each hunk's changes start at its line 1, after an unchanged one, and the second hunk
        // of the first file has no box.
        let stop = |_, entry| match entry {
            DiffRow::File(_) => Some(Stop::File),
            DiffRow::Hunk { file: 0, hunk: 1 } => None,
            DiffRow::Hunk { hunk, .. } => Some(Stop::Gap(hunk)),
            DiffRow::Line { hunk, line: 1, .. } => Some(Stop::Hunk(hunk)),
            _ => None,
        };
        let at = |file, at, row| Some((Cursor { file, at }, row));
        assert_eq!(
            next_stop(&rows, 0, true, stop),
            at(0, Stop::Gap(0), 1),
            "from a file's header to the box above its first hunk"
        );
        assert_eq!(
            next_stop(&rows, 1, true, stop),
            at(0, Stop::Hunk(0), 3),
            "from the box to the hunk's first change, past the unchanged line before it"
        );
        assert_eq!(
            next_stop(&rows, 3, true, stop),
            at(0, Stop::Hunk(1), 6),
            "on to the next hunk's changes, as it has no box"
        );
        assert_eq!(
            next_stop(&rows, 3, false, stop),
            at(0, Stop::Gap(0), 1),
            "back up to the box above the hunk"
        );
        assert_eq!(
            next_stop(&rows, 6, true, stop),
            at(1, Stop::File, 7),
            "from a file's last hunk to the next file's header, not past it to its hunk"
        );
        assert_eq!(
            next_stop(&rows, 7, false, stop),
            at(0, Stop::Hunk(1), 6),
            "from a header back to the last hunk of the file before"
        );
        assert_eq!(
            next_stop(&rows, 0, false, stop),
            None,
            "nothing above the first header"
        );
        assert_eq!(
            next_stop(&rows, 9, true, stop),
            None,
            "nothing below the last hunk"
        );
    }

    #[test]
    fn the_hunk_keys_stop_on_the_commit_message_above_the_files() {
        let rows = [
            DiffRow::Message,
            DiffRow::Gap,
            DiffRow::File(0),
            DiffRow::Hunk { file: 0, hunk: 0 },
        ];
        let stop = |_, entry| match entry {
            DiffRow::Message => Some(Stop::Message),
            DiffRow::File(_) => Some(Stop::File),
            _ => None,
        };
        assert_eq!(
            next_stop(&rows, 2, false, stop),
            Some((Cursor::on_message(), 0)),
            "up from the first file's header to the message, which is in no file"
        );
        assert_eq!(
            next_stop(&rows, 0, true, stop),
            Some((Cursor::on_file(0), 2)),
            "down from the message to the first file's header, past the gap"
        );
    }

    #[test]
    fn a_stop_scrolls_as_far_into_view_as_it_fits() {
        let px = Pixels::from;
        // A view 500 tall, its top 40 under the sticky header, and 20 left below what shows.
        let by = |top, bottom| reveal_by(px(top), px(bottom), px(40.), px(20.), px(500.));
        assert_eq!(by(100., 300.), px(0.), "in view already, it stays put");
        assert_eq!(
            by(400., 600.),
            px(120.),
            "ending below the view, its end comes up to the room left at the bottom"
        );
        assert_eq!(
            by(100., 490.),
            px(10.),
            "in view but into the room at the bottom, it comes up out of it"
        );
        assert_eq!(
            by(-100., 100.),
            px(-140.),
            "starting above the view, its start comes down to below the sticky header"
        );
        assert_eq!(
            by(20., 100.),
            px(-20.),
            "under the sticky header counts as above the view"
        );
        assert_eq!(
            by(300., 1000.),
            px(260.),
            "taller than the view, its start goes to the top rather than its end to the bottom"
        );
        assert_eq!(
            by(300., 750.),
            px(260.),
            "fitting the view but not with the room below, its start goes to the top"
        );
        assert_eq!(
            reveal_by(px(0.), px(600.), px(0.), px(20.), px(500.)),
            px(0.),
            "a file taller than the view, its header at the top, stays put"
        );
    }
}
