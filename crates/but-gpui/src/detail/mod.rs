//! The right side: whatever is selected in the navigator, with its files and diff.
//! Uncommitted changes are for reviewing what an agent did, and committing it;
//! a commit shows its message.

mod branches;
mod composer;
mod diff_view;
mod editing;
mod file_tree;
mod find;
pub use find::{FindInDiff, bind_keys as bind_find_keys};
mod history;
mod panes;
mod pull_request;
mod upstream;

pub use diff_view::DiffPane;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    ListAlignment, ListOffset, ListState, Role, SharedString, Subscription, Task, Toggled,
    UniformListScrollHandle, WeakEntity, Window, actions, div, prelude::*, px,
};

use crate::app::{AddProject, AppView};
use crate::backend::{
    self, DiffBody, DiffLine, DiffSource, FileChange, FileDiff, HunkKey, Selected,
};
use crate::state::{Pane, ProjectId, Review, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::ui::diff::{self, DiffRow};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, button, empty_state, h_flex, resize_handle, resize_pane, shortcut,
    small_icon, v_flex,
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
        KeyBinding::new("alt-z", ToggleWrap, c),
        KeyBinding::new("cmd-b", ToggleSplit, c),
        KeyBinding::new("f", ToggleFiles, c),
        KeyBinding::new("/", FilterFiles, c),
        // Works while typing the message too, as the composer's other key does.
        KeyBinding::new("alt-cmd-enter", AmendTop, Some("Detail")),
    ]);
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

/// How long a diff loads before the pane says it's loading.
const SHOW_LOADING: Duration = Duration::from_millis(150);
/// How long a pull request's reviews and conversations go before a reload asks again.
const FEEDBACK_STALE: Duration = Duration::from_secs(60);

struct LoadedDiff {
    key: DiffKey,
    files: Arc<Vec<FileDiff>>,
    rows: Vec<DiffRow>,
}

