//! The right side: whatever is selected in the navigator, with its files and diff.
//! Uncommitted changes are for reviewing what an agent did, and committing it;
//! a commit shows its message.

mod branches;
mod composer;
mod diff_view;
mod file_list;
mod file_tree;
mod find;
pub use find::FindInDiff;
mod history;
mod lines;
mod machine;
mod panes;
mod pull_request;
mod resolve;
mod upstream;

pub use diff_view::DiffPane;
pub use resolve::ResolveConflicts;

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    ListAlignment, ListOffset, ListState, RenderImage, ScrollHandle, SharedString, Subscription,
    Task, UniformListScrollHandle, WeakEntity, Window, actions, div, prelude::*, px,
};

use crate::app::{AddProject, AppView, Collapse, Expand, SelectNext, SelectPrevious};
use crate::backend::edit::Editing;
use crate::backend::{self, DiffLine, DiffSource, FileChange, FileDiff, HunkKey, Selected};
use crate::state::{FileSort, Pane, Picks, Prefs, ProjectId, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::ui::diff::{self, DiffRow};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, button, commit_title, empty_state, h_flex, resize_handle, resize_pane, shortcut,
    v_flex,
};

actions!(
    detail,
    [
        NextFile,
        PreviousFile,
        PickNextFile,
        PickPreviousFile,
        PickAllFiles,
        NextHunk,
        PreviousHunk,
        DiscardHunk,
        DiscardFile,
        ToggleFold,
        PageDown,
        PageUp,
        AmendTop,
        ToggleWrap,
        TogglePlainLines,
        ToggleSplit,
        ToggleFiles,
        FilterFiles,
    ]
);

/// Discarding a hunk follows Zed's agent review, whose cmd-alt-z rejects one. Moving between
/// files and hunks follows Lite; in the file list, as in but.dev's, ↑ and ↓ step through files,
/// and with ⇧ pick them, as in Finder.
pub fn bind_keys(cx: &mut App) {
    let c = Some("Detail && !TextInput");
    let files = Some("Files && !TextInput");
    let side = Some("SideList && !TextInput");
    cx.bind_keys([
        KeyBinding::new("down", NextFile, files),
        KeyBinding::new("up", PreviousFile, files),
        KeyBinding::new("shift-down", PickNextFile, files),
        KeyBinding::new("shift-up", PickPreviousFile, files),
        KeyBinding::new("secondary-a", PickAllFiles, files),
        KeyBinding::new("j", NextFile, c),
        KeyBinding::new("k", PreviousFile, c),
        KeyBinding::new("down", NextHunk, c),
        KeyBinding::new("up", PreviousHunk, c),
        KeyBinding::new("cmd-alt-z", DiscardHunk, c),
        KeyBinding::new("cmd-backspace", DiscardFile, c),
        KeyBinding::new("z", ToggleFold, c),
        KeyBinding::new("pagedown", PageDown, c),
        KeyBinding::new("space", PageDown, c),
        KeyBinding::new("pageup", PageUp, c),
        KeyBinding::new("shift-space", PageUp, c),
        KeyBinding::new("f", ToggleFiles, c),
        KeyBinding::new("/", FilterFiles, c),
        // From the navigator too: they change how every diff shows. The app passes them on.
        KeyBinding::new(
            "alt-z",
            ToggleWrap,
            Some("(Navigator || Detail) && !TextInput"),
        ),
        KeyBinding::new(
            "cmd-b",
            ToggleSplit,
            Some("(Navigator || Detail) && !TextInput"),
        ),
        // The History and Branches lists step and fold as the navigator does.
        KeyBinding::new("down", SelectNext, side),
        KeyBinding::new("j", SelectNext, side),
        KeyBinding::new("up", SelectPrevious, side),
        KeyBinding::new("k", SelectPrevious, side),
        KeyBinding::new("right", Expand, side),
        KeyBinding::new("l", Expand, side),
        KeyBinding::new("left", Collapse, side),
        KeyBinding::new("h", Collapse, side),
        // Works while typing the message too, as the composer's other key does.
        KeyBinding::new("alt-cmd-enter", AmendTop, Some("Detail")),
        KeyBinding::new("cmd-shift-b", composer::PickCommitTarget, Some("Detail")),
    ]);
    find::bind_keys(cx);
    resolve::bind_keys(cx);
}

/// Where the keyboard is in the diff.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    file: usize,
    hunk: Option<usize>,
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
            .uncommitted_in(self.checkout()?)
    }

    /// The checkout whose uncommitted files this diff is: the main one (`Some(None)`) or a
    /// linked worktree by name. `None` for a diff of commits.
    fn checkout(&self) -> Option<Option<&str>> {
        match &self.source {
            DiffSource::Worktree => Some(None),
            DiffSource::Linked(name) => Some(Some(name)),
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
            (self.checkout()).is_some_and(|checkout| store.hides_file(project, checkout, path))
                || (commit.as_ref())
                    .is_some_and(|commit| store.hides_commit_file(project, commit, path))
        }
    }

    /// The linked worktree this diff is of, whose files open from its own checkout.
    fn linked(&self) -> Option<Arc<str>> {
        match &self.source {
            DiffSource::Linked(name) => Some(name.clone()),
            _ => None,
        }
    }
}

/// How long the pointer rests on the files button before it lists the files.
const FILES_POPOVER_DELAY: Duration = Duration::from_millis(100);

/// How long a diff loads before the pane says it's loading.
const SHOW_LOADING: Duration = Duration::from_millis(150);

/// How tall the uncommitted changes' commit footer is, and the composer's row of buttons.
pub const FOOTER_HEIGHT: f32 = 44.;

struct LoadedDiff {
    key: DiffKey,
    /// In the file list's order: as picked, and in a tree, folder by folder.
    files: Arc<Vec<FileDiff>>,
    /// The files are in the tree's order, and the order they're sorted in.
    tree: bool,
    sort: FileSort,
    rows: Vec<DiffRow>,
}

