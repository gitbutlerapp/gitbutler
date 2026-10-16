//! The right side: whatever is selected in the navigator, with its files and diff.
//! Uncommitted changes are for reviewing what an agent did, and committing it;
//! a commit shows its message.

mod diff_view;
mod file_tree;
mod panes;

pub use diff_view::DiffPane;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    ListAlignment, ListOffset, ListState, SharedString, Subscription, Task, WeakEntity, Window,
    actions, div, prelude::*, px,
};

use crate::app::AppView;
use crate::backend::{
    self, Destination, DiffBody, DiffLine, DiffSource, FileChange, FileDiff, HunkKey, Selected,
};
use crate::state::{Overlay, Pane, ProjectId, Review, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::transfer::short_ref;
use crate::ui::diff::{self, DiffRow};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{Icon, empty_state, h_flex, resize_handle, resize_pane, small_icon, v_flex};

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

struct LoadedDiff {
    key: DiffKey,
    files: Arc<Vec<FileDiff>>,
    rows: Vec<DiffRow>,
}

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
    list: ListState,
    message: Entity<TextInput>,
    /// The branch (full ref) new commits go to; `None` creates a branch.
    commit_target: Option<String>,
    /// Uncommitted paths left out of the next commit.
    unchecked: HashSet<String>,
    /// Hunks left out of the next commit, in files that are otherwise in.
    unchecked_hunks: HashSet<(String, HunkKey)>,
    /// The commit message being edited, with the subscription that saves it.
    rewording: Option<(Entity<TextInput>, Subscription)>,
    /// The project the checkboxes and commit target above belong to.
    checks_project: Option<ProjectId>,
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
    /// The diff takes the whole window.
    pub zoomed: bool,
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
                    this.refresh_diff(true, cx);
                }
                StoreEvent::Loaded(project) => {
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
            cx.subscribe_in(&message, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.commit(window, cx),
                InputEvent::Cancel => {
                    this.close_composer(cx);
                    window.focus(&this.focus, cx);
                }
                InputEvent::Changed => cx.notify(),
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
            list: ListState::new(0, ListAlignment::Top, px(150.)),
            message,
            commit_target: None,
            unchecked: HashSet::new(),
            unchecked_hunks: HashSet::new(),
            rewording: None,
            checks_project: None,
            composing: false,
            cursor: Cursor::default(),
            message_open: false,
            collapsed_dirs: HashSet::new(),
            text_selection: None,
            selecting: false,
            folded: HashSet::new(),
            unfolded: HashSet::new(),
            zoomed: false,
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
            Selection::Project(_) => return None,
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
            Some(key) => current != Some(key) || key.source == DiffSource::Worktree,
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
                // Don't leave the previous selection's diff on screen while loading.
                self.diff = None;
                self.list.reset(0);
            }
        }
        let Some(path) = self.store.read(cx).path(&key.project) else {
            return;
        };
        let source = key.source;
        let worktree = (source == DiffSource::Worktree)
            .then(|| self.store.read(cx).workspace(&key.project))
            .flatten()
            .map(|ws| ws.uncommitted.clone());
        if source == DiffSource::Worktree && worktree.is_none() {
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
            && key.source != DiffSource::Worktree
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
        const RECENT: usize = 16;
        self.recent.retain(|(recent, _)| *recent != key);
        if self.recent.len() == RECENT {
            self.recent.remove(0);
        }
        self.recent.push((key.clone(), files.clone()));
        let folded = self.folded_paths(&key, &files, cx);
        let hidden = self.hidden_paths(&key, &files, cx);
        let rows = diff::rows(
            &files,
            |file| folded.contains(&file.file.path),
            |file| hidden.contains(&file.file.path),
        );
        let same = self.diff.as_ref().is_some_and(|diff| diff.key == key);
        if same {
            let top = self.list.logical_scroll_top();
            self.list.reset(rows.len());
            self.list.scroll_to(top);
            self.cursor = self.cursor_in(&files);
        } else {
            self.list.reset(rows.len());
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
            self.unchecked_hunks
                .retain(|(path, key)| hunks.contains(&(path.as_str(), *key)));
        }
        if self.diff.as_ref().is_none_or(|diff| diff.rows != rows) {
            self.text_selection = None;
        }
        self.diff = Some(LoadedDiff { key, files, rows });
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

    /// Files that show only their header: folded by hand, or reviewed and not unfolded.
    fn folded_paths(&self, key: &DiffKey, files: &[FileDiff], cx: &App) -> HashSet<String> {
        let store = self.store.read(cx);
        files
            .iter()
            .filter(|file| {
                let path = &file.file.path;
                !self.unfolded.contains(path)
                    && (self.folded.contains(path)
                        || (key.source == DiffSource::Worktree
                            && store.review(&key.project, &file.file) == Review::Reviewed))
            })
            .map(|file| file.file.path.clone())
            .collect()
    }

    /// Rebuild the rows when folding changed, keeping the scroll position.
    /// Uncommitted files a pending operation takes away, left out until it's done.
    fn hidden_paths(&self, key: &DiffKey, files: &[FileDiff], cx: &App) -> HashSet<String> {
        if key.source != DiffSource::Worktree {
            return HashSet::new();
        }
        let store = self.store.read(cx);
        files
            .iter()
            .map(|file| &file.file.path)
            .filter(|path| store.hides_file(&key.project, path))
            .cloned()
            .collect()
    }

    fn rebuild_rows(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            cx.notify();
            return;
        };
        let folded = self.folded_paths(&diff.key, &diff.files, cx);
        let hidden = self.hidden_paths(&diff.key, &diff.files, cx);
        let rows = diff::rows(
            &diff.files,
            |file| folded.contains(&file.file.path),
            |file| hidden.contains(&file.file.path),
        );
        if rows != diff.rows {
            self.text_selection = None;
            // Keep the row at the top of the view there, or its file's header when the
            // row just folded away.
            let top = self.list.logical_scroll_top();
            let anchor = diff.rows.get(top.item_ix).copied();
            let scroll_to = match anchor.and_then(|row| rows.iter().position(|r| *r == row)) {
                Some(item_ix) => ListOffset { item_ix, ..top },
                None => ListOffset {
                    item_ix: anchor
                        .and_then(DiffRow::file)
                        .and_then(|file| rows.iter().position(|r| *r == DiffRow::File(file)))
                        .unwrap_or(top.item_ix),
                    offset_in_item: px(0.),
                },
            };
            self.list.reset(rows.len());
            self.list.scroll_to(scroll_to);
            if let Some(diff) = &mut self.diff {
                diff.rows = rows;
            }
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

    /// The index of `path` in the loaded diff.
    fn diff_index(&self, path: &str) -> Option<usize> {
        self.diff
            .as_ref()?
            .files
            .iter()
            .position(|f| f.file.path == path)
    }

    fn row_of(&self, wanted: impl Fn(&DiffRow) -> bool) -> Option<usize> {
        self.diff.as_ref()?.rows.iter().position(wanted)
    }

    fn move_file(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(count) = self
            .diff
            .as_ref()
            .map(|diff| diff.files.len())
            .filter(|n| *n > 0)
        else {
            return;
        };
        let file = (self.cursor.file as isize + delta).clamp(0, count as isize - 1) as usize;
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

    fn next_file(&mut self, _: &NextFile, _: &mut Window, cx: &mut Context<Self>) {
        self.move_file(1, cx);
    }

    fn previous_file(&mut self, _: &PreviousFile, _: &mut Window, cx: &mut Context<Self>) {
        self.move_file(-1, cx);
    }

    fn next_hunk(&mut self, _: &NextHunk, _: &mut Window, cx: &mut Context<Self>) {
        self.move_hunk(true, cx);
    }

    fn previous_hunk(&mut self, _: &PreviousHunk, _: &mut Window, cx: &mut Context<Self>) {
        self.move_hunk(false, cx);
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

    /// The diff line at `row`, if that row is one.
    pub(super) fn line_at(&self, row: usize) -> Option<&DiffLine> {
        let diff = self.diff.as_ref()?;
        let DiffRow::Line {
            file, hunk, line, ..
        } = *diff.rows.get(row)?
        else {
            return None;
        };
        match &diff.files.get(file)?.body {
            DiffBody::Hunks { hunks, .. } => hunks.get(hunk)?.lines.get(line),
            _ => None,
        }
    }

    /// The selection in order, unless it's empty.
    fn text_range(&self) -> Option<(TextPoint, TextPoint)> {
        let (anchor, head) = self.text_selection?;
        let (start, end) = if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        };
        (start != end).then_some((start, end))
    }

    /// The selected part of the line at `row`, which is `len` bytes long.
    /// The selected part of the line at `row`. Nothing when the line changed under the
    /// selection so its ends no longer fall in the text, as after an edit in place.
    pub(super) fn selected_in_row(&self, row: usize) -> Option<Range<usize>> {
        let (start, end) = self.text_range()?;
        if row < start.0 || row > end.0 {
            return None;
        }
        let text = &self.line_at(row)?.text;
        let from = if row == start.0 { start.1 } else { 0 };
        let to = if row == end.0 { end.1 } else { text.len() };
        let fits = text.is_char_boundary(from) && text.is_char_boundary(to);
        (from < to && fits).then_some(from..to)
    }

    /// The selected text, one line per diff line, without the `+` and `-` signs.
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.text_range()?;
        let lines: Vec<&str> = (start.0..=end.0)
            .filter_map(|row| {
                let text = &self.line_at(row)?.text;
                Some(&text[self.selected_in_row(row).unwrap_or(0..0)])
            })
            .collect();
        Some(lines.join("\n"))
    }

    /// The pointer went down on a line: start selecting there, take the word on a double
    /// click and the line on a triple, or reach to it with shift held.
    pub(super) fn start_text_selection(
        &mut self,
        at: TextPoint,
        clicks: usize,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (row, ix) = at;
        let text = self.line_at(row).map_or("", |line| line.text.as_str());
        // The text may have changed since the line was drawn.
        let ix = (0..=ix.min(text.len()))
            .rev()
            .find(|ix| text.is_char_boundary(*ix))
            .unwrap_or(0);
        let at = (row, ix);
        self.text_selection = match clicks {
            2 => {
                let word = word_at(text, ix);
                Some(((row, word.start), (row, word.end)))
            }
            3.. => Some(((row, 0), (row, text.len()))),
            _ => match self.text_selection {
                Some((anchor, _)) if extend => Some((anchor, at)),
                _ => Some((at, at)),
            },
        };
        self.selecting = true;
        if let Some(DiffRow::Line { file, hunk, .. }) = self
            .diff
            .as_ref()
            .and_then(|diff| diff.rows.get(row))
            .copied()
        {
            self.cursor = Cursor {
                file,
                hunk: Some(hunk),
            };
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub(super) fn drag_text_selection(&mut self, at: TextPoint, cx: &mut Context<Self>) {
        if let (true, Some((_, head))) = (self.selecting, &mut self.text_selection)
            && *head != at
        {
            *head = at;
            cx.notify();
        }
    }

    /// Drop the text selection, saying whether there was one.
    pub fn clear_text_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had = self.text_range().is_some();
        self.text_selection = None;
        cx.notify();
        had
    }

    /// The path of the file under the cursor, in any diff.
    pub fn cursor_path(&self) -> Option<String> {
        let diff = self.diff.as_ref()?;
        Some(diff.files.get(self.cursor.file)?.file.path.clone())
    }

    /// The uncommitted file under the cursor, with the project it's in.
    fn cursor_file(&self) -> Option<(ProjectId, FileDiff)> {
        let diff = self
            .diff
            .as_ref()
            .filter(|diff| diff.key.source == DiffSource::Worktree)?;
        let file = diff.files.get(self.cursor.file)?;
        Some((diff.key.project.clone(), file.clone()))
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
            let next = self.diff.as_ref().and_then(|diff| {
                diff.files
                    .iter()
                    .enumerate()
                    .skip(file_ix + 1)
                    .find(|(_, f)| store.review(&project, &f.file) != Review::Reviewed)
                    .map(|(ix, _)| ix)
            });
            if let Some(next) = next {
                self.scroll_to_file(next, cx);
            }
        }
    }

    fn toggle_reviewed(&mut self, _: &ToggleReviewed, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_reviewed_file(self.cursor.file, cx);
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

    fn toggle_fold_action(&mut self, _: &ToggleFold, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_fold(self.cursor.file, cx);
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

    fn discard_hunk(&mut self, _: &DiscardHunk, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(hunk) = self.cursor.hunk {
            self.discard(self.cursor.file, Some(hunk), window, cx);
        }
    }

    fn discard_file(&mut self, _: &DiscardFile, window: &mut Window, cx: &mut Context<Self>) {
        self.discard(self.cursor.file, None, window, cx);
    }

    fn close_composer(&mut self, cx: &mut Context<Self>) {
        self.composing = false;
        self.unchecked.clear();
        self.unchecked_hunks.clear();
        cx.emit(ChecksChanged);
        cx.notify();
    }

    /// Uncommitted files that are checked for the next commit.
    fn checked_uncommitted(&self, cx: &App) -> Vec<FileChange> {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return Vec::new();
        };
        let Some(workspace) = self.store.read(cx).workspace(&project) else {
            return Vec::new();
        };
        workspace
            .uncommitted
            .iter()
            .filter(|file| !self.unchecked.contains(&file.path))
            .cloned()
            .collect()
    }

    /// The checked uncommitted changes: checked files, narrowed to their checked hunks.
    pub fn selected_changes(&self, cx: &App) -> Vec<Selected> {
        self.checked_uncommitted(cx)
            .into_iter()
            .filter_map(|file| {
                let hunks = self.worktree_hunks(&file.path);
                // Hunks left out but not loaded to tell which: leave the file alone rather
                // than take all of it.
                let some_left_out = self
                    .unchecked_hunks
                    .iter()
                    .any(|(path, _)| *path == file.path);
                if hunks.is_empty() && some_left_out {
                    return None;
                }
                let picked: Vec<HunkKey> = hunks
                    .iter()
                    .filter(|key| !self.unchecked_hunks.contains(&(file.path.clone(), **key)))
                    .copied()
                    .collect();
                if picked.is_empty() && !hunks.is_empty() {
                    return None;
                }
                let partial = picked.len() < hunks.len();
                Some(Selected {
                    file,
                    hunks: partial.then_some(picked),
                })
            })
            .collect()
    }

    fn worktree_hunks(&self, path: &str) -> Vec<HunkKey> {
        self.diff
            .as_ref()
            .filter(|diff| diff.key.source == DiffSource::Worktree)
            .and_then(|diff| diff.files.iter().find(|f| f.file.path == path))
            .map(hunk_keys)
            .unwrap_or_default()
    }

    fn file_check(&self, path: &str) -> Check {
        if self.unchecked.contains(path) {
            return Check::None;
        }
        let excluded = self
            .unchecked_hunks
            .iter()
            .filter(|(hunk_path, _)| hunk_path == path)
            .count();
        if excluded == 0 {
            Check::All
        } else {
            Check::Some
        }
    }

    fn hunk_checked(&self, path: &str, key: HunkKey) -> bool {
        !self.unchecked.contains(path) && !self.unchecked_hunks.contains(&(path.to_owned(), key))
    }

    fn toggle_hunk(&mut self, path: &str, key: HunkKey, cx: &mut Context<Self>) {
        let entry = (path.to_owned(), key);
        if self.unchecked.remove(path) {
            // The whole file was out; bring back only this hunk.
            for other in self.worktree_hunks(path) {
                if other != key {
                    self.unchecked_hunks.insert((path.to_owned(), other));
                }
            }
        } else if !self.unchecked_hunks.remove(&entry) {
            self.unchecked_hunks.insert(entry);
        }
        let hunks = self.worktree_hunks(path);
        if hunks
            .iter()
            .all(|key| self.unchecked_hunks.contains(&(path.to_owned(), *key)))
        {
            self.unchecked_hunks
                .retain(|(hunk_path, _)| hunk_path != path);
            self.unchecked.insert(path.to_owned());
        }
        cx.emit(ChecksChanged);
        cx.notify();
    }

    pub fn focus_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composing = true;
        window.focus(&self.message.focus_handle(cx), cx);
        cx.notify();
    }

    /// Get `files` ready to commit to `branch`, after they were dropped on it.
    pub fn stage(
        &mut self,
        branch: String,
        files: &[Selected],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_target = Some(branch);
        if let Some(project) = self.selected_project(cx)
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let keep: HashMap<&str, Option<&[HunkKey]>> = files
                .iter()
                .map(|f| (f.file.path.as_str(), f.hunks.as_deref()))
                .collect();
            self.unchecked = workspace
                .uncommitted
                .iter()
                .filter(|f| !keep.contains_key(f.path.as_str()))
                .map(|f| f.path.clone())
                .collect();
            // Hunks stay out only where the staged change left them out.
            self.unchecked_hunks.retain(|(path, key)| {
                keep.get(path.as_str())
                    .is_some_and(|hunks| hunks.is_some_and(|picked| !picked.contains(key)))
            });
        }
        cx.emit(ChecksChanged);
        self.focus_message(window, cx);
    }

    pub fn start_reword(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Commit(project, id)) = self.selection(cx) else {
            return;
        };
        let Some(message) = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_commit(id))
            .map(|(_, commit)| commit.message.clone())
        else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Commit message", true, cx);
            input.set_text(message, cx);
            input
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, input, event, window, cx| match event {
                InputEvent::Submit => {
                    let message = input.read(cx).text().trim().to_owned();
                    this.rewording = None;
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        store.run(
                            &project,
                            "Edit message",
                            Overlay::default(),
                            move |path, remap| backend::reword(path, remap.get(id)?, &message),
                            |store, rewritten, cx| {
                                store.follow(&rewritten, cx);
                                store.snack_undoable("Message updated", cx)
                            },
                            cx,
                        )
                    });
                    window.focus(&this.focus, cx);
                }
                InputEvent::Cancel => {
                    this.rewording = None;
                    window.focus(&this.focus, cx);
                    cx.notify();
                }
                InputEvent::Changed => {}
            },
        );
        window.focus(&input.focus_handle(cx), cx);
        self.rewording = Some((input, subscription));
        cx.notify();
    }

    fn effective_target(&self, project: &ProjectId, cx: &App) -> Option<String> {
        let workspace = self.store.read(cx).workspace(project)?;
        if let Some(target) = &self.commit_target
            && workspace.find_branch(target).is_some()
        {
            return Some(target.clone());
        }
        workspace
            .stacks
            .first()
            .and_then(|stack| stack.branches.first())
            .filter(|branch| !branch.ref_name.is_empty())
            .map(|branch| branch.ref_name.clone())
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return;
        };
        let message = self.message.read(cx).text().trim().to_owned();
        if message.is_empty() {
            self.store.update(cx, |store, cx| {
                store.snack("Write a commit message first", cx)
            });
            self.focus_message(window, cx);
            return;
        }
        let diff_shown = self.diff.as_ref().is_some_and(|diff| {
            diff.key.source == DiffSource::Worktree && diff.key.project == project
        });
        if !self.unchecked_hunks.is_empty() && !diff_shown {
            // Hunk choices are only known against a loaded diff.
            self.store.update(cx, |store, cx| {
                store.snack("Still loading the diff, try again in a moment", cx)
            });
            return;
        }
        // Files a pending commit or discard already takes, as after a quick second click.
        let store = self.store.read(cx);
        let files: Vec<Selected> = self
            .selected_changes(cx)
            .into_iter()
            .filter(|s| !store.hides_file(&project, &s.file.path))
            .collect();
        if files.is_empty() {
            self.store
                .update(cx, |store, cx| store.snack("Nothing checked to commit", cx));
            return;
        }
        let target = self.effective_target(&project, cx);
        let message_input = self.message.clone();
        let detail = cx.entity().downgrade();
        // The files leave the list and the commit shows on its branch right away.
        let overlay = Overlay {
            new_commit: target.clone().map(|branch| {
                let title = message.lines().next().unwrap_or_default().to_owned();
                (branch, title.into())
            }),
            ..Overlay::files(&files)
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Commit",
                overlay,
                move |path, _| {
                    let branch = match target {
                        Some(branch) => branch,
                        None => backend::create_branch(path, None)?,
                    };
                    let id = backend::commit(
                        path,
                        &Destination::Branch(branch.clone()),
                        &files,
                        &message,
                    )?;
                    Ok((branch, id))
                },
                move |store, (branch, (_id, left_out)), cx| {
                    message_input.update(cx, |input, cx| input.set_text("", cx));
                    detail
                        .update(cx, |detail, cx| detail.close_composer(cx))
                        .ok();
                    store.report_left_out(
                        &format!("Committed to {}", short_ref(&branch)),
                        &left_out,
                        cx,
                    );
                },
                cx,
            )
        });
    }

    fn toggle_checked(&mut self, path: &str, cx: &mut Context<Self>) {
        let check = self.file_check(path);
        self.unchecked_hunks
            .retain(|(hunk_path, _)| hunk_path != path);
        match check {
            Check::All | Check::Some => {
                self.unchecked.insert(path.to_owned());
            }
            Check::None => {
                self.unchecked.remove(path);
            }
        }
        cx.emit(ChecksChanged);
        cx.notify();
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
            None => {
                let has_projects = !self.store.read(cx).projects.is_empty();
                empty_state(
                    if has_projects {
                        "Nothing selected"
                    } else {
                        "Welcome to GitButler"
                    },
                    if has_projects {
                        "Pick a project, branch or commit on the left."
                    } else {
                        "Add a repository with ⌘O to get started."
                    },
                    &theme,
                )
            }
            Some(Selection::Project(project)) => self.render_project(project, &theme, cx),
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
                    Selection::Project(_) => unreachable!("handled above"),
                };
                let width = self.store.read(cx).width(Pane::Files);
                h_flex()
                    .size_full()
                    .items_start()
                    .on_drag_move(resize_pane(Pane::Files, self.store.clone()))
                    .when(!self.zoomed, |el| {
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
            .on_action(cx.listener(Self::next_file))
            .on_action(cx.listener(Self::previous_file))
            .on_action(cx.listener(Self::next_hunk))
            .on_action(cx.listener(Self::previous_hunk))
            .on_action(cx.listener(Self::toggle_reviewed))
            .on_action(cx.listener(Self::discard_hunk))
            .on_action(cx.listener(Self::discard_file))
            .on_action(cx.listener(Self::toggle_fold_action))
            .size_full()
            .bg(theme.bg_1)
            .child(self.render_header(selection.as_ref(), &theme, cx))
            .child(div().flex_1().min_h(px(0.)).child(body))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Check {
    All,
    Some,
    None,
}

fn hunk_keys(file: &FileDiff) -> Vec<HunkKey> {
    match &file.body {
        DiffBody::Hunks { hunks, .. } => hunks.iter().map(|hunk| hunk.key).collect(),
        _ => Vec::new(),
    }
}

pub(super) fn checkbox_box(
    id: impl Into<gpui::ElementId>,
    check: Check,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    let on = check != Check::None;
    div()
        .id(id)
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
            Check::All => el.child(
                small_icon(Icon::Tick)
                    .size(px(10.))
                    .text_color(gpui::white()),
            ),
            Check::Some => el.child(div().w(px(8.)).h(px(2.)).rounded(px(1.)).bg(gpui::white())),
            Check::None => el,
        })
}

/// A spot in the diff's text: a row of the diff and a byte offset in that row's line.
type TextPoint = (usize, usize);

/// The word around byte `ix` of `text`, or an empty range when `ix` isn't in one.
fn word_at(text: &str, ix: usize) -> Range<usize> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..ix]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(ix, |(at, _)| at);
    let end = text[ix..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(text.len(), |(at, _)| ix + at);
    start..end
}