/// A file's new side as diff lines, read for the patch with this fingerprint.
type FileLines = (u64, Arc<Vec<DiffLine>>);

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
    /// Uncommitted paths left out of the next commit.
    unchecked: HashSet<String>,
    /// Hunks left out of the next commit, in files that are otherwise in.
    unchecked_hunks: HashSet<(String, HunkKey)>,
    /// Hunks only some of whose changed lines go in the next commit: those lines, by index.
    line_picks: HashMap<(String, HunkKey), BTreeSet<usize>>,
    /// Narrows the file list and the diff to the paths it matches, while `filtering`.
    file_filter: Entity<TextInput>,
    filtering: bool,
    /// Files shown whole, by path, with their lines once read.
    whole: HashMap<String, Option<FileLines>>,
    /// Files whose lines are being read, one read each at a time.
    whole_reading: HashSet<String>,
    /// Conflicted files of an edited commit the user called resolved, markers and all.
    marked_resolved: HashSet<(ProjectId, backend::CommitId, String)>,
    /// The commit message being edited, with the subscription that saves it.
    rewording: Option<(Entity<TextInput>, Subscription)>,
    /// The project the checkboxes and commit target above belong to.
    checks_project: Option<ProjectId>,
    /// Its uncommitted paths a merge left conflicted, which stay unchecked until they're
    /// marked resolved.
    conflicted: HashSet<String>,
    /// The commit message box is open; until then there are no checkboxes.
    composing: bool,
    cursor: Cursor,
    /// Whether the selected commit's message shows its body, not just its title.
    message_open: bool,
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
    /// The project page's target history, for the project it shows.
    upstream: Option<upstream::Upstream>,
    upstream_scroll: UniformListScrollHandle,
    _upstream_load: Task<()>,
    /// Where the review of the selected branch's pull request stands, once the forge said:
    /// the project and pull request number it's for.
    feedback: Option<(ProjectId, u64, backend::reviews::Feedback)>,
    /// When it was last asked for and how the branch stood then, to ask again as the
    /// workspace reloads: once a while has passed, or the branch moved or was pushed.
    feedback_asked: Option<(Instant, pull_request::BranchMark)>,
    _feedback_load: Task<()>,
    /// The Branches page's branches, for the project it shows, and its filter.
    branches: Option<branches::Branches>,
    branch_filter: Entity<TextInput>,
    branches_scroll: UniformListScrollHandle,
    _branches_load: Task<()>,
    /// The History page's operations, for the project it shows.
    history: Option<history::History>,
    history_scroll: UniformListScrollHandle,
    _history_load: Task<()>,
    /// The diff takes the whole window.
    pub zoomed: bool,
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
                    this.rewording = None;
                    this.message_open = false;
                    this.text_selection = None;
                    this.cursor = Cursor::default();
                    this.folded.clear();
                    this.unfolded.clear();
                    let project = this.selected_project(cx);
                    if this.checks_project != project {
                        this.checks_project = project;
                        this.close_composer(cx);
                        this.commit_target = None;
                    }
                    this.refresh_conflicted(cx);
                    match this.selection(cx) {
                        Some(Selection::History(project)) => this.load_history(&project, false, cx),
                        Some(Selection::Branches(project)) => {
                            this.load_branches(&project, true, cx)
                        }
                        Some(Selection::Project(project)) => {
                            this.load_upstream(&project, false, true, cx)
                        }
                        Some(Selection::Branch(project, ref_name)) => {
                            this.load_feedback(&project, &ref_name, cx)
                        }
                        _ => {}
                    }
                    this.refresh_diff(true, cx);
                }
                StoreEvent::Loaded(project) => {
                    this.refresh_conflicted(cx);
                    // A push can dismiss approvals; a reviewer can weigh in any time.
                    if let Some(Selection::Branch(shown, ref_name)) = this.selection(cx)
                        && shown == *project
                    {
                        let mark = this.branch_mark(&shown, &ref_name, cx);
                        let stale = this.feedback_asked.as_ref().is_none_or(|(at, asked)| {
                            at.elapsed() >= FEEDBACK_STALE || *asked != mark
                        });
                        if stale {
                            this.load_feedback(&shown, &ref_name, cx)
                        }
                    }
                    // Calling a file resolved lasts as long as that edit does.
                    if this.editing(project, cx).is_none() {
                        this.marked_resolved.retain(|(p, ..)| p != project);
                    }
                    match this.selection(cx) {
                        Some(Selection::History(shown)) if shown == *project => {
                            this.load_history(project, false, cx)
                        }
                        Some(Selection::Branches(shown)) if shown == *project => {
                            this.load_branches(project, false, cx)
                        }
                        Some(Selection::Project(shown)) if shown == *project => {
                            this.load_upstream(project, false, false, cx)
                        }
                        _ => {}
                    }
                    // A reload that didn't touch the files shown, like the forge poll's,
                    // shouldn't re-highlight them.
                    let store = this.store.read(cx);
                    let unchanged = store
                        .workspace(project)
                        .is_some_and(|ws| this.shows_worktree(&ws.uncommitted));
                    if this.selected_project(cx).as_ref() == Some(project) && !unchanged {
                        this.refresh_diff(false, cx);
                    }
                }
                StoreEvent::Reviewed(project) | StoreEvent::Pending(project) => {
                    if this.selected_project(cx).as_ref() == Some(project) {
                        this.rebuild_rows(cx);
                    }
                }
            }),
            cx.subscribe_in(
                &file_filter,
                window,
                |this, _, event, window, cx| match event {
                    InputEvent::Changed => this.rebuild_rows(cx),
                    InputEvent::Submit => window.focus(&this.focus, cx),
                    InputEvent::Cancel => this.close_filter(window, cx),
                },
            ),
            cx.subscribe_in(&message, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.commit(window, cx),
                InputEvent::Cancel => {
                    this.close_composer(cx);
                    window.focus(&this.focus, cx);
                }
                // The input draws itself; nothing here shows the message, and notifying
                // would redraw the diff on every keystroke.
                InputEvent::Changed => {}
            }),
            cx.subscribe(&branch_filter, |_, _, event, cx| {
                if let InputEvent::Changed = event {
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
            list: ListState::new(0, ListAlignment::Top, px(150.)),
            uncommitted_files: UniformListScrollHandle::new(),
            diff_files: UniformListScrollHandle::new(),
            message,
            commit_target: None,
            unchecked: HashSet::new(),
            unchecked_hunks: HashSet::new(),
            line_picks: HashMap::new(),
            file_filter,
            filtering: false,
            whole: HashMap::new(),
            whole_reading: HashSet::new(),
            marked_resolved: HashSet::new(),
            rewording: None,
            checks_project: None,
            conflicted: HashSet::new(),
            composing: false,
            cursor: Cursor::default(),
            message_open: false,
            collapsed_dirs: HashSet::new(),
            text_selection: None,
            selecting: false,
            folded: HashSet::new(),
            unfolded: HashSet::new(),
            writing: None,
            find: None,
            upstream: None,
            upstream_scroll: UniformListScrollHandle::new(),
            _upstream_load: Task::ready(()),
            feedback: None,
            feedback_asked: None,
            _feedback_load: Task::ready(()),
            branches: None,
            branch_filter,
            branches_scroll: UniformListScrollHandle::new(),
            _branches_load: Task::ready(()),
            history: None,
            history_scroll: UniformListScrollHandle::new(),
            _history_load: Task::ready(()),
            zoomed: false,
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

    fn key_for_selection(&self, cx: &App) -> Option<DiffKey> {
        let selection = self.selection(cx)?;
        let store = self.store.read(cx);
        let source = match &selection {
            Selection::Project(project) if self.editing(project, cx).is_some() => DiffSource::Edit,
            Selection::Project(project) => self.upstream_source(project)?,
            Selection::History(project) => self.history_source(project)?,
            Selection::Branches(project) => self.branches_source(project)?,
            Selection::Uncommitted(_) => DiffSource::Worktree,
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

    /// Load the diff for the current selection. `selection_changed` resets the scroll;
    /// otherwise only a worktree diff, which can change under us, is reloaded.
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
            if selection_changed {
                // The previous diff stays until this one lands, usually a frame or two
                // later, rather than flashing an empty pane; a slow load says so.
                self.waiting_since = Some(Instant::now());
                self._waiting = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(SHOW_LOADING).await;
                    this.update(cx, |_, cx| cx.notify()).ok();
                });
            }
        }
        let Some(path) = self.store.read(cx).path(&key.project) else {
            return;
        };
        let source = key.source;
        let worktree = (source.live())
            .then(|| self.store.read(cx).workspace(&key.project))
            .flatten()
            .map(|ws| ws.uncommitted.clone());
        if source.live() && worktree.is_none() {
            // The workspace is still loading and lists the files itself; diffing now would run
            // a second status on the same repository, slowing both. Its `Loaded` brings us back.
            return;
        }
        self.loading = Some(key.clone());
        let previous = self
            .recent
            .iter()
            .find(|(recent, _)| *recent == key)
            .map(|(_, files)| files.clone());
        if let Some(files) = previous.clone()
            && !key.source.live()
            && files.iter().all(|file| file.annotated)
        {
            self.loading = None;
            self.show_diff(key, files, cx);
            cx.notify();
            return;
        }
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
        self._load = cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            // Only the files likely on screen are highlighted first; the rest follow.
            const FIRST: usize = 16;
            let (first_path, first_worktree) = (path.clone(), worktree.clone());
            let result = backend::run(move || {
                backend::load_diff(&first_path, source, first_worktree, previous, Some(FIRST))
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
                        Err(err) => this.diff_error = Some(format!("{err:#}").into()),
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

    /// Show a loaded diff, keeping the scroll position and checks when it's a reload.
    fn show_diff(&mut self, key: DiffKey, files: Arc<Vec<FileDiff>>, cx: &mut Context<Self>) {
        if self.diff.as_ref().is_none_or(|diff| diff.key != key) {
            self.whole.clear();
        }
        let before = (self.recent.iter())
            .find(|(recent, _)| *recent == key)
            .map(|(_, files)| files.clone());
        const RECENT: usize = 16;
        self.recent.retain(|(recent, _)| *recent != key);
        if self.recent.len() == RECENT {
            self.recent.remove(0);
        }
        self.recent.push((key.clone(), files.clone()));
        let rows = self.build_rows(&key, &files, cx);
        let same = self.diff.as_ref().is_some_and(|diff| diff.key == key);
        let top = same.then(|| self.keep_place(&rows, &files));
        // Most rows are a line tall: rows not drawn yet count as one, so the scrollbar's thumb
        // is about the right size before they are.
        let line = Theme::get(cx).code_line;
        self.list.reset_with_uniform_height(rows.len(), px(line));
        if let Some(top) = top {
            self.list.scroll_to(top);
            self.cursor = self.cursor_in(&files);
        }
        if key.source == DiffSource::Worktree {
            let present: HashSet<&str> = files.iter().map(|f| f.file.path.as_str()).collect();
            self.unchecked
                .retain(|path| present.contains(path.as_str()));
            let hunks: HashSet<(&str, HunkKey)> = files
                .iter()
                .flat_map(|f| {
                    hunk_keys(f)
                        .into_iter()
                        .map(|key| (f.file.path.as_str(), key))
                })
                .collect();
            // A hunk left out stays out when an edit renumbers it: what's out now is every
            // hunk over the same committed lines.
            let renumbered = |path: &str, key: HunkKey| {
                let file = files.iter().find(|f| f.file.path == path);
                (file.map(hunk_keys).unwrap_or_default().into_iter())
                    .filter(move |now| covers_same_lines(*now, key))
            };
            self.unchecked_hunks = (self.unchecked_hunks.iter())
                .flat_map(|(path, key)| match hunks.contains(&(path.as_str(), *key)) {
                    true => vec![(path.clone(), *key)],
                    false => renumbered(path, *key)
                        .map(|now| (path.clone(), now))
                        .collect(),
                })
                .collect();
            // Picked lines are by their place in the hunk, which holds only while the
            // hunk's lines stay the same; an edit can change them under the same header.
            let lines = |files: &[FileDiff], path: &str, key: HunkKey| {
                let file = files.iter().find(|f| f.file.path == path)?;
                match &file.body {
                    DiffBody::Hunks { hunks, .. } => hunks.iter().find(|h| h.key == key),
                    _ => None,
                }
                .map(|hunk| {
                    (hunk.lines.iter())
                        .map(|line| (line.kind, line.text.clone()))
                        .collect::<Vec<_>>()
                })
            };
            // A hunk whose picks can't be trusted any more is left out rather than taken
            // whole: committing lines that weren't picked is the worse mistake.
            let stale: Vec<(String, HunkKey)> = (self.line_picks.keys())
                .filter(|(path, key)| {
                    let then = before.as_deref().and_then(|b| lines(b, path, *key));
                    then.is_none() || then != lines(&files, path, *key)
                })
                .cloned()
                .collect();
            for (path, key) in stale {
                self.line_picks.remove(&(path.clone(), key));
                // An edit can renumber the hunk, but the lines it covers on the committed
                // side stay put: every hunk over them now is left out.
                for now in renumbered(&path, key) {
                    self.unchecked_hunks.insert((path.clone(), now));
                }
            }
        }
        if self.diff.as_ref().is_none_or(|diff| diff.rows != rows) {
            self.text_selection = None;
        }
        let stale: Vec<usize> = (files.iter().enumerate())
            .filter(|(_, file)| {
                matches!(self.whole.get(&file.file.path),
                    Some(Some((fingerprint, _))) if *fingerprint != file.fingerprint)
            })
            .map(|(ix, _)| ix)
            .collect();
        self.diff = Some(LoadedDiff { key, files, rows });
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
    /// header; uncommitted files a pending operation takes away are left out until it's done.
    fn build_rows(&self, key: &DiffKey, files: &[FileDiff], cx: &App) -> Vec<DiffRow> {
        let store = self.store.read(cx);
        let worktree = key.source == DiffSource::Worktree;
        let terms = self.filter_terms(cx);
        diff::rows(
            files,
            |file| {
                let path = &file.file.path;
                !self.unfolded.contains(path)
                    && (self.folded.contains(path)
                        || (worktree && store.review(&key.project, &file.file) == Review::Reviewed))
            },
            |file| {
                (worktree && store.hides_file(&key.project, &file.file.path))
                    || !keeps_path(&terms, &file.file.path)
            },
            |file| match self.whole.get(&file.file.path) {
                Some(Some((fingerprint, lines))) if *fingerprint == file.fingerprint => {
                    Some(lines.len() as u32)
                }
                _ => None,
            },
            store.prefs.split,
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

    pub(super) fn close_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let Some(diff) = &self.diff else {
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
                    Some(row.in_file(now))
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
            let line = Theme::get(cx).code_line;
            self.list.reset_with_uniform_height(rows.len(), px(line));
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
            next.is_some_and(|row| matches!(diff.rows.get(row + 1), Some(DiffRow::Gap)))
        })
    }

    fn diff_index(&self, path: &str) -> Option<usize> {
        self.diff
            .as_ref()?
            .files
            .iter()
            .position(|f| f.file.path == path)
    }

    /// Wrap long lines, or stop. Rows change height, so the list measures them again.
    pub fn toggle_split(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.split = !prefs.split, cx)
        });
        self.rebuild_rows(cx);
    }

    pub fn toggle_wrap(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.wrap = !prefs.wrap, cx)
        });
        self.remeasure(cx);
    }

    /// Show code in diffs at `size`, in this window and any other.
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
        let line = Theme::get(cx).code_line;
        self.list.reset_with_uniform_height(count, px(line));
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
        let hunk = self.cursor.hunk.and_then(|ix| {
            let key = *hunk_keys(old).get(ix)?;
            hunk_keys(&files[file]).iter().position(|k| *k == key)
        });
        Cursor { file, hunk }
    }

    /// Whether the loaded diff is the worktree's and shows exactly `files` as they are now.
    fn shows_worktree(&self, files: &[FileChange]) -> bool {
        self.diff.as_ref().is_some_and(|diff| {
            diff.key.source == DiffSource::Worktree
                && diff.files.len() == files.len()
                && diff.files.iter().zip(files).all(|(shown, now)| {
                    shown.file.path == now.path
                        && shown.file.modified == now.modified
                        && shown.file.base() == now.base()
                })
        })
    }

    /// The path of the file under the cursor, in any diff.
    /// Open the file under the cursor in the editor, at its hunk.
    fn open_in_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let Some(file) = diff.files.get(self.cursor.file) else {
            return;
        };
        let line = match (&file.body, self.cursor.hunk) {
            (DiffBody::Hunks { hunks, .. }, Some(ix)) => hunks.get(ix).map(|h| h.key.0.new_start),
            _ => None,
        };
        let (project, path) = (diff.key.project.clone(), file.file.path.clone());
        let position = window.mouse_position();
        self.app
            .update(cx, |app, cx| {
                app.open_in_editor(&project, Some(path), line, position, cx)
            })
            .ok();
    }

    pub fn cursor_path(&self) -> Option<String> {
        let diff = self.diff.as_ref()?;
        Some(diff.files.get(self.cursor.file)?.file.path.clone())
    }

    /// The uncommitted file under the cursor, with the project it's in. Never a file the
    /// diff doesn't show, as one the filter hides.
    fn cursor_file(&self) -> Option<(ProjectId, FileDiff)> {
        let diff = self
            .diff
            .as_ref()
            .filter(|diff| diff.key.source == DiffSource::Worktree)?;
        if !self.shown_files().contains(&self.cursor.file) {
            return None;
        }
        let file = diff.files.get(self.cursor.file)?;
        Some((diff.key.project.clone(), file.clone()))
    }

    /// The files the diff shows, in order: not those a filter or a pending operation hides.
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
        let Some((project, file)) = self.cursor_file() else {
            return;
        };
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
                    .filter(|(ix, _)| shown.contains(ix))
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

    /// Throw away one hunk, or the whole file when `hunk` is `None`. Undo brings it back.
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
        let Some((project, file)) = self.cursor_file() else {
            return;
        };
        let hunks = match (hunk, &file.body) {
            (None, _) => None,
            (Some(hunk), DiffBody::Hunks { hunks, .. }) if hunk < hunks.len() => {
                Some(vec![hunks[hunk].key])
            }
            // Never widen a hunk discard to the whole file.
            (Some(_), _) => return,
        };
        let change = Selected {
            file: file.file,
            hunks,
        };
        self.app
            .update(cx, |app, cx| app.discard(project, vec![change], window, cx))
            .ok();
    }

    /// Show `path` in the loaded diff, at `line` of its new side when the diff has it.
    /// Returns whether the diff has the file.
    pub(super) fn reveal_line(
        &mut self,
        path: &str,
        line: Option<u32>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(diff) = self.shown_diff() else {
            return false;
        };
        let Some(file_ix) = diff.files.iter().position(|f| f.file.path == path) else {
            return false;
        };
        let new_of = |hunk: usize, ix: usize| match &diff.files[file_ix].body {
            DiffBody::Hunks { hunks, .. } => hunks.get(hunk)?.lines.get(ix)?.new,
            _ => None,
        };
        let row = line.and_then(|line| {
            self.row_of(|row| match *row {
                DiffRow::Line {
                    file,
                    hunk,
                    line: ix,
                    ..
                }
                | DiffRow::Pair {
                    file,
                    hunk,
                    right: Some(ix),
                    ..
                } => file == file_ix && new_of(hunk, ix) == Some(line),
                DiffRow::Context { file, new, .. } => file == file_ix && new == line,
                _ => false,
            })
        });
        // The file filter can leave the file out entirely.
        let shown = row.is_some()
            || self
                .row_of(|row| matches!(row, DiffRow::File(ix) if *ix == file_ix))
                .is_some();
        if !shown {
            return false;
        }
        match row {
            Some(row) => {
                self.cursor = Cursor {
                    file: file_ix,
                    hunk: None,
                };
                // A few lines above it, for what leads up to it.
                self.list.scroll_to(ListOffset {
                    item_ix: row.saturating_sub(3),
                    offset_in_item: px(0.),
                });
                cx.notify();
            }
            None => self.scroll_to_file(file_ix, cx),
        }
        true
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
                "Pick a project, branch or commit on the left.",
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
                    Selection::Branches(project) => {
                        self.render_branches_side(project, &theme, window, cx)
                    }
                    Selection::Project(project) => match self.editing(project, cx) {
                        Some(editing) => self.render_edit_side(project, editing, &theme, cx),
                        None => self.render_upstream_side(project, &theme, cx),
                    },
                };
                let width = self.store.read(cx).width(Pane::Files);
                h_flex()
                    .size_full()
                    .items_start()
                    .on_drag_move(resize_pane(Pane::Files, self.store.clone()))
                    .when(!self.zoomed && !self.files_hidden, |el| {
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
                    .child(div().flex_1())
                    .into_any_element()
            }
        };
        v_flex()
            .id("detail")
            .key_context("Detail")
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