/// A file's new side as diff lines, read for the patch with this fingerprint, and the
/// widest of them in columns.
type FileLines = (u64, Arc<Vec<DiffLine>>, u32);

/// A spot in the diff's text: a row of the diff and a byte offset in that row's line.
type TextPoint = (usize, usize);

/// The files checked for the next commit changed.
pub struct ChecksChanged;

pub struct Detail {
    store: Entity<Store>,
    /// For what the window owns: focus, menus, and prompts that ask first.
    app: WeakEntity<AppView>,
    focus: FocusHandle,
    /// The file list's own, where ↑ and ↓ step through the files.
    files_focus: FocusHandle,
    /// The History or Branches page's list, which a click on one of its rows focuses.
    side_focus: FocusHandle,
    /// The cursor was put where it is on purpose, by a key or a click: scrolling the diff
    /// moves it again only once the user scrolls.
    pinned: bool,
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
    uncommitted_files: UniformListScrollHandle,
    diff_files: UniformListScrollHandle,
    /// The cursor's file the file list last scrolled to show, so it follows the cursor's
    /// moves but not its own scrolling.
    listed_cursor: std::cell::RefCell<Option<String>>,
    message: Entity<TextInput>,
    /// The branch (full ref) new commits go to; `None` creates a branch.
    commit_target: Option<String>,
    checks: composer::Checks,
    /// The checks of the other checkouts looked at, by project and linked worktree, kept
    /// for coming back to them.
    other_checks: HashMap<(Option<ProjectId>, Option<Arc<str>>), composer::Checks>,
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
    /// What signed each commit previewed, like "an SSH key", or `None` when nothing did or
    /// it's still being read. A commit never changes, so neither does its signature.
    signers: HashMap<backend::CommitId, Option<&'static str>>,
    /// How far each file's lines are scrolled sideways, by path: the old side's and the new
    /// side's, which a unified diff scrolls.
    sideways: HashMap<String, [ScrollHandle; 2]>,
    /// The commit message being edited, by the commit's change ID, with the subscription that
    /// saves it. Looking at another commit keeps it, to pick up again on coming back.
    rewording: Option<(String, Entity<TextInput>, Subscription)>,
    /// The project the checks and commit target above belong to.
    checks_project: Option<ProjectId>,
    /// The linked worktree whose files the checks are of, or `None` for the project's own.
    checks_worktree: Option<Arc<str>>,
    /// The commit message box is open; until then there are no checkboxes.
    composing: bool,
    /// Where the focus was when writing a commit message started, to go back to after.
    composer_back: Option<FocusHandle>,
    /// What scrolls the commit message box, which keeps the caret in view.
    message_scroll: ScrollHandle,
    cursor: Cursor,
    /// Whether the selected commit's message shows its body, not just its title.
    message_open: bool,
    /// Whether a branch with a pull request shows it in place of its changes.
    review_tab: bool,
    /// A commit only the selected branch's remote has, picked on its page to see its diff in
    /// place of the branch's: the branch's project and ref, and the commit.
    remote_commit: Option<(ProjectId, String, backend::CommitId)>,
    /// Whether the pull request's checks that passed are listed, not just counted.
    passed_checks_open: bool,
    review_scroll: ScrollHandle,
    /// How tall a commit's header across the page was drawn, which the diff starts below.
    header_height: Rc<Cell<f32>>,
    /// Directories folded shut in tree-shaped file lists, by path.
    collapsed_dirs: HashSet<String>,
    /// Text selected in the diff, from where the drag started to where it is now.
    text_selection: Option<(TextPoint, TextPoint)>,
    /// The left button went down on a line and hasn't come up yet.
    selecting: bool,
    /// Files folded by hand.
    folded: HashSet<String>,
    /// The commit message being written with AI, streaming into the box.
    writing: Option<Task<()>>,
    /// The find bar over the diff, while it's open.
    find: Option<find::Find>,
    /// What's picked for the shown commit's conflicts, until it's applied.
    resolving: resolve::Resolving,
    /// The project page's target history, for the project it shows.
    upstream: Option<upstream::Upstream>,
    upstream_scroll: UniformListScrollHandle,
    _upstream_load: Task<()>,
    /// The Branches page's branches, for the project it shows, and its filter.
    branches: Option<branches::Branches>,
    branch_filter: Entity<TextInput>,
    branches_scroll: UniformListScrollHandle,
    _branches_load: Task<()>,
    /// What's fetched of other machines' branches; see `detail/machine.rs`.
    mesh: machine::MeshFetches,
    _branch_commits_load: Task<()>,
    /// The History page's operations, for the project it shows, its filter, and the kinds of
    /// operation it leaves out.
    history: Option<history::History>,
    history_filter: Entity<TextInput>,
    history_hidden: HashSet<history::Group>,
    history_scroll: UniformListScrollHandle,
    _history_load: Task<()>,
    /// The file list beside the diff is hidden, for more room.
    pub files_hidden: bool,
    /// Since when the pointer is on the files button over the diff, and whether it's on the
    /// popover of the files the button opens, [`FILES_POPOVER_DELAY`] on, while they aren't
    /// beside the diff.
    files_hover: (Option<Instant>, bool),
    /// Whether that popover was drawn last: back on the button from it, it stays open.
    files_popover_shown: Cell<bool>,
    _files_popover_delay: Task<()>,
    /// The header starts where the window's traffic lights are.
    pub beside_traffic_lights: bool,
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
        app: WeakEntity<AppView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let message_scroll = ScrollHandle::new();
        let message = cx.new(|cx| {
            TextInput::new("Commit message", true, cx).scrolled_by(message_scroll.clone())
        });
        let branch_filter = cx.new(|cx| TextInput::new("Filter branches", false, cx));
        let history_filter = cx.new(|cx| TextInput::new("Filter operations", false, cx));
        let file_filter = cx.new(|cx| TextInput::new("Filter files", false, cx));
        let subscriptions = vec![
            cx.subscribe(&store, |this, _, event, cx| match event {
                StoreEvent::SelectionChanged => {
                    this.remote_commit = None;
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
                        this.switch_checkout(project, checkout.flatten(), cx);
                        this.commit_target = None;
                    } else if let Some(worktree) = checkout
                        && worktree != this.checks_worktree
                    {
                        // Checks are of one checkout's files, which another's share paths with.
                        this.switch_checkout(project, worktree, cx);
                    }
                    this.reload_page(true, cx);
                    this.refresh_diff(true, cx);
                }
                StoreEvent::Loaded(project) => {
                    if this.selected_project(cx).as_ref() == Some(project) {
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
            }),
            cx.subscribe_in(
                &file_filter,
                window,
                |this, _, event, window, cx| match event {
                    InputEvent::Changed => {
                        this.rebuild_rows(cx);
                        // So the diff's keys act on a file in view.
                        let shown = this.shown_files();
                        if let Some(&file) = shown.first()
                            && !shown.contains(&this.cursor.file)
                        {
                            this.place_cursor(Cursor { file, hunk: None });
                        }
                    }
                    InputEvent::Submit => window.focus(&this.focus, cx),
                    InputEvent::Cancel => this.close_filter(window, cx),
                },
            ),
            cx.subscribe_in(&message, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.commit(window, cx),
                InputEvent::Cancel => this.leave_composer(window, cx),
                // The input draws itself; nothing here shows the message, and notifying
                // would redraw the diff on every keystroke.
                InputEvent::Changed => {}
            }),
            list_filter(&history_filter, Self::select_shown_operation, window, cx),
            list_filter(&branch_filter, Self::select_shown_branch, window, cx),
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
                let order = (store.read(cx).file_tree, store.read(cx).file_sort);
                if let Some(diff) =
                    (this.diff.as_ref()).filter(|diff| (diff.tree, diff.sort) != order)
                {
                    let key = diff.key.clone();
                    let files = this
                        .recent_files(&key)
                        .unwrap_or_else(|| diff.files.clone());
                    this.show_diff(key, files, cx);
                }
                cx.notify()
            }),
        ];
        let mut detail = Self {
            store,
            app,
            focus: cx.focus_handle(),
            files_focus: cx.focus_handle(),
            side_focus: cx.focus_handle(),
            pinned: false,
            diff: None,
            diff_error: None,
            loading: None,
            reload_queued: false,
            selected_at: None,
            recent: Vec::new(),
            _load: Task::ready(()),
            waiting_since: None,
            _waiting: Task::ready(()),
            list: ListState::new(0, ListAlignment::Top, px(40.)),
            uncommitted_files: UniformListScrollHandle::new(),
            diff_files: UniformListScrollHandle::new(),
            listed_cursor: Default::default(),
            message,
            commit_target: None,
            checks: Default::default(),
            other_checks: HashMap::new(),
            file_filter,
            filtering: false,
            whole: HashMap::new(),
            whole_reading: HashSet::new(),
            file_picks: Picks::default(),
            line_picks: None,
            grip_over: None,
            grip_held: false,
            picking_lines: false,
            svg_text: HashSet::new(),
            signers: HashMap::new(),
            sideways: HashMap::new(),
            rewording: None,
            checks_project: None,
            checks_worktree: None,
            composing: false,
            composer_back: None,
            message_scroll,
            cursor: Cursor::default(),
            message_open: false,
            review_tab: false,
            remote_commit: None,
            passed_checks_open: false,
            review_scroll: ScrollHandle::new(),
            header_height: Rc::new(Cell::new(panes::COMMIT_HEADER_HEIGHT)),
            collapsed_dirs: HashSet::new(),
            text_selection: None,
            selecting: false,
            folded: HashSet::new(),
            writing: None,
            find: None,
            resolving: Default::default(),
            upstream: None,
            upstream_scroll: UniformListScrollHandle::new(),
            _upstream_load: Task::ready(()),
            branches: None,
            branch_filter,
            branches_scroll: UniformListScrollHandle::new(),
            _branches_load: Task::ready(()),
            mesh: machine::MeshFetches::default(),
            _branch_commits_load: Task::ready(()),
            history: None,
            history_filter,
            history_hidden: HashSet::new(),
            history_scroll: UniformListScrollHandle::new(),
            _history_load: Task::ready(()),
            files_hidden: false,
            files_hover: (None, false),
            files_popover_shown: Cell::new(false),
            _files_popover_delay: Task::ready(()),
            beside_traffic_lights: false,
            _subscriptions: subscriptions,
        };
        // A wheel scroll is the user's, after which the cursor follows the diff again.
        let this = cx.weak_entity();
        detail.list.set_scroll_handler(move |_, _, cx| {
            (this.update(cx, |detail, _| detail.pinned = false)).ok();
        });
        detail.refresh_diff(true, cx);
        detail
    }

    fn selection(&self, cx: &App) -> Option<Selection> {
        self.store.read(cx).selection.clone()
    }

    fn selected_project(&self, cx: &App) -> Option<ProjectId> {
        self.selection(cx).map(|s| s.project().clone())
    }

    /// The checkout whose uncommitted changes are shown, which the composer commits: the
    /// project's own (`None`) or a linked worktree, by name.
    pub fn selected_checkout(&self, cx: &App) -> Option<(ProjectId, Option<Arc<str>>)> {
        match self.selection(cx)? {
            Selection::Uncommitted(project) => Some((project, None)),
            Selection::Worktree(project, name) => Some((project, Some(name))),
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
            Selection::Project(project) => self.upstream_source(project)?,
            Selection::History(project) => self.history_source(project)?,
            Selection::Branches(project) => self.branches_source(project)?,
            Selection::Uncommitted(_) => DiffSource::Worktree,
            Selection::Worktree(_, name) => DiffSource::Linked(name.clone()),
            Selection::Mesh(project, id, pick) => self.machine_source(project, id, pick)?,
            Selection::Commit(_, id) => DiffSource::Commit(*id),
            Selection::Branch(project, ref_name) => {
                if let Some(id) = self.remote_commit(project, ref_name, cx) {
                    return Some(DiffKey {
                        project: project.clone(),
                        source: DiffSource::Commit(id),
                    });
                }
                let branch = store.workspace(project)?.find_branch(ref_name)?;
                let newest = branch.commits.first()?.id;
                let oldest = branch.commits.last()?.id;
                DiffSource::Range { oldest, newest }
            }
            // Its top branch's newest commit back to its bottom branch's oldest, skipping
            // branches with none yet.
            Selection::Stack(project, bottom) => {
                let branches = store.workspace(project)?.lane(bottom)?;
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
            _ => {}
        }
    }

    /// Forget how the last diff was looked at: its text selection, cursor and folds. For a
    /// new selection, or another item picked on the same page.
    fn reset_view(&mut self) {
        self.text_selection = None;
        self.file_picks.clear();
        self.line_picks = None;
        self.cursor = Cursor::default();
        self.folded.clear();
    }

    /// Show the diff of what was just picked on the selected page, from its top.
    fn show_picked(&mut self, cx: &mut Context<Self>) {
        self.reset_view();
        self.refresh_diff(true, cx);
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
        let Some(path) = self.store.read(cx).path(&key.project) else {
            return;
        };
        let worktree = key.uncommitted(self.store.read(cx)).map(<[_]>::to_vec);
        if key.source.live() && worktree.is_none() {
            // The workspace is still loading and lists the files itself; diffing now would run
            // a second status on the same repository, slowing both. Its `Loaded` brings us back.
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
        let same = self.diff.as_ref().is_some_and(|diff| diff.key == key) || self.follows(&key, cx);
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
        let (tree, sort) = (self.store.read(cx).file_tree, self.store.read(cx).file_sort);
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
        let rows = self.build_rows(&key, &files, cx);
        let top = same.then(|| self.keep_place(&rows, &files));
        // Most rows are a line tall: rows not drawn yet count as one, so the scrollbar's thumb
        // is about the right size before they are.
        (self.list).reset_with_uniform_height(rows.len(), px(Theme::get(cx).code_line));
        if let Some(top) = top {
            self.list.scroll_to(top);
            self.cursor = self.cursor_in(&files);
        }
        if key.checkout() == Some(self.checks_worktree.as_deref()) {
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
        });
        self.settle_line_picks();
        self.prune_resolving();
        for file in stale {
            self.load_whole(file, cx);
        }
        self.run_find(false, cx);
        self.diff_error = None;
    }

    /// What dragging the uncommitted changes of `project`'s checkout `worktree` carries: the
    /// checked files and hunks when they're shown here, otherwise all of them.
    pub fn files_to_move(
        &self,
        project: &ProjectId,
        worktree: Option<Arc<str>>,
        cx: &App,
    ) -> Vec<Selected> {
        if self.selected_checkout(cx) == Some((project.clone(), worktree.clone())) {
            return self.selected_changes(cx);
        }
        let store = self.store.read(cx);
        let files = (store.workspace(project))
            .and_then(|ws| ws.uncommitted_in(worktree.as_deref()))
            .unwrap_or_default();
        files.iter().map(Selected::whole).collect()
    }

    /// Focus the diff.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Focus the file list, or the diff while the list is hidden. On the Branches, Operations
    /// history and project pages, focus their list instead, as it takes the arrow keys itself.
    pub fn focus_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = match &self.store.read(cx).selection {
            Some(Selection::Branches(_) | Selection::History(_) | Selection::Project(_)) => {
                &self.side_focus
            }
            _ if self.files_shown(cx) => &self.files_focus,
            _ => &self.focus,
        };
        window.focus(handle, cx);
        cx.notify();
    }

    /// Whether `handle` is one of the detail's own, the diff's or a list's, rather than a
    /// text field's.
    pub fn owns_focus(&self, handle: &FocusHandle) -> bool {
        *handle == self.focus || *handle == self.files_focus || *handle == self.side_focus
    }

    /// Whether the file list holds the focus.
    pub fn files_focused(&self, window: &Window) -> bool {
        self.files_focus.is_focused(window)
    }

    /// Put the cursor at `cursor` on purpose, as a key or a click does: scrolling the diff
    /// moves it again only once the user scrolls.
    fn place_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
        self.pinned = true;
    }

    /// While the user scrolls the diff, the file at its top, the one its sticky header names,
    /// is the current one, and the file list marks it, as in but.dev.
    pub(super) fn follow_scroll(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.as_ref().filter(|_| !self.pinned) else {
            return;
        };
        let top = self.list.logical_scroll_top().item_ix;
        let Some(file) = (diff.rows.get(top..)).and_then(|rows| rows.iter().find_map(|r| r.file()))
        else {
            return;
        };
        if file != self.cursor.file {
            self.cursor = Cursor { file, hunk: None };
            cx.notify();
        }
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }

    /// The diff's rows. Files folded by hand show only their header; uncommitted files and hunks, or files of the commit shown, that a pending
    /// operation takes away are left out until it's done.
    fn build_rows(&self, key: &DiffKey, files: &[FileDiff], cx: &App) -> Vec<DiffRow> {
        let store = self.store.read(cx);
        let terms = self.filter_terms(cx);
        let leaving = key.leaving(store);
        // Hunks, and lines of them, a pending operation takes out of their file.
        let commit = match key.source {
            DiffSource::Commit(id) => store.key_of(&key.project, id),
            _ => None,
        };
        let parts = match (key.checkout(), &commit) {
            (Some(checkout), _) => store.parts_leaving(&key.project, checkout, None),
            (None, Some(commit)) => store.parts_leaving(&key.project, None, Some(commit)),
            (None, None) => HashMap::new(),
        };
        let parts_of = |file: &FileDiff| parts.get(file.file.path.as_str());
        diff::rows(
            files,
            |file| self.folded.contains(&file.file.path),
            |file| self.drawing_of(file).is_some(),
            |file| leaving(&file.file.path) || !keeps_path(&terms, &file.file.path),
            |file, hunk| parts_of(file).is_some_and(|keys| keys.contains(&hunk.key)),
            |file, hunk, line| {
                parts_of(file).is_some_and(|keys| keys.iter().any(|key| key.takes(hunk, line)))
            },
            |file| match self.whole.get(&file.file.path) {
                Some(Some((fingerprint, lines, _))) if *fingerprint == file.fingerprint => {
                    Some(lines.len() as u32)
                }
                _ => None,
            },
            store.prefs.split,
            |conflict| self.conflict_view(conflict),
        )
    }

    /// What the open file filter keeps paths by: each term, lowercased, somewhere in them.
    fn filter_terms(&self, cx: &App) -> Vec<String> {
        if !self.filtering {
            return Vec::new();
        }
        (self.file_filter.read(cx).text().split_whitespace())
            .map(str::to_lowercase)
            .collect()
    }

    fn open_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filtering = true;
        // Its field heads the file list, so a hidden list comes back with it.
        self.files_hidden = false;
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
        self.file_filter
            .update(cx, |input, cx| input.set_text("", cx));
        window.focus(&self.focus, cx);
        self.rebuild_rows(cx);
    }

    /// The diff to draw: the loaded one, or while the next loads the previous one, until
    /// the wait is long enough to show it's loading instead.
    fn shown_diff(&self) -> Option<&LoadedDiff> {
        let diff = self.diff.as_ref()?;
        let waiting = self.loading.as_ref().is_some_and(|key| *key != diff.key);
        let slow = self
            .waiting_since
            .is_some_and(|since| since.elapsed() >= SHOW_LOADING);
        (!waiting || !slow).then_some(diff)
    }

    /// Scroll the diff by most of a screen, keeping a few lines in view for context.
    fn page(&mut self, direction: f32, cx: &mut Context<Self>) {
        self.pinned = false;
        let height = self.list.viewport_bounds().size.height;
        self.list.scroll_by(height * 0.85 * direction);
        cx.notify();
    }

    /// Where the view goes in `rows` of `files` to keep showing what's at its top now: the
    /// same row, or its file's header when that row went away.
    fn keep_place(&self, rows: &[DiffRow], files: &[FileDiff]) -> ListOffset {
        let top = self.list.logical_scroll_top();
        // At the very top, what's added above shows: a file a merge left conflicted lists
        // first.
        let at_top = top.item_ix == 0 && top.offset_in_item == px(0.);
        let Some(diff) = self.diff.as_ref().filter(|_| !at_top) else {
            return top;
        };
        // Gaps all look alike, so a gap at the top is found by the row after it.
        let gaps = (diff.rows.iter().skip(top.item_ix))
            .take_while(|row| **row == DiffRow::Gap)
            .count();
        // Its file found by path, as a reload can add or drop files before it.
        let anchor = diff
            .rows
            .get(top.item_ix + gaps)
            .and_then(|row| match row.file() {
                Some(file) => {
                    let path = &diff.files.get(file)?.file.path;
                    let now = files.iter().position(|f| &f.file.path == path)?;
                    let mut row = row.in_file(now);
                    // Its hunk as the cursor follows one, which hunks an agent adds or drops
                    // elsewhere in the file don't move.
                    if let Some(hunk) = row.hunk_mut()
                        && let Some(ix) = follow_hunk(
                            &hunk_keys(&diff.files[file]),
                            *hunk,
                            &hunk_keys(&files[now]),
                        )
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
            Some(item_ix) => ListOffset { item_ix, ..top },
            None => ListOffset {
                item_ix: anchor
                    .and_then(DiffRow::file)
                    .and_then(|file| rows.iter().position(|r| *r == DiffRow::File(file)))
                    .unwrap_or(top.item_ix),
                offset_in_item: px(0.),
            },
        }
    }

    /// Rebuild the rows when folding changed, keeping the scroll position.
    fn rebuild_rows(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            cx.notify();
            return;
        };
        let rows = self.build_rows(&diff.key, &diff.files, cx);
        if rows != diff.rows {
            self.text_selection = None;
            let scroll_to = self.keep_place(&rows, &diff.files);
            (self.list).reset_with_uniform_height(rows.len(), px(Theme::get(cx).code_line));
            self.list.scroll_to(scroll_to);
            if let Some(diff) = &mut self.diff {
                diff.rows = rows;
            }
            // What Find counts follows what's shown.
            self.run_find(false, cx);
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
    pub fn toggle_plain_lines(&mut self, cx: &mut Context<Self>) {
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
    fn remeasure(&mut self, cx: &mut Context<Self>) {
        let top = self.list.logical_scroll_top();
        let count = self.diff.as_ref().map_or(0, |diff| diff.rows.len());
        (self.list).reset_with_uniform_height(count, px(Theme::get(cx).code_line));
        self.list.scroll_to(top);
        cx.notify();
    }

    fn row_of(&self, wanted: impl Fn(&DiffRow) -> bool) -> Option<usize> {
        self.diff.as_ref()?.rows.iter().position(wanted)
    }

    /// Step `delta` files through the ones shown, letting picks go.
    fn move_file(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(file) = self.file_step(delta, cx) {
            self.file_picks.clear();
            self.scroll_to_file(file, cx);
        }
    }

    /// The file `delta` files from the cursor, through the ones shown.
    fn file_step(&self, delta: isize, cx: &App) -> Option<usize> {
        let shown = self.listed_files(cx);
        if shown.is_empty() {
            return None;
        }
        // From a file that isn't shown, the next shown one counts as a step.
        let at = shown.partition_point(|file| *file < self.cursor.file) as isize;
        let at = if shown.contains(&self.cursor.file) || delta < 0 {
            at
        } else {
            at - 1
        };
        Some(shown[(at + delta).clamp(0, shown.len() as isize - 1) as usize])
    }

    /// The files the file list shows, in its order: not those a filter or a pending
    /// operation hides, nor, in the tree, those of a directory folded shut.
    fn listed_files(&self, cx: &App) -> Vec<usize> {
        // Not into a directory folded shut in the tree, whose files the list doesn't show.
        let folded_away = |file: &usize| {
            let path = (self.diff.as_ref()).and_then(|diff| diff.files.get(*file));
            path.is_some_and(|f| {
                (self.collapsed_dirs.iter()).any(|dir| {
                    (f.file.path.strip_prefix(dir.as_str()))
                        .is_some_and(|rest| rest.starts_with('/'))
                })
            })
        };
        let tree = self.store.read(cx).file_tree;
        let mut shown = self.shown_files();
        if tree {
            shown.retain(|file| !folded_away(file));
        }
        shown
    }

    fn move_hunk(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let hunks: Vec<(usize, usize, usize)> = diff
            .rows
            .iter()
            .enumerate()
            .filter_map(|(row, entry)| match *entry {
                DiffRow::Hunk { file, hunk } => Some((file, hunk, row)),
                _ => None,
            })
            .collect();
        let at = (self.cursor.file, self.cursor.hunk);
        let next = if forward {
            hunks.iter().find(|(file, hunk, _)| match at {
                (f, Some(h)) => (*file, *hunk) > (f, h),
                (f, None) => *file >= f,
            })
        } else {
            hunks.iter().rev().find(|(file, hunk, _)| match at {
                (f, Some(h)) => (*file, *hunk) < (f, h),
                (f, None) => *file < f,
            })
        };
        if let Some(&(file, hunk, row)) = next {
            self.place_cursor(Cursor {
                file,
                hunk: Some(hunk),
            });
            self.reveal_row(row);
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
        let Some(file) = files.iter().position(|f| f.file.path == old.file.path) else {
            return Cursor {
                file: self.cursor.file.min(files.len().saturating_sub(1)),
                hunk: None,
            };
        };
        let hunk = (self.cursor.hunk)
            .and_then(|ix| follow_hunk(&hunk_keys(old), ix, &hunk_keys(&files[file])));
        Cursor { file, hunk }
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
        let at = (self.cursor.file, self.cursor.hunk);
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
            self.place_cursor(Cursor { file, hunk });
        }
    }

    /// The loaded diff when it's of a checkout's uncommitted changes, the ones the composer
    /// commits: the project's own or a linked worktree's.
    fn worktree_diff(&self) -> Option<&LoadedDiff> {
        (self.diff.as_ref()).filter(|diff| diff.key.checkout().is_some())
    }

    /// Whether the loaded diff is the worktree's and shows exactly `files` as they are now.
    fn shows_worktree(&self, files: &[FileChange]) -> bool {
        self.worktree_diff().is_some_and(|diff| {
            diff.files.len() == files.len()
                && diff.files.iter().zip(files).all(|(shown, now)| {
                    shown.file.path == now.path
                        && shown.file.modified == now.modified
                        && shown.file.base() == now.base()
                })
        })
    }

    /// Open the file under the cursor in the editor, at its hunk.
    fn open_in_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let Some(file) = diff.files.get(self.cursor.file) else {
            return;
        };
        let line = (self.cursor.hunk)
            .and_then(|ix| file.hunks().get(ix))
            .map(|h| h.key.0.new_start);
        let (project, path) = (diff.key.project.clone(), file.file.path.clone());
        let worktree = diff.key.linked();
        let position = window.mouse_position();
        self.app
            .update(cx, |app, cx| {
                app.open_in_editor(&project, worktree, Some(path), line, position, cx)
            })
            .ok();
    }

    /// Keep committing to branch `from` (a full ref) now that it's renamed `to`.
    pub fn branch_renamed(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        if self.commit_target.as_deref() == Some(from) {
            self.commit_target = Some(to.to_owned());
            cx.notify();
        }
    }

    /// Whether the uncommitted changes' commit footer, or the composer that takes its place,
    /// is at the bottom of the page, where a snackbar would cover its buttons.
    pub fn shows_footer(&self, cx: &App) -> bool {
        let store = self.store.read(cx);
        !self.files_hidden
            && self
                .selected_checkout(cx)
                .is_some_and(|(project, worktree)| {
                    (store.workspace(&project))
                        .and_then(|ws| ws.uncommitted_in(worktree.as_deref()))
                        .is_some_and(|files| !files.is_empty())
                })
    }

    /// The commit composer is open.
    pub fn composing(&self) -> bool {
        self.composing
    }

    /// Whether the diff's cursor is on a hunk, rather than a file's header, for ⌥⌘Z to discard.
    pub fn on_hunk(&self) -> bool {
        self.cursor.hunk.is_some()
    }

    /// Whether there's a file shown for the diff's keys to act on, not one the filter hides.
    pub fn has_files(&self) -> bool {
        (self.shown_diff())
            .is_some_and(|diff| diff.rows.iter().any(|r| matches!(r, DiffRow::File(_))))
    }

    /// Commit what the open composer has once its message is written; until then, go to
    /// the message box.
    pub fn commit_or_write(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing && !self.message.read(cx).text().trim().is_empty() {
            self.commit(window, cx)
        } else {
            self.focus_message(window, cx)
        }
    }

    /// Commit to `branch` of `project` next: a branch just made is where its commits go.
    pub fn branch_created(&mut self, project: &ProjectId, branch: String, cx: &mut Context<Self>) {
        if self.checks_project.as_ref() == Some(project) {
            self.commit_target = Some(branch);
            cx.notify();
        }
    }

    /// How wide the file list is drawn while it's shown.
    fn files_width(&self, window: &Window, cx: &App) -> f32 {
        let window = window.viewport_size().width.into();
        (self.store.read(cx)).fitted_width(Pane::Files, window, !self.beside_traffic_lights, true)
    }

    /// Where the diff's cursor is, as an editor's status bar says the line: the file under
    /// it, that file's place among the ones shown, and its hunk.
    pub fn cursor_place(&self) -> Option<(String, String)> {
        let diff = self.shown_diff()?;
        let file = diff.files.get(self.cursor.file)?;
        let shown = self.shown_files();
        let at = shown.binary_search(&self.cursor.file).ok()? + 1;
        let mut place = format!("file {at} of {}", shown.len());
        let hunks = file.hunks().len();
        if let Some(hunk) = self.cursor.hunk.filter(|hunk| *hunk < hunks) {
            place.push_str(&format!(" · hunk {} of {hunks}", hunk + 1));
        }
        Some((file.file.path.clone(), place))
    }

    /// The path of the file under the cursor, in any diff.
    pub fn cursor_path(&self) -> Option<String> {
        self.diff_path(self.cursor.file)
    }

    /// The indexes of the files the diff shows, in ascending order for the binary searches
    /// over them (Find's among them): not those a filter or a pending operation hides.
    fn shown_files(&self) -> Vec<usize> {
        (self.diff.iter())
            .flat_map(|diff| &diff.rows)
            .filter_map(|row| match row {
                DiffRow::File(file) => Some(*file),
                _ => None,
            })
            .collect()
    }

    pub fn toggle_fold(&mut self, file_ix: usize, cx: &mut Context<Self>) {
        let Some(path) = self.diff_path(file_ix) else {
            return;
        };
        if !self.folded.remove(&path) {
            self.folded.insert(path);
        }
        self.place_cursor(Cursor {
            file: file_ix,
            hunk: None,
        });
        self.rebuild_rows(cx);
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
        self.place_cursor(Cursor {
            file: file_ix,
            hunk,
        });
        let Some(diff) = (self.diff.as_ref()).filter(|diff| diff.key.source.live()) else {
            return;
        };
        let Some(file) = diff.files.get(file_ix) else {
            return;
        };
        // A conflicted file's hunk has no discard button either: it goes whole or not at all.
        if !self.shown_files().contains(&file_ix) || (hunk.is_some() && file.file.conflicted) {
            return;
        }
        let hunks = match hunk.map(|ix| file.hunks().get(ix)) {
            None => None,
            Some(Some(hunk)) => Some(vec![hunk.key]),
            // Never widen a hunk discard to the whole file.
            Some(None) => return,
        };
        let (project, worktree) = (diff.key.project.clone(), diff.key.linked());
        let change = file.part(hunks);
        self.app
            .update(cx, |app, cx| {
                app.discard(project, worktree, vec![change], window, cx)
            })
            .ok();
    }

    /// Discard the picked files of a diff of uncommitted changes.
    fn discard_picked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff) = (self.diff.as_ref()).filter(|diff| diff.key.source.live()) else {
            return;
        };
        let (project, worktree) = (diff.key.project.clone(), diff.key.linked());
        let changes = self.picked_changes();
        self.app
            .update(cx, |app, cx| {
                app.discard(project, worktree, changes, window, cx)
            })
            .ok();
    }

    /// Bring diff row `row` into view: left where it is when it's on screen, else scrolled to
    /// just below the top. gpui's own reveal goes by estimated heights, and past rows not
    /// drawn yet, like a file header or an image, it stops short.
    fn reveal_row(&mut self, row: usize) {
        let view = self.list.viewport_bounds();
        let shown = (self.list.bounds_for_item(row))
            .is_some_and(|item| item.top() >= view.top() && item.bottom() <= view.bottom());
        if !shown {
            self.list.scroll_to(ListOffset {
                item_ix: row.saturating_sub(2),
                offset_in_item: px(0.),
            });
        }
    }

    fn scroll_to_file(&mut self, file_ix: usize, cx: &mut Context<Self>) {
        self.place_cursor(Cursor {
            file: file_ix,
            hunk: None,
        });
        if let Some(row) = self.row_of(|row| matches!(row, DiffRow::File(ix) if *ix == file_ix)) {
            self.list.scroll_to(ListOffset {
                item_ix: row,
                offset_in_item: px(0.),
            });
            cx.notify();
        }
    }

    /// How far below the breadcrumbs the diff starts: under the page's header, for the pages
    /// the render's `header` gives one.
    pub fn view_header_height(&self, cx: &App) -> f32 {
        match self.selection(cx) {
            Some(
                Selection::Commit(..)
                | Selection::Branch(..)
                | Selection::Stack(..)
                | Selection::Project(..),
            ) => self.header_height.get() + self.list_tabs_height(cx),
            Some(Selection::Branches(_)) => self.list_tabs_height(cx),
            Some(Selection::History(project)) if self.selected_operation(&project).is_some() => {
                self.header_height.get()
            }
            _ => 0.,
        }
    }

    /// Whether the column beside the diff shows. On a page whose column is its list
    /// (Branches, Operations history, the project's) it always does: without the list,
    /// nothing says what the diff is of.
    pub fn files_shown(&self, cx: &App) -> bool {
        !self.files_hidden || self.list_page(cx)
    }

    pub(super) fn list_page(&self, cx: &App) -> bool {
        matches!(
            self.store.read(cx).selection,
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
        let theme = Theme::get(cx).clone();
        let selection = self.selection(cx);
        let body: AnyElement = match &selection {
            None if self.store.read(cx).projects.is_empty() => welcome(&theme, cx),
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
                        self.render_uncommitted_side(project, None, &theme, window, cx)
                    }
                    Selection::Commit(project, id) => {
                        self.render_commit_side(project, *id, &theme, cx)
                    }
                    Selection::Branch(project, ref_name) => {
                        self.render_branch_side(project, ref_name, &theme, cx)
                    }
                    Selection::History(project) => {
                        self.render_history_side(project, &theme, window, cx)
                    }
                    Selection::Worktree(project, name) => {
                        self.render_uncommitted_side(project, Some(name), &theme, window, cx)
                    }
                    Selection::Stack(project, bottom) => {
                        self.render_stack_side(project, bottom, &theme, cx)
                    }
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
                        None => self.render_upstream_side(project, &theme, cx),
                    },
                };
                let width = self.files_width(window, cx);
                // A page's header runs across it, over its files and its diff, as but.dev's
                // does. `view_header_height` lists the same pages.
                let header = match selection {
                    Selection::Commit(project, id) => {
                        Some(self.render_commit_header(project, *id, &theme, cx))
                    }
                    Selection::Branch(project, ref_name) => {
                        Some(self.render_branch_header(project, ref_name, &theme, cx))
                    }
                    Selection::Stack(project, bottom) => {
                        Some(self.render_stack_header(project, bottom, &theme, cx))
                    }
                    Selection::Project(project) => {
                        Some(self.render_project_header(project, &theme, cx))
                    }
                    _ => None,
                };
                // Beside a list, the selected branch or commit's tabs when it has a pull
                // request, over the pull request or the diff.
                let beside = match selection {
                    Selection::Branches(project) | Selection::Project(project) => (self
                        .selected_review(cx))
                    .map(|review| self.render_list_tabs(project, review, &theme, cx)),
                    // What the selected operation did, over the files it changed.
                    Selection::History(project) => {
                        self.render_operation_header(project, &theme, cx)
                    }
                    _ => None,
                };
                let files_shown = self.files_shown(cx);
                let panes = match selection {
                    Selection::Branch(project, _) if self.showing_review(cx) => {
                        self.render_review_tab(project, &theme, cx)
                    }
                    _ => h_flex()
                        .w_full()
                        .flex_1()
                        .min_h(px(0.))
                        .items_start()
                        .on_drag_move(resize_pane(Pane::Files, self.store.clone()))
                        .when(files_shown, |el| {
                            el.child(
                                v_flex()
                                    .w(px(width))
                                    .flex_none()
                                    .relative()
                                    .h_full()
                                    .border_r_1()
                                    .border_color(theme.border_3)
                                    .track_focus(&self.files_focus)
                                    .key_context("Files")
                                    .child(side)
                                    .child(resize_handle(Pane::Files, self.store.clone(), &theme)),
                            )
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
        let context = self.key_context();
        v_flex()
            .id("detail")
            .key_context(context)
            .when(context != "Detail", |el| resolve::conflict_keys(el, cx))
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NextFile, _, cx| this.move_file(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousFile, _, cx| this.move_file(-1, cx)))
            .on_action(cx.listener(|this, _: &PickNextFile, _, cx| this.pick_step(1, cx)))
            .on_action(cx.listener(|this, _: &PickPreviousFile, _, cx| this.pick_step(-1, cx)))
            .on_action(cx.listener(|this, _: &PickAllFiles, _, cx| this.pick_all_files(cx)))
            .on_action(cx.listener(|this, _: &NextHunk, _, cx| this.move_hunk(true, cx)))
            .on_action(cx.listener(|this, _: &PreviousHunk, _, cx| this.move_hunk(false, cx)))
            .on_action(cx.listener(|this, _: &DiscardHunk, window, cx| {
                if let Some(hunk) = this.cursor.hunk {
                    this.discard(this.cursor.file, Some(hunk), window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &DiscardFile, window, cx| {
                match this.file_picks.is_empty() {
                    true => this.discard(this.cursor.file, None, window, cx),
                    false => this.discard_picked(window, cx),
                }
            }))
            .on_action(
                cx.listener(|this, _: &ToggleFold, _, cx| this.toggle_fold(this.cursor.file, cx)),
            )
            .on_action(cx.listener(|this, _: &PageDown, _, cx| this.page(1., cx)))
            .on_action(cx.listener(|this, _: &PageUp, _, cx| this.page(-1., cx)))
            .on_action(cx.listener(|this, _: &ToggleWrap, _, cx| this.toggle_wrap(cx)))
            .on_action(cx.listener(|this, _: &TogglePlainLines, _, cx| this.toggle_plain_lines(cx)))
            .on_action(cx.listener(|this, _: &ToggleSplit, _, cx| this.toggle_split(cx)))
            .on_action(
                cx.listener(|this, _: &FilterFiles, window, cx| this.open_filter(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleFiles, window, cx| {
                if this.list_page(cx) {
                    return;
                }
                this.files_hidden = !this.files_hidden;
                // Keys go on to the diff once the list they went to is gone.
                if this.files_hidden && this.files_focus.is_focused(window) {
                    window.focus(&this.focus, cx);
                }
                cx.notify();
            }))
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
                el.on_action(
                    cx.listener(|this, _: &AmendTop, window, cx| this.amend_top(window, cx)),
                )
            })
            .size_full()
            .bg(theme.bg_1)
            .child(self.render_header(selection.as_ref(), &theme, cx))
            .child(div().flex_1().min_h(px(0.)).child(body))
    }
}

/// Whether `path` has every one of `terms` in it.
fn keeps_path(terms: &[String], path: &str) -> bool {
    if terms.is_empty() {
        return true;
    }
    let path = path.to_lowercase();
    terms.iter().all(|term| path.contains(term.as_str()))
}

/// What the app shows before it has a project: what it's for, and the two ways in.
fn welcome(theme: &Theme, cx: &Context<Detail>) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(18.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_1)
                .child("Welcome to GitButler"),
        )
        .child(
            div()
                .max_w(px(420.))
                .text_center()
                .text_size(px(13.))
                .text_color(theme.text_2)
                .child(
                    "Add a repository your coding agents work in. What they change shows up \
                     here as it happens, to review, commit and stack into branches.",
                ),
        )
        .child(
            h_flex()
                .mt(px(6.))
                .gap(px(8.))
                .child(
                    button("welcome-add", "Add a repository…")
                        .style(ButtonStyle::Pop)
                        .tooltip_with_key(
                            "Pick a folder with a Git repository",
                            shortcut(&AddProject, cx),
                        )
                        .dispatch(&AddProject),
                )
                .child(
                    button("welcome-clone", "Clone a repository…")
                        .dispatch(&crate::projects::CloneRepository),
                ),
        )
        .into_any_element()
}

fn hunk_keys(file: &FileDiff) -> Vec<HunkKey> {
    file.hunks().iter().map(|hunk| hunk.key).collect()
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
/// shows, and Esc empties it, then goes back to the list, as the navigator's filter does.
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
            InputEvent::Submit => {}
        },
    )
}

#[cfg(test)]
mod tests {
    use super::follow_hunk;
    use crate::backend::HunkKey;

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
}
