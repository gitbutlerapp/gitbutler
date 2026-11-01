//! The right side: whatever is selected in the navigator, with its files and diff.
//! Uncommitted changes are for reviewing what an agent did, and committing it;
//! a commit shows its message.

mod blame;
mod branches;
mod composer;
mod conversation;
mod diff_view;
mod file_list;
mod file_tree;
mod find;
mod graph;
pub use branches::{ApplyBranch, EditDescription, NewWorktreeFromBranch, ShowOnlyLocalBranches};
pub use file_list::{DiscardFromCommit, MarkResolved, SplitIntoNewCommit, UncommitChanges};
pub use find::FindInDiff;
pub use history::RestoreOperation;
mod history;
mod lines;
mod machine;
mod panes;
mod pull_request;
mod rename;
mod resolve;
mod review_edit;
mod slots;
mod upstream;
mod worktree;

pub use diff_view::DiffPane;
pub use resolve::{ResolveConflicts, oldest_conflict};

use std::cell::Cell;
use std::cmp::Ordering;
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

use crate::app::{AddProject, AppHandle, CopyAbsolutePath, CopyRelativePath};
use crate::backend::edit::Editing;
use crate::backend::{
    self, Checkout, CommitId, DiffLine, DiffSource, FileChange, FileDiff, FullName, HunkKey,
    Selected,
};
use crate::clock;
use crate::overlays::MenuItem;
use crate::palette::score;
use crate::state::{
    Cut, FileSort, Pane, Picks, Prefs, ProjectId, Section, Selection, Store, StoreEvent, snack,
};
use crate::theme::Theme;
use crate::ui::diff::{self, DiffRow};
use crate::ui::fold::{FOLD, Folds, Slide, edge, inert};
use crate::ui::illustration::{Illustration, illustration};
use crate::ui::message_editor::{COMMIT_PLACEHOLDERS, MessageEditor, MessageSize};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, TextPoint, button, commit_title, dp, empty_state, fixed, h_flex,
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
        DiscardHunk,
        DiscardFile,
        FoldFile,
        UnfoldFile,
        ToggleFileFold,
        PageDown,
        PageUp,
        HalfPageDown,
        HalfPageUp,
        SwitchTab,
        AmendTop,
        PickCommitTarget,
        ToggleWrap,
        TogglePlainLines,
        ToggleSplit,
        ToggleFiles,
        FilterFiles,
        ToggleCheck,
        WriteMessage,
    ]
);

/// How far Space and Page Down scroll the diff, in its heights, keeping a few lines in view.
pub const PAGE: f32 = 0.85;

/// Where the keyboard is in the diff.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    file: usize,
    at: Stop,
}

/// What of its file the cursor is on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The file's header.
    #[default]
    File,
    /// The box above hunk `.0`, saying what's hidden before it: the file's, as the unchanged
    /// lines it stands for are, not the hunk's.
    Gap(usize),
    /// Hunk `.0`'s lines, from its first change to its last: what the hunk keys act on.
    Hunk(usize),
}

impl Cursor {
    /// On file `file`'s header.
    fn on_file(file: usize) -> Self {
        Cursor {
            file,
            at: Stop::File,
        }
    }

    /// On hunk `hunk` of file `file`.
    fn on_hunk(file: usize, hunk: usize) -> Self {
        Cursor {
            file,
            at: Stop::Hunk(hunk),
        }
    }
}

/// What of the detail y copied, which blinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Copied {
    /// The text selected in the diff.
    Text,
    /// The path of the file under the cursor: its row in the file list, or in the diff what
    /// the cursor's on, whichever has the keys.
    Cursor,
}

impl Stop {
    /// The hunk the hunk keys act on, not one with the cursor on the box above it.
    fn hunk(self) -> Option<usize> {
        match self {
            Stop::Hunk(hunk) => Some(hunk),
            Stop::File | Stop::Gap(_) => None,
        }
    }