/// Whether two hunks touch any of the same lines of the committed side, or lines next to
/// them: leaning toward yes keeps what was left out of a commit out of it.
fn covers_same_lines(a: HunkKey, b: HunkKey) -> bool {
    let lines = |key: HunkKey| {
        let start = key.0.old_start;
        start.saturating_sub(1)..start + key.0.old_lines.max(1) + 1
    };
    let (a, b) = (lines(a), lines(b));
    a.start < b.end && b.start < a.end
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
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(AddProject), cx)),
                )
                .child(
                    button("welcome-clone", "Clone a repository…").on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(crate::projects::CloneRepository), cx)
                    }),
                ),
        )
        .into_any_element()
}

fn hunk_keys(file: &FileDiff) -> Vec<HunkKey> {
    match &file.body {
        DiffBody::Hunks { hunks, .. } => hunks.iter().map(|hunk| hunk.key).collect(),
        _ => Vec::new(),
    }
}

pub(super) fn checkbox_box(
    id: impl Into<gpui::ElementId>,
    check: Toggled,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    let on = check != Toggled::False;
    div()
        .id(id)
        .role(Role::CheckBox)
        .aria_toggled(check)
        .flex_none()
        .size(px(14.))
        .rounded(px(3.))
        .border_1()
        .border_color(if on { theme.pop } else { theme.border_2 })
        .when(on, |el| el.bg(theme.pop))
        .flex()
        .items_center()
        .justify_center()
        .map(|el| match check {
            Toggled::True => el.child(
                small_icon(Icon::Tick)
                    .size(px(10.))
                    .text_color(gpui::white()),
            ),
            Toggled::Mixed => el.child(div().w(px(8.)).h(px(2.)).rounded(px(1.)).bg(gpui::white())),
            Toggled::False => el,
        })
}

/// A spot in the diff's text: a row of the diff and a byte offset in that row's line.
type TextPoint = (usize, usize);

#[cfg(test)]
mod tests {
    use super::covers_same_lines;
    use crate::backend::HunkKey;

    fn key(old_start: u32, old_lines: u32) -> HunkKey {
        HunkKey(but_core::HunkHeader {
            old_start,
            old_lines,
            new_start: old_start,
            new_lines: old_lines,
        })
    }

    #[test]
    fn renumbered_hunks_cover_the_same_committed_lines() {
        assert!(
            covers_same_lines(key(10, 7), key(12, 3)),
            "a hunk grown around the old one covers its lines"
        );
        assert!(
            covers_same_lines(key(5, 0), key(6, 1)),
            "an insertion after line 5 touches line 6 next to it"
        );
        assert!(
            !covers_same_lines(key(1, 7), key(40, 7)),
            "hunks far apart don't"
        );
    }
}
