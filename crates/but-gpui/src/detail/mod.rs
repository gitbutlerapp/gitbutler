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
mod panes;
mod pull_request;
mod resolve;
mod upstream;
mod worktree;

pub use diff_view::DiffPane;
pub use resolve::ResolveConflicts;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    ListAlignment, ListOffset, ListState, ScrollHandle, SharedString, Subscription, Task,
    UniformListScrollHandle, WeakEntity, Window, actions, div, prelude::*, px,
};

use crate::app::{AddProject, AppView};
use crate::backend::edit::Editing;
use crate::backend::{self, DiffLine, DiffSource, FileChange, FileDiff, HunkKey, Selected};
use crate::state::{Pane, ProjectId, Review, Selection, Store, StoreEvent};
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
        NextHunk,
        PreviousHunk,
        ToggleReviewed,
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

/// Reviewing keys follow Zed's agent review: cmd-y keeps (here: marks reviewed) and
/// cmd-alt-z rejects (here: discards). Moving between files and hunks follows Lite.
pub fn bind_keys(cx: &mut App) {
    let c = Some("Detail && !TextInput");
    cx.bind_keys([
        KeyBinding::new("j", NextFile, c),
        KeyBinding::new("k", PreviousFile, c),
        KeyBinding::new("down", NextHunk, c),
        KeyBinding::new("up", PreviousHunk, c),
        KeyBinding::new("cmd-y", ToggleReviewed, c),
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
    /// Where the review marks of this diff's files are kept, when it's uncommitted changes:
    /// of the project's own checkout or of one of its linked worktrees.
    fn review_scope(&self) -> Option<ProjectId> {
        match &self.source {
            DiffSource::Worktree => Some(self.project.clone()),
            DiffSource::Linked(name) => {
                Some(crate::state::worktree_review_scope(&self.project, name))
            }
            _ => None,
        }
    }

    /// The uncommitted files this diff follows, as the store has them: the project's
    /// own checkout's, or a linked worktree's. `None` for a diff of commits.
    fn uncommitted<'a>(&self, store: &'a Store) -> Option<&'a [FileChange]> {
        let workspace = store.workspace(&self.project)?;
        match &self.source {
            DiffSource::Worktree => Some(&workspace.uncommitted),
            DiffSource::Linked(name) => (workspace.worktrees.iter())
                .find(|worktree| worktree.name == *name)
                .map(|worktree| &worktree.uncommitted[..]),
            _ => None,
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

/// How long a diff loads before the pane says it's loading.
const SHOW_LOADING: Duration = Duration::from_millis(150);

/// How tall the uncommitted changes' commit footer is, and the composer's row of buttons.
pub const FOOTER_HEIGHT: f32 = 44.;

struct LoadedDiff {
    key: DiffKey,
    files: Arc<Vec<FileDiff>>,
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
    message: Entity<TextInput>,
    /// The branch (full ref) new commits go to; `None` creates a branch.
    commit_target: Option<String>,
    checks: composer::Checks,
    /// Narrows the file list and the diff to the paths it matches, while `filtering`.
    file_filter: Entity<TextInput>,
    filtering: bool,
    /// Files shown whole, by path, with their lines once read.
    whole: HashMap<String, Option<FileLines>>,
    /// Files whose lines are being read, one read each at a time.
    whole_reading: HashSet<String>,
    /// How far each file's lines are scrolled sideways, by path: the old side's and the new
    /// side's, which a unified diff scrolls.
    sideways: HashMap<String, [ScrollHandle; 2]>,
    /// The commit message being edited, with the subscription that saves it.
    rewording: Option<(Entity<TextInput>, Subscription)>,
    /// The project the checks and commit target above belong to.
    checks_project: Option<ProjectId>,
    /// The commit message box is open; until then there are no checkboxes.
    composing: bool,
    /// Where the focus was when writing a commit message started, to go back to after.
    composer_back: Option<FocusHandle>,
    cursor: Cursor,
    /// Whether the selected commit's message shows its body, not just its title.
    message_open: bool,
    /// How tall a commit's header across the page was drawn, which the diff starts below.
    header_height: Rc<Cell<f32>>,
    /// Directories folded shut in tree-shaped file lists, by path.
    collapsed_dirs: HashSet<String>,
    /// Text selected in the diff, from where the drag started to where it is now.
    text_selection: Option<(TextPoint, TextPoint)>,
    /// The left button went down on a line and hasn't come up yet.
    selecting: bool,
    /// Files folded or unfolded by hand; otherwise reviewed files fold.
    folded: HashSet<String>,
    unfolded: HashSet<String>,
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
    _branch_commits_load: Task<()>,
    /// The History page's operations, for the project it shows.
    history: Option<history::History>,
    history_scroll: UniformListScrollHandle,
    _history_load: Task<()>,
    /// The file list beside the diff is hidden, for more room.
    pub files_hidden: bool,
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
        let message = cx.new(|cx| TextInput::new("Commit message", true, cx));
        let branch_filter = cx.new(|cx| TextInput::new("Filter branches", false, cx));
        let file_filter = cx.new(|cx| TextInput::new("Filter files", false, cx));
        let subscriptions = vec![
            cx.subscribe(&store, |this, _, event, cx| match event {
                StoreEvent::SelectionChanged => {
                    // The message being edited was of the commit as it was.
                    this.rewording = None;
                    // A commit an operation rewrote is still the one being looked at.
                    let followed =
                        (this.key_for_selection(cx)).is_some_and(|key| this.follows(&key, cx));
                    if !followed {
                        this.message_open = false;
                        this.reset_view();
                    }
                    let project = this.selected_project(cx);
                    if this.checks_project != project {
                        this.checks_project = project;
                        this.close_composer(cx);
                        this.commit_target = None;
                    }
                    this.reload_page(true, cx);
                    this.refresh_diff(true, cx);
                }
                StoreEvent::Loaded(project) => {
                    if this.selected_project(cx).as_ref() == Some(project) {
                        this.reload_page(false, cx);
                        // A reload that didn't touch the files shown, like the forge poll's,
                        // shouldn't re-highlight them.
                        let unchanged = (this.store.read(cx).workspace(project))
                            .is_some_and(|ws| this.shows_worktree(&ws.uncommitted));
                        if !unchanged {
                            this.refresh_diff(false, cx);
                        }
                    }
                }
                StoreEvent::Reviewed(project) | StoreEvent::Pending(project) => {
                    // Marks of a linked worktree's files come under their own scope.
                    let scope = (this.diff.as_ref()).and_then(|diff| diff.key.review_scope());
                    if this.selected_project(cx).as_ref() == Some(project)
                        || scope.as_ref() == Some(project)
                    {
                        this.rebuild_rows(cx);
                        if let StoreEvent::Pending(_) = event {
                            this.skip_taken(cx);
                        }
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
                            this.cursor = Cursor { file, hunk: None };
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
            cx.subscribe(&branch_filter, |this, _, event, cx| {
                if let InputEvent::Changed = event {
                    this.select_shown_branch(cx);
                    cx.notify()
                }
            }),
            // What the side panel shows comes from the store.
            cx.observe(&store, |_, _, cx| cx.notify()),
        ];
        let mut detail = Self {
            store,
            app,
            focus: cx.focus_handle(),
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
            message,
            commit_target: None,
            checks: Default::default(),
            file_filter,
            filtering: false,
            whole: HashMap::new(),
            whole_reading: HashSet::new(),
            sideways: HashMap::new(),
            rewording: None,
            checks_project: None,
            composing: false,
            composer_back: None,
            cursor: Cursor::default(),
            message_open: false,
            header_height: Rc::new(Cell::new(panes::COMMIT_HEADER_HEIGHT)),
            collapsed_dirs: HashSet::new(),
            text_selection: None,
            selecting: false,
            folded: HashSet::new(),
            unfolded: HashSet::new(),
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
            _branch_commits_load: Task::ready(()),
            history: None,
            history_scroll: UniformListScrollHandle::new(),
            _history_load: Task::ready(()),
            files_hidden: false,
            beside_traffic_lights: false,
            _subscriptions: subscriptions,
        };
        detail.refresh_diff(true, cx);
        detail
    }

    fn selection(&self, cx: &App) -> Option<Selection> {
        self.store.read(cx).selection.clone()
    }

    fn selected_project(&self, cx: &App) -> Option<ProjectId> {
        self.selection(cx).map(|s| s.project().clone())
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
            Selection::Commit(_, id) => DiffSource::Commit(*id),
            Selection::Branch(project, ref_name) => {
                let branch = store.workspace(project)?.find_branch(ref_name)?;
                let newest = branch.commits.first()?.id;
                let oldest = branch.commits.last()?.id;
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
            _ => {}
        }
    }

    /// Forget how the last diff was looked at: its text selection, cursor and folds. For a
    /// new selection, or another item picked on the same page.
    fn reset_view(&mut self) {
        self.text_selection = None;
        self.cursor = Cursor::default();
        self.folded.clear();
        self.unfolded.clear();
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
        if let Some(files) = recent.clone()
            && !key.source.live()
            && files.iter().all(|file| file.annotated)
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
        self._load = cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            // About a screen's worth: the rest follow highlighted a moment later.
            const FIRST: usize = 6;
            let result = backend::run({
                let (path, source, worktree) = (path.clone(), source.clone(), worktree.clone());
                move || backend::load_diff(&path, source, worktree, previous, Some(FIRST))
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
                            let files = Arc::new(files);
                            if files.iter().any(|file| !file.annotated) {
                                shown = Some(files.clone());
                            }
                            this.show_diff(key.clone(), files, cx);
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
            let result =
                backend::run(move || backend::load_diff(&path, source, worktree, previous, None))
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
        const RECENT: usize = 16;
        self.recent.retain(|(recent, _)| *recent != key);
        if self.recent.len() == RECENT {
            self.recent.remove(0);
        }
        self.recent.push((key.clone(), files.clone()));
        let rows = self.build_rows(&key, &files, cx);
        let top = same.then(|| self.keep_place(&rows, &files));
        // Most rows are a line tall: rows not drawn yet count as one, so the scrollbar's thumb
        // is about the right size before they are.
        (self.list).reset_with_uniform_height(rows.len(), px(Theme::get(cx).code_line));
        if let Some(top) = top {
            self.list.scroll_to(top);
            self.cursor = self.cursor_in(&files);
        }
        if key.source == DiffSource::Worktree {
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
        self.diff = Some(LoadedDiff { key, files, rows });
        self.prune_resolving();
        for file in stale {
            self.load_whole(file, cx);
        }
        self.run_find(false, cx);
        self.diff_error = None;
    }

    /// What dragging `project`'s uncommitted changes carries: the checked files and hunks
    /// when they're shown here, otherwise all of them.
    pub fn files_to_move(&self, project: &ProjectId, cx: &App) -> Vec<Selected> {
        if self.selection(cx) == Some(Selection::Uncommitted(project.clone())) {
            return self.selected_changes(cx);
        }
        let store = self.store.read(cx);
        let files = store
            .workspace(project)
            .map_or(&[][..], |ws| &ws.uncommitted);
        files
            .iter()
            .map(|file| Selected {
                file: file.clone(),
                hunks: None,
            })
            .collect()
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }

    /// The diff's rows. Files folded by hand, or reviewed and not unfolded, show only their
    /// header; uncommitted files and hunks, or files of the commit shown, that a pending
    /// operation takes away are left out until it's done.
    fn build_rows(&self, key: &DiffKey, files: &[FileDiff], cx: &App) -> Vec<DiffRow> {
        let store = self.store.read(cx);
        let worktree = key.source == DiffSource::Worktree;
        let commit = match key.source {
            DiffSource::Commit(id) => store.key_of(&key.project, id),
            _ => None,
        };
        let scope = key.review_scope();
        let terms = self.filter_terms(cx);
        diff::rows(
            files,
            |file| {
                let path = &file.file.path;
                let reviewed = (scope.as_ref())
                    .is_some_and(|scope| store.review(scope, &file.file) == Review::Reviewed);
                !self.unfolded.contains(path) && (self.folded.contains(path) || reviewed)
            },
            |file| {
                let path = &file.file.path;
                (worktree && store.hides_file(&key.project, path))
                    || (commit.as_ref())
                        .is_some_and(|commit| store.hides_commit_file(&key.project, commit, path))
                    || !keeps_path(&terms, path)
            },
            |file, hunk| worktree && store.hides_hunk(&key.project, &file.file.path, hunk.key),
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

    /// Step `delta` files through the ones shown.
    fn move_file(&mut self, delta: isize, cx: &mut Context<Self>) {
        let shown = self.shown_files();
        if shown.is_empty() {
            return;
        }
        // From a file that isn't shown, the next shown one counts as a step.
        let at = shown.partition_point(|file| *file < self.cursor.file) as isize;
        let at = if shown.contains(&self.cursor.file) || delta < 0 {
            at
        } else {
            at - 1
        };
        let file = shown[(at + delta).clamp(0, shown.len() as isize - 1) as usize];
        self.scroll_to_file(file, cx);
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
            self.cursor = Cursor {
                file,
                hunk: Some(hunk),
            };
            self.list.scroll_to_reveal_item(row);
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
            self.cursor = Cursor { file, hunk };
        }
    }

    /// The loaded diff when it's of the project's own uncommitted changes, the ones the
    /// composer commits.
    fn worktree_diff(&self) -> Option<&LoadedDiff> {
        (self.diff.as_ref()).filter(|diff| diff.key.source == DiffSource::Worktree)
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
            && matches!(&store.selection, Some(Selection::Uncommitted(project))
            if store.workspace(project).is_some_and(|ws| !ws.uncommitted.is_empty()))
    }

    /// The commit composer is open.
    pub fn composing(&self) -> bool {
        self.composing
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

    /// The path of the file under the cursor, in any diff.
    pub fn cursor_path(&self) -> Option<String> {
        let diff = self.diff.as_ref()?;
        Some(diff.files.get(self.cursor.file)?.file.path.clone())
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

    pub fn toggle_reviewed_file(&mut self, file_ix: usize, cx: &mut Context<Self>) {
        self.cursor = Cursor {
            file: file_ix,
            hunk: None,
        };
        let Some(diff) = &self.diff else {
            return;
        };
        let (Some(project), Some(file)) = (diff.key.review_scope(), diff.files.get(file_ix)) else {
            return;
        };
        if !self.shown_files().contains(&file_ix) {
            return;
        }
        let file = file.clone();
        let reviewed = self.store.read(cx).review(&project, &file.file) == Review::Reviewed;
        self.unfolded.remove(&file.file.path);
        self.store.update(cx, |store, cx| {
            store.set_reviewed(&project, &[file.file], !reviewed, cx)
        });
        // Fold it now, so the scroll below lands on the next file's header.
        self.rebuild_rows(cx);
        if !reviewed {
            // Like Zed's keep-and-next: move on to the next file still wanting a look.
            let store = self.store.read(cx);
            let shown = self.shown_files();
            let next = self.diff.as_ref().and_then(|diff| {
                diff.files
                    .iter()
                    .enumerate()
                    .skip(file_ix + 1)
                    .filter(|(ix, _)| shown.binary_search(ix).is_ok())
                    .find(|(_, f)| store.review(&project, &f.file) != Review::Reviewed)
                    .map(|(ix, _)| ix)
            });
            if let Some(next) = next {
                self.scroll_to_file(next, cx);
            }
        }
    }

    pub fn toggle_fold(&mut self, file_ix: usize, cx: &mut Context<Self>) {
        let Some(path) = self
            .diff
            .as_ref()
            .and_then(|diff| diff.files.get(file_ix))
            .map(|f| f.file.path.clone())
        else {
            return;
        };
        if self.is_folded(file_ix) {
            self.folded.remove(&path);
            self.unfolded.insert(path);
        } else {
            self.unfolded.remove(&path);
            self.folded.insert(path);
        }
        self.cursor = Cursor {
            file: file_ix,
            hunk: None,
        };
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
        self.cursor = Cursor {
            file: file_ix,
            hunk,
        };
        let Some(diff) = (self.diff.as_ref()).filter(|diff| diff.key.source.live()) else {
            return;
        };
        let Some(file) = diff.files.get(file_ix) else {
            return;
        };
        if !self.shown_files().contains(&file_ix) {
            return;
        }
        let hunks = match hunk.map(|ix| file.hunks().get(ix)) {
            None => None,
            Some(Some(hunk)) => Some(vec![hunk.key]),
            // Never widen a hunk discard to the whole file.
            Some(None) => return,
        };
        let (project, worktree) = (diff.key.project.clone(), diff.key.linked());
        let change = Selected {
            file: file.file.clone(),
            hunks,
        };
        self.app
            .update(cx, |app, cx| {
                app.discard(project, worktree, vec![change], window, cx)
            })
            .ok();
    }

    fn scroll_to_file(&mut self, file_ix: usize, cx: &mut Context<Self>) {
        self.cursor = Cursor {
            file: file_ix,
            hunk: None,
        };
        if let Some(row) = self.row_of(|row| matches!(row, DiffRow::File(ix) if *ix == file_ix)) {
            self.list.scroll_to(ListOffset {
                item_ix: row,
                offset_in_item: px(0.),
            });
            cx.notify();
        }
    }

    /// How far below the breadcrumbs the diff starts: under a commit's header, when there's one.
    pub fn view_header_height(&self, cx: &App) -> f32 {
        match self.selection(cx) {
            Some(Selection::Commit(..)) => self.header_height.get(),
            _ => 0.,
        }
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
                "Nothing selected",
                "Pick a project, branch or commit on the left",
                &theme,
            ),
            Some(selection) => {
                let side = match selection {
                    Selection::Uncommitted(project) => {
                        self.render_uncommitted_side(project, &theme, window, cx)
                    }
                    Selection::Commit(project, id) => {
                        self.render_commit_side(project, *id, &theme, cx)
                    }
                    Selection::Branch(project, ref_name) => {
                        self.render_branch_side(project, ref_name, &theme, cx)
                    }
                    Selection::History(project) => self.render_history_side(project, &theme, cx),
                    Selection::Worktree(project, name) => {
                        self.render_worktree_side(project, name, &theme, cx)
                    }
                    Selection::Branches(project) => {
                        self.render_branches_side(project, &theme, window, cx)
                    }
                    Selection::Project(project) => match self.editing(project, cx) {
                        Some(editing) => empty_state(
                            commit_title(&editing.title),
                            format!(
                                "Commit {} is checked out for editing. Finish it with \
                                 `but resolve finish`, or cancel with `but resolve cancel`.",
                                editing.commit.short()
                            ),
                            &theme,
                        ),
                        None => self.render_upstream_side(project, &theme, cx),
                    },
                };
                let width = self.store.read(cx).fitted_width(
                    Pane::Files,
                    window.viewport_size().width.into(),
                    !self.beside_traffic_lights,
                    true,
                );
                // A commit's header runs across the page, over its files and its diff.
                let header = match selection {
                    Selection::Commit(project, id) => {
                        Some(self.render_commit_header(project, *id, &theme, cx))
                    }
                    _ => None,
                };
                let panes = h_flex()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
                    .items_start()
                    .on_drag_move(resize_pane(Pane::Files, self.store.clone()))
                    .when(!self.files_hidden, |el| {
                        el.child(
                            v_flex()
                                .w(px(width))
                                .flex_none()
                                .relative()
                                .h_full()
                                .border_r_1()
                                .border_color(theme.border_3)
                                .child(side)
                                .child(resize_handle(Pane::Files, self.store.clone(), &theme)),
                        )
                    })
                    // `DiffPane` draws the diff over this space.
                    .child(div().flex_1());
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
            .on_action(cx.listener(|this, _: &NextHunk, _, cx| this.move_hunk(true, cx)))
            .on_action(cx.listener(|this, _: &PreviousHunk, _, cx| this.move_hunk(false, cx)))
            .on_action(cx.listener(|this, _: &ToggleReviewed, _, cx| {
                this.toggle_reviewed_file(this.cursor.file, cx)
            }))
            .on_action(cx.listener(|this, _: &DiscardHunk, window, cx| {
                if let Some(hunk) = this.cursor.hunk {
                    this.discard(this.cursor.file, Some(hunk), window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &DiscardFile, window, cx| {
                this.discard(this.cursor.file, None, window, cx)
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
            .on_action(cx.listener(|this, _: &ToggleFiles, _, cx| {
                this.files_hidden = !this.files_hidden;
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