    /// The hunk the cursor is on or just above.
    fn near(self) -> Option<usize> {
        match self {
            Stop::Gap(hunk) | Stop::Hunk(hunk) => Some(hunk),
            Stop::File => None,
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
    /// The one file the rows show, with Settings' all-in-one diff off.
    alone: Option<usize>,
    /// What was selected when it loaded.
    selection: Option<Selection>,
}

/// A file's new side as diff lines, read for the patch with this fingerprint, and the
/// widest of them in columns.
type FileLines = (u64, Arc<Vec<DiffLine>>, u32);

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
    /// The keys went left out of the tabs, so coming back from the left lands on them again
    /// rather than on the files under them. Another page forgets it.
    tabs_kept: bool,
    /// A People card's, which a click on it takes and a click anywhere else gives back to the
    /// list it's under: its person picked shows as a focused selection only then.
    people_focus: FocusHandle,
    /// The People card lists everyone, scrolling within it, rather than the busiest few.
    people_open: bool,
    people_scroll: ScrollHandle,
    /// Where the diff is gliding to, a step each frame, after the keys moved its cursor.
    glide: Option<Glide>,
    /// The file the diff was scrolled to past the cursor, which the file list marks as the
    /// current one: see [`Detail::current_file`].
    reading: Option<usize>,
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
    /// Where the uncommitted file list and a commit's or branch's file list are scrolled.
    uncommitted_files: ListState,
    diff_files: ListState,
    /// The cursor's file the file list last scrolled to show, so it follows the cursor's
    /// moves but not its own scrolling.
    listed_cursor: std::cell::RefCell<Option<String>>,
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
    /// Files shown whole, by path, with their lines once read.
    whole: HashMap<String, Option<FileLines>>,
    /// SVG files shown as text rather than drawn, by path.
    svg_text: HashSet<String>,
    /// Files picked in the file list, by path, to act on together.
    file_picks: Picks<String>,
    /// Lines picked by their numbers, in one file, to act on together.
    line_picks: Option<lines::LinePicks>,
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
    /// The pull request whose title and description are being edited on its tab. Looking at
    /// another keeps it, as `rewording` is kept.
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
    /// Whether the selected commit's message shows its body, not just its title.
    message_open: bool,
    /// Whether a branch with a pull request shows it in place of its changes.
    review_tab: bool,
    /// The selected checkout's page shows its overview, Now, rather than its changes.
    now_open: bool,
    /// What that overview reads beyond the workspace.
    now: worktree::Now,
    /// The file to put the cursor on once the diff loading now lands, as when following the
    /// file being read into the commit an agent made of it.
    pending_file: Option<String>,
    /// The page a click on a commit of a branch that left a checkout picked, and that commit,
    /// whose merge or branch it opens once it has read its list.
    opening: Option<(Selection, crate::state::SeenCommit)>,
    /// Whether each pull request's checks are listed, by project and number, as the reader left
    /// them or as they opened by themselves on a failure. Others are folded. Rendering records
    /// the opening, so it's a cell.
    checks_open: std::cell::RefCell<HashMap<(ProjectId, u64), bool>>,
    /// The pull request threads shown open while resolved, or folded while not, by id.
    threads_flipped: HashSet<String>,
    /// The pull request tab's column: its title, description and conversation.
    review_list: ListState,
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
    /// Directories folded shut in tree-shaped file lists, by path.
    collapsed_dirs: HashSet<String>,
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
    /// Files folded by hand.
    folded: HashSet<String>,
    /// Files folding or unfolding now, and the rows each moves, by path.
    file_folds: Folds<String>,
    fold_rows: HashMap<String, FoldRows>,
    /// The commit message being written with AI, streaming into the box, and what the box
    /// held before, which a write cut short puts back.
    writing: Option<(Task<()>, String)>,
    /// The find bar over the diff, while it's open.
    find: Option<find::Find>,
    /// What's picked for the shown commit's conflicts, until it's applied.
    resolving: resolve::Resolving,
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
    /// Each History page's operations, the same way, its filter, and the kinds of operation
    /// it leaves out.
    history: HashMap<ProjectId, history::History>,
    history_filter: Entity<TextInput>,
    history_hidden: HashSet<history::Group>,
    history_scroll: ListState,
    _history_load: Task<()>,
    /// The file list beside the diff is hidden, for more room, and its slide there.
    files_hidden: bool,
    files_slide: Slide,
    /// The files between a list page's list and the diff are hidden, as they are at first: the
    /// page's own list already takes the room the file list has elsewhere. And their slide.
    beside_files_hidden: bool,
    beside_slide: Slide,
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
    /// The window's width to lay out for, in unscaled pixels, while the window narrows to the
    /// navigator or widens back as the detail slides, so it isn't laid out again at every
    /// width; otherwise the window's own.
    pub window_width: Cell<Option<f32>>,
    /// The Graph page, which draws itself as it pans and zooms.
    graph: Entity<graph::GraphView>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ChecksChanged> for Detail {}

impl Detail {
    /// No diff loading or queued, for the dev script's timings.
    #[cfg(feature = "dev-script")]
    pub fn is_settled(&self) -> bool {
        self.loading.is_none() && !self.reload_queued
    }

    pub fn new(
        store: Entity<Store>,
        app: AppHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let message =
            cx.new(|cx| MessageEditor::new(MessageSize::Compact, COMMIT_PLACEHOLDERS, cx));
        let branch_filter = cx.new(|cx| TextInput::new("Filter branches", false, cx));
        let target_filter = cx.new(|cx| TextInput::new("Filter commits", false, cx));
        let history_filter = cx.new(|cx| TextInput::new("Filter operations", false, cx));
        let file_filter = cx.new(|cx| TextInput::new("Filter files", false, cx));
        let graph = cx.new(|cx| graph::GraphView::new(store.clone(), cx));
        let subscriptions = vec![
            cx.subscribe(&store, |this, _, event, cx| match event {
                StoreEvent::SelectionChanged => {
                    // A commit an operation rewrote is still the one being looked at.
                    let followed =
                        (this.key_for_selection(cx)).is_some_and(|key| this.follows(&key, cx));
                    if !followed {
                        this.message_open = false;
                        this.reset_view();
                    }
                    let project = this.selected_project(cx);
                    let checkout = this.selected_checkout(cx).map(|(_, worktree)| worktree);
                    if this.checks_project != project {
                        // The branch picked to commit to waits for coming back, as checks do.
                        let left = this.checks_project.clone();
                        if let Some((left, target)) = left.zip(this.commit_target.take()) {
                            this.other_targets.insert(left, target);
                        }
                        this.commit_target =
                            (project.as_ref()).and_then(|next| this.other_targets.remove(next));
                        this.switch_checkout(project, checkout.unwrap_or_default(), cx);
                    } else if let Some(worktree) = checkout
                        && worktree != this.checks_worktree
                    {
                        // Checks are of one checkout's files, which another's share paths with.
                        this.switch_checkout(project, worktree, cx);
                    }
                    // A checkout's page opens on its overview, as the project's does.
                    this.now_open = true;
                    // A page read before opens on its overview too.
                    match this.selection(cx) {
                        Some(Selection::Branches(_)) => this.select_other_branch(None, cx),
                        Some(Selection::Project(project)) => this.open_on_shipped(&project),
                        Some(Selection::History(project)) => {
                            this.open_on_newest_operation(&project)
                        }
                        _ => {}
                    }
                    this.reload_page(true, cx);
                    this.refresh_diff(true, cx);
                    this.open_waiting(false, cx);
                }
                // Picked again, as by the crumb naming it, a page goes back to how it opens.
                StoreEvent::Reopened => match this.selection(cx) {
                    Some(Selection::Project(project)) => this.pick_opening(&project, cx),
                    Some(Selection::History(_)) => this.pick_newest_operation(cx),
                    Some(Selection::Uncommitted(_) | Selection::Worktree(..)) => {
                        this.close_changes(cx);
                    }
                    _ => {
                        this.close_opened_branch(cx);
                    }
                },
                StoreEvent::Loaded(project) => {
                    if this.selected_project(cx).as_ref() == Some(project) {
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
                    this.branch_renamed(project, from.as_ref(), to, cx)
                }
                StoreEvent::MeshRead(project) => {
                    if let Some(Selection::Mesh(p, ..)) = this.selection(cx)
                        && p == *project
                    {
                        this.load_machines(&p, false, cx);
                        this.refresh_diff(false, cx);
                        cx.notify();
                    }
                }
                StoreEvent::Pending(project) => {
                    if this.selected_project(cx).as_ref() == Some(project) {
                        this.rebuild_rows(cx);
                        this.skip_taken(cx);
                    }
                }
                // The app view asks for it, and gives a terminal opened the keys.
                StoreEvent::AuthorMissing(_) | StoreEvent::TerminalOpened => {}
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
                        if let Some(file) = first
                            && !shown.contains(&this.cursor.file)
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
            cx.subscribe_in(&message, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.commit(window, cx),
                // While the navigator places the commit, back to that.
                InputEvent::Cancel if this.committing(cx).is_some() => {
                    this.back_to_placing(window, cx)
                }
                InputEvent::Cancel => this.leave_composer(window, cx),
                // The input draws itself; nothing here shows the message, and notifying
                // would redraw the diff on every keystroke.
                InputEvent::Changed => {
                    let message = this.message.read(cx).text(cx);
                    if let Some(project) = &this.checks_project {
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
                let order = (prefs.file_tree, prefs.file_sort);
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
                if matches!(store.read(cx).selection, Some(Selection::Commit(..)))
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
            tabs_kept: false,
            people_focus: cx.focus_handle(),
            people_open: false,
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
            uncommitted_files: panes::side_list_state(),
            diff_files: panes::side_list_state(),
            listed_cursor: Default::default(),
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
            file_picks: Picks::default(),
            line_picks: None,
            grip_over: None,
            grip_held: false,
            picking_lines: false,
            svg_text: HashSet::new(),
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
            message_open: false,
            review_tab: false,
            now_open: false,
            now: worktree::Now::default(),
            pending_file: None,
            opening: None,
            checks_open: Default::default(),
            threads_flipped: HashSet::new(),
            review_list: ListState::new(0, ListAlignment::Top, fixed(200.)),
            review_side_scroll: ScrollHandle::new(),
            review_width: [(); 2].map(|_| Rc::new(Cell::new(pull_request::TWO_COLUMNS))),
            target_message_height: Rc::new(Cell::new(0.)),
            beside_width: Rc::new(Cell::new(0.)),
            shipped_focus: cx.focus_handle(),
            header_height: Rc::new(Cell::new(panes::COMMIT_HEADER_HEIGHT)),
            collapsed_dirs: HashSet::new(),
            page_folds: Folds::default(),
            text_selection: None,
            selecting: false,
            follow: None,
            note_selection: None,
            folded: HashSet::new(),
            file_folds: Folds::default(),
            fold_rows: HashMap::new(),
            writing: None,
            find: None,
            resolving: Default::default(),
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
            history_hidden: HashSet::new(),
            history_scroll: panes::side_list_state(),
            _history_load: Task::ready(()),
            files_hidden: false,
            files_slide: Slide::default(),
            beside_files_hidden: true,
            beside_slide: Slide::default(),
            _slid: Task::ready(()),
            files_hover: (None, false),
            files_popover_shown: Cell::new(false),
            _files_popover_delay: Task::ready(()),
            navigator_shown: Cell::new(1.),
            window_width: Cell::new(None),
            graph,
            _subscriptions: subscriptions,
        };
        // A wheel scroll is the user's, so the diff stops gliding where the keys sent it.
        let this = cx.weak_entity();
        detail.list.set_scroll_handler(move |_, _, cx| {
            (this.update(cx, |detail, _| detail.glide = None)).ok();
        });
        detail.refresh_diff(true, cx);
        detail
    }

    /// The page shown: what's selected.
    fn selection(&self, cx: &App) -> Option<Selection> {
        self.store.read(cx).selection.clone()
    }

    fn selected_project(&self, cx: &App) -> Option<ProjectId> {
        self.selection(cx).map(|s| s.project().clone())
    }

    /// Register `A` on `el` as copying `text`, as the app copies it.
    fn copy_on<A: gpui::Action>(
        el: gpui::Stateful<gpui::Div>,
        text: String,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        el.on_action(cx.listener(move |this, _: &A, window, cx| {
            let text = text.clone();
            this.app
                .update_later(window, cx, move |app, _, cx| app.copy_text(text, cx))
        }))
    }

    /// The pull request of what a page has picked: the branch opened on the Branches page, or
    /// the target's merge.
    pub(crate) fn page_pull_request(&self, cx: &App) -> Option<String> {
        match self.selection(cx)? {
            Selection::Branches(project) => self
                .selected_other_branch(&project)?
                .review
                .as_ref()?
                .url
                .clone(),
            Selection::Section(project, Section::Upstream(_)) => self.target_pull_request(&project),
            _ => None,
        }
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
    pub fn selected_checkout(&self, cx: &App) -> Option<(ProjectId, Checkout)> {
        match self.selection(cx)? {
            Selection::Uncommitted(project) => Some((project, Checkout::Main)),
            Selection::Worktree(project, name) => Some((project, Checkout::Linked(name))),
            _ => None,
        }
    }

    /// The commit `project` is editing, if `but resolve` has one checked out.
    fn editing<'a>(&self, project: &ProjectId, cx: &'a App) -> Option<&'a Editing> {
        self.store.read(cx).workspace(project)?.editing.as_ref()
    }

    fn key_for_selection(&self, cx: &App) -> Option<DiffKey> {
        let selection = self.selection(cx)?;
        let store = self.store.read(cx);
        let source = match &selection {
            Selection::Project(project) if self.editing(project, cx).is_some() => return None,
            Selection::Project(project) => self.upstream_source(project, cx)?,
            Selection::History(project) => self.history_source(project)?,
            Selection::Branches(project) => self.branches_source(project)?,
            Selection::Graph(_) => return None,
            Selection::Uncommitted(_) => DiffSource::Worktree,
            Selection::Worktree(_, name) => DiffSource::Linked(name.clone()),
            Selection::Mesh(project, id, pick) => self.machine_source(project, id, pick)?,
            // Commits picked with it in one run show their changes together, and with others
            // between them, each one's in turn.
            Selection::Commit(project, id) if store.among_picks(project, *id) => {
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
            Selection::Section(project, Section::Upstream(Some(ref_name))) => {
                let branch = store.workspace(project)?.find_branch(Some(ref_name))?;
                DiffSource::Commits(branch.incoming.iter().rev().map(|c| c.id).collect())
            }
            // What a pull would bring in from the target, at its newest commit.
            Selection::Section(project, Section::Upstream(None)) => {
                let target = store.workspace(project)?.target.as_ref()?;
                DiffSource::Incoming {
                    tip: target.incoming.first()?.id,
                    behind: target.commits_behind,
                }
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
        match self.selection(cx) {
            Some(Selection::History(project)) => self.load_history(&project, false, cx),
            Some(Selection::Branches(project)) => self.load_branches(&project, force, cx),
            Some(Selection::Project(project)) => self.load_upstream(&project, false, force, cx),
            Some(Selection::Mesh(project, ..)) => self.load_machines(&project, force, cx),
            Some(Selection::Uncommitted(_) | Selection::Worktree(..)) => self.load_now(force, cx),
            _ => {}
        }
    }

    /// Forget how the last diff was looked at: its text selection, cursor and folds, and how
    /// far the pull request was scrolled. For a new selection, or another
    /// item picked on the same page.
    fn reset_view(&mut self) {
        self.tabs_kept = false;
        self.text_selection = None;
        self.note_selection = None;
        self.file_picks.clear();
        self.line_picks = None;
        self.cursor = Cursor::default();
        self.reading = None;
        self.folded.clear();
        self.file_folds.stop_all();
        self.fold_rows.clear();
        self.review_list.reset(0);
        self.review_side_scroll.set_offset(Default::default());
    }

    /// Show the diff of what was just picked on the selected page, from its top.
    fn show_picked(&mut self, cx: &mut Context<Self>) {
        self.reset_view();
        self.refresh_diff(true, cx);
        // A pick on the page changes no selection, which the store's observer would ask on.
        self.ask_forge(cx);
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
            return;
        }
        let Some(key) = key else {
            self.diff = None;
            self.loading = None;
            cx.notify();
            return;
        };
        if !selection_changed && self.loading.as_ref() == Some(&key) {
            self.reload_queued = true;
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
        // A load still running is of what was selected before, which mustn't land under this.
        let Some(path) = self.store.read(cx).path(&key.project) else {
            self.loading = None;
            return;
        };
        let worktree = key.uncommitted(self.store.read(cx)).map(<[_]>::to_vec);
        if key.source.live() && worktree.is_none() {
            // The workspace is still loading and lists the files itself; diffing now would run
            // a second status on the same repository, slowing both. Its `Loaded` brings us back.
            self.loading = None;
            return;
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
        let selection = self.selection(cx);
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
        let (tree, sort) = (prefs.file_tree, prefs.file_sort);
        let order = |a: &FileDiff, b: &FileDiff| {
            let folders = match tree {
                true => file_tree::folder_order(&a.file.path, &b.file.path),
                false => Ordering::Equal,
            };
            folders.then_with(|| file_tree::file_order(sort, &a.file, &b.file))
        };
        let files = match files.is_sorted_by(|a, b| order(a, b).is_le()) {
            true => files,
            false => {
                let mut sorted = files.to_vec();
                sorted.sort_by(order);
                Arc::new(sorted)
            }
        };
        // Before the rows, which can show the cursor's file alone. The file scrolled to is
        // found again in the new files once they're drawn.
        if same {
            self.cursor = self.cursor_in(&files);
        }
        self.reading = None;
        let (rows, shown, alone) = self.build_rows(&key, &files, false, cx);
        let top = same.then(|| self.keep_place(&rows, &files, alone.is_some()));
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
        if !self.file_picks.is_empty() {
            let paths: HashSet<&str> = files.iter().map(|f| f.file.path.as_str()).collect();
            (self.file_picks).follow(|path| paths.contains(path.as_str()).then(|| path.clone()));
        }
        if !same {
            self.line_picks = None;
        }
        self.diff = Some(LoadedDiff {
            key,
            files,
            tree,
            sort,
            rows,
            shown,
            alone,
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
        self.run_find(!same, cx);
        self.diff_error = None;
        if let Some(path) = self.pending_file.take()
            && let Some(at) = (self.diff.iter())
                .flat_map(|diff| diff.files.iter())
                .position(|file| file.file.path == path)
        {
            self.cursor = Cursor::on_file(at);
            self.scroll_to_file(at, false, cx);
        }
    }

    /// What dragging or cutting the uncommitted changes of `project`'s checkout `worktree`
    /// carries: the checked files and hunks when they're shown here, otherwise all of them, but
    /// only what's [`Selected::movable`].
    pub fn files_to_move(
        &self,
        project: &ProjectId,
        worktree: Checkout,
        cx: &App,
    ) -> Vec<Selected> {
        let files = match self.selected_checkout(cx) == Some((project.clone(), worktree.clone())) {
            true => self.selected_changes(cx),
            false => (self.store.read(cx).workspace(project))
                .and_then(|ws| ws.uncommitted_in(&worktree))
                .map_or_else(Vec::new, |files| {
                    files.iter().map(Selected::whole).collect()
                }),
        };
        files.into_iter().filter(Selected::movable).collect()
    }

    /// Focus the diff.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Focus the file list, or the diff while the list is hidden. On the Branches, Operations
    /// history and project pages, focus their list instead, as it takes the arrow keys itself.
    pub fn focus_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let side_list = self.list_page(cx);
        // Not the file list under a pull request's tab, which draws none.
        let files = !side_list && self.files_shown(cx) && !self.showing_review(cx);
        // A checkout's overview has nothing for the keys, so the list takes them on its first
        // file, which opens in the overview's place, as the first step down would.
        if files
            && self.showing_now(cx)
            && let Some(&first) = self.listed_files(cx).first()
        {
            self.scroll_to_file(first, true, cx);
        }
        // So has what the project's page opens on, what merged lately, and the Branches page's
        // overview: their lists take them on their first row, as the first step down would.
        if self.showing_shipped(cx) {
            self.step_upstream(1, cx);
        } else if self.showing_branches_overview(cx) {
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

    /// Focus the files between a list page's list and the diff.
    pub fn focus_beside_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.files_focus, cx);
        cx.notify();
    }

    /// Whether `handle` is one of the detail's own, the diff's or a list's, rather than a
    /// text field's.
    pub fn owns_focus(&self, handle: &FocusHandle) -> bool {
        [
            &self.focus,
            &self.files_focus,
            &self.side_focus,
            &self.people_focus,
            &self.shipped_focus,
        ]
        .contains(&handle)
    }

    /// Whether the file list holds the focus.
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
        let changes = at
            .hunk()
            .and_then(|hunk| self.row_of(|row| self.changes_at(*row) == Some((file, hunk))));
        let gap = || {
            let hunk = at.near()?;
            self.row_of(|row| *row == DiffRow::Hunk { file, hunk })
        };
        (changes.or_else(gap)).or_else(|| self.row_of(|row| *row == DiffRow::File(file)))
    }

    /// The file the file list marks, and what acts on a whole file acts on: the cursor's, or
    /// once the diff is scrolled past it, the file at the diff's top, the one its sticky header
    /// names, as in but.dev. What acts on a hunk still acts on the cursor's.
    fn current_file(&self) -> usize {
        self.reading.unwrap_or(self.cursor.file)
    }

    /// Mark the file the diff is scrolled to as the current one while the cursor is out of
    /// sight. Scrolling leaves the cursor where it is, so its hunk is still selected when the
    /// diff comes back to it. Not while the diff glides to the cursor, past other files.
    pub(super) fn follow_scroll(&mut self, cx: &mut Context<Self>) {
        let reading = match self.glide.is_none() && !self.cursor_in_sight(cx) {
            true => self.top_file().filter(|file| *file != self.cursor.file),
            false => None,
        };
        if self.reading != reading {
            self.reading = reading;
            cx.notify();
        }
    }

    /// The file of the row at the diff's top.
    fn top_file(&self) -> Option<usize> {
        let top = self.list.logical_scroll_top().item_ix;
        let rows = self.diff.as_ref()?.rows.get(top..)?;
        rows.iter().find_map(|row| row.file())
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
    /// the file under the cursor in the file list or the diff, else what the Branches,
    /// Operations history or target's page has selected in its list.
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
        match self.store.read(cx).selection.clone()? {
            Selection::Branches(project) => self.branches_page_menu(&project, cx),
            Selection::History(project) => self.operation_page_menu(&project, cx),
            Selection::Section(project, Section::Upstream(_)) => {
                self.target_page_menu(&project, cx)
            }
            _ => None,
        }
    }

    /// The rows `files` show: files folded by hand show only their header, and uncommitted
    /// files and hunks, or files of the commit shown, that a pending operation takes away are
    /// left out until it's done. `unfolded`, as they would with every file open and one line a
    /// row, as Find searches them. With them, the files shown and, with Settings' all-in-one
    /// diff off, the one file the rows are of.
    fn build_rows(
        &self,
        key: &DiffKey,
        files: &[FileDiff],
        unfolded: bool,
        cx: &App,
    ) -> (Vec<DiffRow>, Vec<usize>, Option<usize>) {
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
            !unfolded && self.folded.contains(path) && !self.file_folds.closing(path)
        };
        let hidden =
            |file: &FileDiff| leaving(&file.file.path) || !keeps_path(&terms, &file.file.path);
        let shown: Vec<usize> = (files.iter().enumerate())
            .filter(|(_, file)| !hidden(file))
            .map(|(ix, _)| ix)
            .collect();
        let alone = self.alone(&shown, cx);
        let alone_path = alone.and_then(|ix| Some(&files.get(ix)?.file.path));
        let rows = diff::rows(
            files,
            folded,
            |file| self.drawing_of(file).is_some(),
            |file| hidden(file) || alone_path.is_some_and(|path| *path != file.file.path),
            |file, hunk| parts_of(file).is_some_and(|keys| keys.contains(&hunk.key)),
            |file, hunk, line| {
                parts_of(file).is_some_and(|keys| keys.iter().any(|key| key.takes(hunk, line)))
            },
            |file| (self.whole_lines(file)).map(|(_, lines, _)| lines.len() as u32),
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
        (rows, shown, alone)
    }

    /// The one file of `shown` the diff shows with Settings' all-in-one diff off: the
    /// cursor's, or when that's hidden, the next shown after it, else the last, as
    /// `skip_taken` moves the cursor.
    fn alone(&self, shown: &[usize], cx: &App) -> Option<usize> {
        if !self.store.read(cx).prefs.one_file_diff {
            return None;
        }
        (shown.get(shown.partition_point(|&file| file < self.cursor.file)))
            .or(shown.last())
            .copied()
    }

    /// Show the cursor's file alone once the cursor goes to another, or every file once
    /// Settings turns the all-in-one diff back on.
    pub(super) fn follow_alone(&mut self, cx: &mut Context<Self>) {
        if let Some(diff) = &self.diff
            && diff.alone != self.alone(&diff.shown, cx)
        {
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
        let page = match self.selection(cx) {
            _ if self.files_focused(window) => None,
            Some(Selection::Branches(_)) => Some(self.branch_filter.clone()),
            Some(Selection::History(_)) => Some(self.history_filter.clone()),
            Some(Selection::Project(_)) => Some(self.target_filter.clone()),
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
        self.hide_files(self.list_page(cx), false, cx);
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

    /// Scroll the diff by most of a screen, keeping a few lines in view for context.
    /// Scroll the diff `by` its height, down or, negative, up. Down from its end, the app goes
    /// on to the next unread commit, so Space reads a run of commits from the first to the last.
    pub fn page(&mut self, by: f32, window: &mut Window, cx: &mut Context<Self>) {
        let at_end = -self.list.scroll_px_offset_for_scrollbar().y
            >= self.list.max_offset_for_scrollbar().y - crate::ui::fixed(1.);
        if by > 0. && at_end {
            // A file at a time, it goes on to the next file first: in folders folded shut too,
            // as the whole diff runs through them.
            if self.diff.as_ref().is_some_and(|diff| diff.alone.is_some()) {
                let order = self.files_in_list_order(&HashSet::new(), cx);
                let at = order.iter().position(|&file| file == self.cursor.file);
                if let Some(&file) = at.and_then(|at| order.get(at + 1)) {
                    return self.scroll_to_file(file, false, cx);
                }
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

    /// Where the view goes in `rows` of `files` to keep showing what's at its top now: the
    /// same row, or its file's header when that row went away. Rows of one file `alone`
    /// that's not the one at the top start at their own top.
    fn keep_place(&self, rows: &[DiffRow], files: &[FileDiff], alone: bool) -> ListOffset {
        let top = self.list.logical_scroll_top();
        // At the very top, what's added above shows: a file a merge left conflicted lists
        // first.
        let at_top = top.item_ix == 0 && top.offset_in_item == Pixels::ZERO;
        if at_top || self.diff.is_none() {
            return top;
        }
        match self.row_in(top.item_ix, rows, files) {
            Some((item_ix, true)) => ListOffset { item_ix, ..top },
            Some((item_ix, false)) => ListOffset {
                item_ix,
                offset_in_item: Pixels::ZERO,
            },
            None if alone => ListOffset::default(),
            None => ListOffset {
                item_ix: top.item_ix,
                offset_in_item: Pixels::ZERO,
            },
        }
    }

    /// Where row `at` of the rows shown is in `rows` of `files`: the same row, and true, or its
    /// file's header when that row went away, and false.
    fn row_in(&self, at: usize, rows: &[DiffRow], files: &[FileDiff]) -> Option<(usize, bool)> {
        let diff = self.diff.as_ref()?;
        // Gaps all look alike, so a gap is found by the row after it.
        let gaps = (diff.rows.iter().skip(at))
            .take_while(|row| **row == DiffRow::Gap)
            .count();
        // Its file found by path, as a reload can add or drop files before it.
        let anchor = diff.rows.get(at + gaps).and_then(|row| match row.file() {
            Some(file) => {
                let old = diff.files.get(file)?;
                let (now, new) =
                    (files.iter().enumerate()).find(|(_, f)| f.file.path == old.file.path)?;
                let mut row = row.in_file(now);
                // Its hunk as the cursor follows one, which hunks an agent adds or drops
                // elsewhere in the file don't move.
                if let Some(hunk) = row.hunk_mut()
                    && let Some(ix) = follow_hunk(&hunk_keys(old), *hunk, &hunk_keys(new))
                {
                    *hunk = ix;
                }
                Some(row)
            }
            None => Some(*row),
        });
        let found = anchor
            .and_then(|row| rows.iter().position(|r| r.same_place(row)))
            .and_then(|ix| ix.checked_sub(gaps));
        match found {
            Some(ix) => Some((ix, true)),
            None => anchor
                .and_then(DiffRow::file)
                .and_then(|file| rows.iter().position(|r| *r == DiffRow::File(file)))
                .map(|ix| (ix, false)),
        }
    }

    /// Rebuild the rows when folding changed, keeping the scroll position.
    fn rebuild_rows(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            cx.notify();
            return;
        };
        let (rows, shown, alone) = self.build_rows(&diff.key, &diff.files, false, cx);
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
            (diff.shown, diff.alone) = (shown, alone);
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

    /// Show code in diffs at `size`.
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

    /// Step `delta` files through the ones shown, letting picks go.
    fn move_file(&mut self, delta: isize, cx: &mut Context<Self>) {
        // From a checkout's overview, the first step opens the file the list starts with, and
        // G the one it ends with.
        if self.showing_now(cx)
            && let Some(&file) = panes::step_from(&self.listed_files(cx), None, delta)
        {
            return self.scroll_to_file(file, true, cx);
        }
        if let Some(file) = self.file_step(delta, cx) {
            self.file_picks.clear();
            self.scroll_to_file(file, true, cx);
        }
    }

    /// The file `delta` files from the current one, through the ones shown.
    fn file_step(&self, delta: isize, cx: &App) -> Option<usize> {
        let shown = self.listed_files(cx);
        if shown.is_empty() {
            return None;
        }
        // Found by place, as the filter's ranking can list the files out of the diff's order.
        // From a file that isn't shown, the next shown one counts as a step.
        let current = self.current_file();
        let at = match shown.iter().position(|file| *file == current) {
            Some(at) => at as isize,
            None => {
                let at = shown.partition_point(|file| *file < current) as isize;
                at - isize::from(delta >= 0)
            }
        };
        (shown.get((at + delta).clamp(0, shown.len() as isize - 1) as usize)).copied()
    }

    /// The files the file list shows, in its order: not those a filter or a pending
    /// operation hides, nor, in the tree, those of a directory folded shut. While the filter
    /// is open, the best matches first, as it ranks them.
    fn listed_files(&self, cx: &App) -> Vec<usize> {
        self.files_in_list_order(&self.collapsed_dirs, cx)
    }

    /// The files the diff shows, in the file list's order, with the directories in
    /// `collapsed` folded shut in a tree.
    fn files_in_list_order(&self, collapsed: &HashSet<String>, cx: &App) -> Vec<usize> {
        let shown = self.shown_files();
        let Some(diff) = &self.diff else {
            return shown;
        };
        let shown: Vec<(usize, &str)> = (shown.into_iter())
            .filter_map(|ix| Some((ix, diff.files.get(ix)?.file.path.as_str())))
            .collect();
        let (shown, paths): (Vec<usize>, Vec<&str>) =
            (ranked(&self.filter_terms(cx), shown, |(_, path)| *path).into_iter())
                .map(|(file, _)| file)
                .unzip();
        if !self.store.read(cx).prefs.file_tree {
            return shown;
        }
        (unfolded(&paths, collapsed).into_iter())
            .filter_map(|pos| shown.get(pos).copied())
            .collect()
    }

    pub(crate) fn move_hunk(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        // The keys go on to hunks that show, not a cursor out of sight. From a checkout's
        // overview, the first step opens the file the list starts with, as `move_file`'s does,
        // and from a branch's pull request, its changes. Nothing else that hides the diff shows
        // its hunks.
        if self.showing_now(cx)
            && let Some(&first) = self.listed_files(cx).first()
        {
            self.scroll_to_file(first, true, cx);
        }
        if self.showing_review(cx) {
            self.review_tab = false;
            cx.notify();
        }
        if self.diff_hidden(cx) {
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
        // Up from its top, the diff's keys go to the tabs over it, as the file list's do.
        if stop.is_none()
            && !forward
            && self.focus.is_focused(window)
            && self.selected_review(cx).is_some()
        {
            return self.focus_tabs(window, cx);
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
            Stop::File => cursor.at == Stop::File && cursor.file != from.file,
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
        let Some(old) = self
            .diff
            .as_ref()
            .and_then(|diff| diff.files.get(self.cursor.file))
        else {
            return Cursor::default();
        };
        // The file went, say discarded: stay where it was, on what moved up into its
        // place, rather than jumping to the first file the next key would act on.
        let Some((file, new)) =
            (files.iter().enumerate()).find(|(_, f)| f.file.path == old.file.path)
        else {
            return Cursor::on_file(self.cursor.file.min(files.len().saturating_sub(1)));
        };
        let near = (self.cursor.at.near())
            .and_then(|ix| follow_hunk(&hunk_keys(old), ix, &hunk_keys(new)));
        let at = match (self.cursor.at, near) {
            (Stop::Gap(_), Some(hunk)) => Stop::Gap(hunk),
            (_, Some(hunk)) => Stop::Hunk(hunk),
            (_, None) => Stop::File,
        };
        Cursor { file, at }
    }

    /// Move the cursor off a file or hunk a pending operation just took away, onto the next
    /// one shown, so the next key acts on what's on screen.
    fn skip_taken(&mut self, cx: &App) {
        let Some(diff) = &self.diff else {
            return;
        };
        let Some(path) = diff.files.get(self.cursor.file).map(|f| &f.file.path) else {
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

    /// The loaded diff when it's of a checkout's uncommitted changes, the ones the composer
    /// commits: the project's own or a linked worktree's.
    fn worktree_diff(&self) -> Option<&LoadedDiff> {
        (self.diff.as_ref()).filter(|diff| diff.key.checkout().is_some())
    }

    /// Whether the loaded diff is the worktree's and shows exactly `files` as they are now.
    fn shows_worktree(&self, files: &[FileChange]) -> bool {
        (self.worktree_diff())
            .is_some_and(|diff| same_files(diff.files.iter().map(|shown| &shown.file), files))
    }

    /// Open the file under the cursor in the editor, at its hunk.
    fn open_in_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let Some(file) = diff.files.get(self.cursor.file) else {
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
        !self.files_hidden
            && self
                .selected_checkout(cx)
                .is_some_and(|(project, worktree)| {
                    (store.workspace(&project))
                        .and_then(|ws| ws.uncommitted_in(&worktree))
                        .is_some()
                })
    }

    /// The commit composer is open.
    pub fn composing(&self) -> bool {
        self.composing
    }

    /// Whether the composer is open on `project`'s `checkout`, whose changes then go by
    /// what's checked.
    pub fn composing_in(&self, project: &ProjectId, checkout: &Checkout) -> bool {
        self.composing
            && self.checks_project.as_ref() == Some(project)
            && self.checks_worktree == *checkout
    }

    /// Whether the diff's cursor is on a hunk, rather than a file's header or the box above a
    /// hunk, for ⌥⌘Z to discard.
    pub fn on_hunk(&self) -> bool {
        self.cursor.at.hunk().is_some()
    }

    /// Whether there's a file shown for the diff's keys to act on, not one the filter hides.
    pub fn has_files(&self) -> bool {
        (self.shown_diff())
            .is_some_and(|diff| diff.rows.iter().any(|r| matches!(r, DiffRow::File(_))))
    }

    /// Commit what the open composer has once its message is written; until then, go to
    /// the message box.
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
        let window = self.laid_out_width(window, cx);
        let navigator = self.navigator_shown.get();
        (self.store.read(cx)).fitted_width(Pane::Files, window, navigator, 1.)
    }

    /// How wide the navigator is drawn while it's open, beside the file list as much as that
    /// shows at `now`. To the point, as AppKit sizes the window, so the window narrowed to it
    /// fits it.
    pub(crate) fn navigator_width(&self, now: Instant, window: &Window, cx: &App) -> f32 {
        let window = self.laid_out_width(window, cx);
        let files = self.files_share(now, cx);
        let width = (self.store.read(cx)).fitted_width(Pane::Navigator, window, 1., files);
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
        let diff = self.shown_diff()?;
        let file = diff.files.get(self.current_file())?;
        // The list's order is worked out only while the filter ranks the files, as this is drawn
        // with every frame: otherwise the list has them as the diff does, which loads them in
        // its order. Folders folded shut count too, as the diff shows their files.
        let shown = match self.filter_terms(cx).is_empty() {
            true => self.shown_files(),
            false => self.files_in_list_order(&HashSet::new(), cx),
        };
        let at = shown.iter().position(|file| *file == self.current_file())? + 1;
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
        self.diff_path(self.current_file())
    }

    /// The path of the current file, unless the diff it's in is hidden: under a
    /// pull request's tab, which draws no files, or a checkout's overview, where the list
    /// marks only picked ones.
    pub fn shown_cursor_path(&self, cx: &App) -> Option<String> {
        if self.showing_review(cx) {
            return None;
        }
        let path = self.cursor_path()?;
        (!self.showing_now(cx) || self.file_picks.contains(&path)).then_some(path)
    }

    /// What copying takes from here: the selected text, or the path of the file under the
    /// cursor, which blinks, as the TUI highlights what it copied. Under a pull request's
    /// tab, the text selected in its description or comments.
    pub fn copy(&mut self, cx: &mut Context<Self>) -> Option<String> {
        if self.showing_review(cx) {
            return self.selected_note_text(cx);
        }
        let (text, copied) = match self.selected_text() {
            Some(text) => (text, Copied::Text),
            None => (self.shown_cursor_path(cx)?, Copied::Cursor),
        };
        self.blinked = (self.blinked.0 + 1, Some((copied, Instant::now())));
        cx.notify();
        Some(text)
    }

    /// Which blink `copied` is in while it blinks: see [`crate::ui::blink_once`].
    fn blink(&self, copied: Copied) -> Option<usize> {
        let (n, last) = self.blinked;
        last.filter(|&(what, at)| what == copied && at.elapsed() < crate::ui::BLINK)
            .map(|_| n)
    }

    /// The indexes of the files the diff shows, in ascending order for the binary searches
    /// over them (Find's among them): not those a filter or a pending operation hides. Every
    /// one, though the all-in-one diff off shows them one at a time.
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
        let open = self.folded.remove(&path);
        if !open {
            self.folded.insert(path.clone());
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
    fn fold_cursor_file(&mut self, fold: bool, cx: &mut Context<Self>) {
        let file = self.current_file();
        let folded = self.diff_path(file).map(|path| self.folded.contains(&path));
        if folded == Some(!fold) && !self.diff_hidden(cx) {
            self.toggle_fold(file, cx);
        }
    }

    /// Throw away one hunk of the uncommitted file at `file_ix`, or the whole file when `hunk`
    /// is `None`. Never a file the diff doesn't show, as one the filter hides.
    pub fn discard(
        &mut self,
        file_ix: usize,
        hunk: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cursor = Cursor {
            file: file_ix,
            at: hunk.map_or(Stop::File, Stop::Hunk),
        };
        let Some(diff) = (self.diff.as_ref()).filter(|diff| diff.key.source.live()) else {
            return self.not_live(window, cx);
        };
        let Some(file) = diff.files.get(file_ix) else {
            return;
        };
        // A conflicted file's hunk can't be discarded alone: it goes whole or not at all.
        if !self.shown_files().contains(&file_ix) || (hunk.is_some() && file.file.conflicted) {
            return;
        }
        let hunks = match hunk.map(|ix| file.hunks().get(ix)) {
            None => None,
            Some(Some(hunk)) => Some(vec![hunk.key]),
            // Never widen a hunk discard to the whole file.
            Some(None) => return,
        };
        let (project, worktree) = (diff.key.project.clone(), diff.key.files_checkout());
        let change = file.part(hunks);
        self.app.update_later(window, cx, move |app, window, cx| {
            app.discard(project, worktree, vec![change], window, cx)
        });
    }

    /// Discard the picked lines of a diff of uncommitted changes.
    fn discard_picked_lines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Cut::Files {
            project,
            from: None,
            worktree,
            files,
        }) = self.line_picks_cut(cx)
        else {
            return self.not_live(window, cx);
        };
        self.app.update_later(window, cx, move |app, window, cx| {
            app.discard(project, worktree, files.to_vec(), window, cx)
        });
    }

    /// Say why the discard keys do nothing on a workspace commit's diff, whose file menu
    /// discards from it. Not with a page's list in front, as its rows aren't the diff's.
    fn not_live(&self, window: &Window, cx: &mut Context<Self>) {
        if !self.side_list_focused(window) && self.owned_cut(Vec::new(), cx).is_some() {
            let why = "Only uncommitted changes can be discarded with these keys. A commit's file menu has \
                 Discard from commit";
            snack(&self.store, why, cx);
        }
    }

    /// Discard the picked files of a diff of uncommitted changes.
    fn discard_picked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff) = (self.diff.as_ref()).filter(|diff| diff.key.source.live()) else {
            return self.not_live(window, cx);
        };
        let (project, worktree) = (diff.key.project.clone(), diff.key.files_checkout());
        let changes = self.picked_changes();
        self.app.update_later(window, cx, move |app, window, cx| {
            app.discard(project, worktree, changes, window, cx)
        });
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
            Stop::File => Pixels::ZERO,
            Stop::Gap(_) | Stop::Hunk(_) => Theme::get(cx).scaled(diff::FILE_HEADER_HEIGHT),
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
            Some(DiffRow::File(_)) => theme.scaled(diff::FILE_HEADER_HEIGHT),
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

    /// Put the cursor on file `file_ix`'s header and scroll it to the top, gliding there with
    /// `glide`, as a key or a click goes from file to file. A diff that's just loaded, or a
    /// filter being typed, goes there at once.
    fn scroll_to_file(&mut self, file_ix: usize, glide: bool, cx: &mut Context<Self>) {
        // A file picked on a checkout's page shows its diff in place of the overview.
        self.now_open = false;
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
        match self.selection(cx) {
            Some(
                Selection::Commit(..)
                | Selection::Upstream(..)
                | Selection::Branch(..)
                | Selection::Stack(..),
            ) => self.header_height.get(),
            Some(Selection::Project(project)) => {
                let message = match self.showing_target_commit(&project, cx) {
                    true => self.target_message_height.get(),
                    false => 0.,
                };
                self.header_height.get() + message + self.list_tabs_height(cx)
            }
            Some(Selection::Branches(project)) => {
                // The opened branch's header, or its picked commit's message.
                let message = match self.selected_other_branch(&project) {
                    Some(_) => self.target_message_height.get(),
                    None => 0.,
                };
                message + self.list_tabs_height(cx)
            }
            Some(Selection::History(project)) if self.selected_operation(&project).is_some() => {
                self.header_height.get()
            }
            Some(Selection::Uncommitted(project)) if self.editing(&project, cx).is_none() => {
                self.header_height.get()
            }
            Some(Selection::Worktree(..)) => self.header_height.get(),
            _ => 0.,
        }
    }

    /// Whether the column beside the diff shows. On a page whose column is its list
    /// (Branches, Operations history, the project's) it always does: without the list,
    /// nothing says what the diff is of. Elsewhere the files show unless `f` hid them, however
    /// narrow the window. A checkout's overview shows beside them, or alone with them hidden,
    /// as its diff does.
    pub fn files_shown(&self, cx: &App) -> bool {
        !self.overview_only(cx) && (self.list_page(cx) || !self.files_hidden)
    }

    /// How much of the column beside the diff shows at `now`, from 0 to 1, as `f` slides the
    /// file list. A list page's list always shows.
    pub fn files_share(&self, now: Instant, cx: &App) -> f32 {
        match (self.overview_only(cx), self.list_page(cx)) {
            (true, _) => 0.,
            (false, true) => 1.,
            (false, false) => self.files_slide.share(!self.files_hidden, now),
        }
    }

    /// Whether the column beside the diff is drawn at `now`: shown, or still sliding shut.
    fn files_drawn(&self, now: Instant, cx: &App) -> bool {
        self.files_shown(cx) || (!self.overview_only(cx) && self.files_slide.closing(now))
    }

    /// How far right of the detail's left edge the diff starts at `now`: past the column beside
    /// it and a list page's files beside its list, each as much as it shows as it slides.
    pub fn diff_left(&self, now: Instant, window: &Window, cx: &App) -> f32 {
        let files = self.files_width(window, cx) * self.files_share(now, cx);
        let beside = (self.has_files_beside(cx)).then(|| {
            let shown = self.beside_slide.share(!self.beside_files_hidden, now);
            self.beside_files_width(cx) * shown
        });
        files + beside.unwrap_or(0.)
    }

    /// Whether the file list, or the files beside a list page's list, slide at `now`.
    pub fn sliding(&self, now: Instant) -> bool {
        self.files_slide.openness(now).is_some() || self.beside_slide.openness(now).is_some()
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
        self.focus.is_focused(window) && self.without_changes(cx)
    }

    /// Whether the page is an overview with no column beside it: Inactive and Network
    /// worktrees', as the navigator lists what they sum up, and the Graph's.
    pub fn overview_only(&self, cx: &App) -> bool {
        matches!(
            self.selection(cx),
            Some(
                Selection::Graph(_)
                    | Selection::Section(_, Section::Inactive | Section::Mesh)
                    | Selection::Terminal(..)
            )
        )
    }

    /// Show or hide the file list beside the diff or a checkout's overview, or a list page's
    /// files beside its list, as `f` does.
    pub(crate) fn toggle_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let list_page = self.list_page(cx);
        if self.overview_only(cx) || (list_page && !self.has_files_beside(cx)) {
            return;
        }
        let hidden = match list_page {
            true => !self.beside_files_hidden,
            false => !self.files_hidden,
        };
        self.hide_files(list_page, hidden, cx);
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
        let (flag, slide) = match beside {
            true => (&mut self.beside_files_hidden, &mut self.beside_slide),
            false => (&mut self.files_hidden, &mut self.files_slide),
        };
        if *flag == hidden {
            return;
        }
        *flag = hidden;
        slide.start(!hidden, cx);
        self._slid = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FOLD).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        });
        cx.notify();
    }

    pub(crate) fn list_page(&self, cx: &App) -> bool {
        matches!(
            self.selection(cx),
            Some(Selection::Branches(_) | Selection::History(_) | Selection::Project(_))
        )
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
        let selection = self.selection(cx);
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
                    Selection::Uncommitted(project) => {
                        self.render_uncommitted_side(project, &Checkout::Main, &theme, window, cx)
                    }
                    Selection::Commit(project, id) => {
                        self.render_commit_side(project, *id, &theme, cx)
                    }
                    Selection::Branch(project, ref_name) => {
                        self.render_branch_side(project, ref_name.as_ref(), &theme, cx)
                    }
                    Selection::History(project) => {
                        self.render_history_side(project, &theme, window, cx)
                    }
                    Selection::Worktree(project, name) => {
                        let checkout = Checkout::Linked(name.clone());
                        self.render_uncommitted_side(project, &checkout, &theme, window, cx)
                    }
                    Selection::Stack(project, bottom) => {
                        self.render_stack_side(project, bottom.as_ref(), &theme, cx)
                    }
                    Selection::Upstream(..) | Selection::Section(_, Section::Upstream(_)) => {
                        self.render_diff_file_list(&theme, cx)
                    }
                    // No column: the overview takes the page.
                    Selection::Graph(_)
                    | Selection::Section(_, Section::Inactive | Section::Mesh)
                    | Selection::Terminal(..) => div().into_any_element(),
                    Selection::Mesh(project, id, pick) => {
                        self.render_mesh_side(project, id, pick, &theme, cx)
                    }
                    Selection::Branches(project) => {
                        self.render_branches_side(project, &theme, window, cx)
                    }
                    Selection::Project(project) => match self.editing(project, cx) {
                        Some(editing) => empty_state(
                            None,
                            commit_title(&editing.title),
                            format!(
                                "Commit {} is checked out for editing. Its files are under \
                                 Editing a commit, with Finish and Cancel.",
                                editing.commit.short()
                            ),
                            &theme,
                            cx,
                        ),
                        None => self.render_upstream_side(project, &theme, window, cx),
                    },
                };
                let width = self.files_width(window, cx);
                // A page's header runs across it, over its files and its diff, as but.dev's
                // does. `view_header_height` lists the same pages.
                let header = match selection {
                    Selection::Commit(project, id)
                        if self.store.read(cx).among_picks(project, *id) =>
                    {
                        Some(self.render_picks_header(project, *id, &theme, cx))
                    }
                    Selection::Commit(project, id) => {
                        Some(self.render_commit_header(project, *id, &theme, cx))
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
                    Selection::Upstream(project, ref_name, id) => {
                        Some(self.render_upstream_header(
                            project,
                            ref_name.as_ref(),
                            *id,
                            &theme,
                            window,
                            cx,
                        ))
                    }
                    Selection::Project(project) => {
                        Some(self.render_project_header(project, &theme, cx))
                    }
                    Selection::Uncommitted(project) if self.editing(project, cx).is_none() => {
                        Some(self.render_checkout_header(project, &Checkout::Main, &theme, cx))
                    }
                    Selection::Worktree(project, name) => {
                        let checkout = Checkout::Linked(name.clone());
                        Some(self.render_checkout_header(project, &checkout, &theme, cx))
                    }
                    _ => None,
                };
                // Beside a list, the selected branch or commit's tabs when it has a pull
                // request, over the pull request or the diff.
                let beside = match selection {
                    Selection::Project(project) if self.showing_target_commit(project, cx) => {
                        let message = (self.shown_target_commit(project, cx))
                            .map(|commit| self.render_target_message(commit, &theme, cx));
                        Some(self.list_beside(project, message, &theme, window, cx))
                    }
                    // An opened branch: its header, its files and its pull request, as a merge
                    // on the project's page has them, and a commit of it picked shows its
                    // message, as a merge's does.
                    Selection::Branches(project) if self.showing_branches_overview(cx) => {
                        Some(self.render_branches_overview(project, &theme, cx))
                    }
                    Selection::Branches(project) => {
                        let header = match self.picked_branch_commit(project) {
                            Some(commit) => Some(self.render_target_message(commit, &theme, cx)),
                            None => self.render_other_branch_header(project, &theme, cx),
                        };
                        Some(self.list_beside(project, header, &theme, window, cx))
                    }
                    // A target several local branches track is picked where what shipped shows.
                    Selection::Project(project)
                        if let Some(choice) = (self.store.read(cx).project(project))
                            .and_then(|entry| entry.target_choice.clone()) =>
                    {
                        Some(self.target_chooser(project, &choice, &theme, cx))
                    }
                    Selection::Project(project) if self.showing_shipped(cx) => {
                        Some(self.render_shipped(project, window, cx))
                    }
                    Selection::Section(project, Section::Inactive) => {
                        Some(self.render_inactive_overview(project, &theme, cx))
                    }
                    Selection::Section(project, Section::Mesh) => {
                        Some(self.render_machines_overview(project, &theme, cx))
                    }
                    Selection::Graph(_) => Some(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(
                                (self.graph.clone())
                                    .cached(gpui::StyleRefinement::default().size_full()),
                            )
                            .into_any_element(),
                    ),
                    // What the selected operation did, over the files it changed.
                    Selection::History(project) => {
                        self.render_operation_header(project, &theme, cx)
                    }
                    Selection::Uncommitted(project) if self.showing_now(cx) => {
                        Some(self.render_now(project, &Checkout::Main, cx))
                    }
                    Selection::Worktree(project, name) if self.showing_now(cx) => {
                        Some(self.render_now(project, &Checkout::Linked(name.clone()), cx))
                    }
                    _ => None,
                };
                let (files_shown, list_page) = (self.files_shown(cx), self.list_page(cx));
                // A list that went while it had the keys hands them to the diff, as `f` does.
                if !files_shown && self.files_focus.is_focused(window) {
                    let focus = self.focus.clone();
                    window.defer(cx, move |window, cx| window.focus(&focus, cx));
                }
                // The file list sliding beside an overview is cut to as much as shows, drawn again
                // as it goes. Under the diff it stays as it is, and the diff slides over it.
                let now = clock::now();
                let cut =
                    (self.files_slide.openness(now)).filter(|_| !list_page && self.diff_hidden(cx));
                if cut.is_some() {
                    window.request_animation_frame();
                }
                let panes = match selection {
                    Selection::Branch(project, _) | Selection::Upstream(project, ..)
                        if self.showing_review(cx) =>
                    {
                        self.render_review_tab(project, &theme, cx)
                    }
                    _ => h_flex()
                        .w_full()
                        .flex_1()
                        .min_h_0()
                        .items_start()
                        .on_drag_move(resize_pane(Pane::Files, self.store.clone()))
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
                                .child(resize_handle(Pane::Files, self.store.clone(), &theme));
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
            .map(|el| self.branches_page_actions(el, cx))
            .map(|el| self.history_actions(el, cx))
            .map(|el| self.upstream_page_actions(el, cx))
            .map(|el| self.change_actions(el, cx))
            // The file under the cursor, while the file list or the diff has the keys.
            .when_some(
                self.focused_file_paths(window, cx),
                |el, (relative, absolute)| {
                    let revealed = std::path::PathBuf::from(&absolute);
                    let el = Self::copy_on::<CopyRelativePath>(el, relative, cx);
                    Self::copy_on::<CopyAbsolutePath>(el, absolute, cx).on_action(cx.listener(
                        move |_, _: &crate::navigator::Reveal, _, cx| cx.reveal_path(&revealed),
                    ))
                },
            )
            .when(self.shows_conflicts(), |el| resolve::conflict_keys(el, cx))
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NextFile, window, cx| {
                if !this.down_from_tabs(window, cx) {
                    this.move_file(1, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &PreviousFile, window, cx| {
                if !this.up_to_tabs(window, cx) {
                    this.move_file(-1, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &FirstFile, _, cx| this.move_file(isize::MIN / 2, cx)))
            .on_action(cx.listener(|this, _: &LastFile, _, cx| this.move_file(isize::MAX / 2, cx)))
            .on_action(cx.listener(|this, _: &PickNextFile, _, cx| this.pick_step(1, cx)))
            .on_action(cx.listener(|this, _: &PickPreviousFile, _, cx| this.pick_step(-1, cx)))
            .on_action(cx.listener(|this, _: &PickAllFiles, _, cx| this.pick_all_files(cx)))
            .on_action(cx.listener(|this, _: &NextHunk, window, cx| {
                if !this.down_from_tabs(window, cx) {
                    this.move_hunk(true, window, cx)
                }
            }))
            // Nothing is over the tabs.
            .on_action(cx.listener(|this, _: &PreviousHunk, window, cx| {
                if !this.tabs_focus.is_focused(window) {
                    this.move_hunk(false, window, cx)
                }
            }))
            // Picked lines go first, as ⌘X takes them first. The cursor's file only while it's
            // shown, not under a checkout's overview, where no row says which it is. Only with
            // the diff's keys: elsewhere its hunk shows faint, and a discard mustn't land on it.
            .on_action(cx.listener(|this, _: &DiscardHunk, window, cx| {
                if !this.focus.is_focused(window) {
                    return;
                }
                if this.line_picks.is_some() {
                    this.discard_picked_lines(window, cx)
                } else if let Some(hunk) = this.cursor.at.hunk().filter(|_| !this.showing_now(cx)) {
                    this.discard(this.cursor.file, Some(hunk), window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &DiscardFile, window, cx| {
                if this.line_picks.is_some() {
                    this.discard_picked_lines(window, cx)
                } else if !this.file_picks.is_empty() {
                    this.discard_picked(window, cx)
                } else if !this.showing_now(cx) {
                    this.discard(this.current_file(), None, window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &FoldFile, _, cx| this.fold_cursor_file(true, cx)))
            .on_action(cx.listener(|this, _: &UnfoldFile, _, cx| this.fold_cursor_file(false, cx)))
            // On its header, or its row in the file list, or scrolled to past the cursor, not
            // from inside one of its hunks.
            .on_action(cx.listener(|this, _: &ToggleFileFold, window, cx| {
                let in_hunk = this.cursor.at != Stop::File && this.reading.is_none();
                if in_hunk && !this.files_focused(window) {
                    return;
                }
                let file = this.diff_path(this.current_file());
                let folded = file.is_some_and(|path| this.folded.contains(&path));
                this.fold_cursor_file(!folded, cx)
            }))
            .on_action(cx.listener(|this, _: &PageDown, window, cx| this.page(PAGE, window, cx)))
            .on_action(cx.listener(|this, _: &PageUp, window, cx| this.page(-PAGE, window, cx)))
            .on_action(cx.listener(|this, _: &HalfPageDown, window, cx| this.page(0.5, window, cx)))
            .on_action(cx.listener(|this, _: &HalfPageUp, window, cx| this.page(-0.5, window, cx)))
            .on_action(cx.listener(|this, _: &SwitchTab, window, cx| this.switch_tab(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleWrap, _, cx| this.toggle_wrap(cx)))
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
                    this.open_in_editor(window, cx)
                }),
            )
            .when(self.composing, |el| {
                el.on_action(cx.listener(|this, _: &ToggleCheck, window, cx| {
                    this.toggle_cursor_check(window, cx)
                }))
                .on_action(
                    cx.listener(|this, _: &WriteMessage, window, cx| {
                        this.focus_message(window, cx)
                    }),
                )
            })
            .size_full()
            .bg(theme.bg_1)
            .child(self.render_header(selection.as_ref(), &theme, window, cx))
            .child(div().flex_1().min_h_0().child(body))
    }
}

/// Which of `paths`, by position, a tree of them lists with the directories in `collapsed`
/// folded shut, in its order. A row the tree merges, as `src/backend`, folds by its own path.
fn unfolded(paths: &[&str], collapsed: &HashSet<String>) -> Vec<usize> {
    (file_tree::tree_rows(paths, collapsed).into_iter())
        .filter_map(|row| match row {
            file_tree::TreeRow::File { pos, .. } => Some(pos),
            file_tree::TreeRow::Dir { .. } => None,
        })
        .collect()
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
        let cursor = Cursor {
            file: entry.file()?,
            at: stop(row, entry)?,
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
        Cursor, Stop, follow_hunk, keeps_path, next_stop, path_match, ranked, reveal_by,
        same_files, unfolded,
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
        assert_eq!(
            unfolded(&["src/backend/x.rs"], &collapsed(&["src"])),
            [0],
            "`src` holds only `backend`, so the tree has one `src/backend` row, not folded"
        );
        assert_eq!(
            unfolded(&["src/backend/x.rs"], &collapsed(&["src/backend"])),
            [] as [usize; 0],
            "the merged row folds by its own path"
        );
        assert_eq!(
            unfolded(
                &["src/backend/x.rs", "src/y.rs", "z.rs"],
                &collapsed(&["src"])
            ),
            [2],
            "`src` has a file of its own here, so it's a row of its own, folding all under it"
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
