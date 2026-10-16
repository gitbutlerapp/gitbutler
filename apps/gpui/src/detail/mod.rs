//! The right side: whatever is selected in the navigator, with its files and diff.
//! Uncommitted changes are for reviewing what an agent did, and committing it;
//! a commit shows its message.

mod diff_view;
mod panes;

use std::collections::HashSet;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, KeyBinding, ListAlignment,
    ListOffset, ListState, SharedString, Subscription, Task, WeakEntity, Window, actions, div,
    prelude::*, px,
};

use crate::app::AppView;
use crate::backend::{
    self, Destination, DiffBody, DiffSource, FileChange, FileDiff, HunkKey, Selected,
};
use crate::state::{Pane, ProjectId, Review, Selection, Store, StoreEvent};
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
    files: Vec<FileDiff>,
    rows: Vec<DiffRow>,
}

pub struct Detail {
    store: Entity<Store>,
    /// For app-level actions like pushing, which live on the root view.
    app: WeakEntity<AppView>,
    focus: FocusHandle,
    diff: Option<LoadedDiff>,
    diff_error: Option<SharedString>,
    loading: Option<DiffKey>,
    /// The worktree changed while its diff was loading; load again when done.
    reload_queued: bool,
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
    /// Files folded or unfolded by hand; otherwise reviewed files fold.
    folded: HashSet<String>,
    unfolded: HashSet<String>,
    /// The diff takes the whole window.
    pub zoomed: bool,
    /// The header starts where the window's traffic lights are.
    pub beside_traffic_lights: bool,
    _subscriptions: Vec<Subscription>,
}

impl Detail {
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
                    if this.selected_project(cx).as_ref() == Some(project) {
                        this.refresh_diff(false, cx);
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
            // Reviewing a file folds it, and review state lives in the store.
            cx.observe(&store, |this, _, cx| this.rebuild_rows(cx)),
        ];
        let mut detail = Self {
            store,
            app,
            focus: cx.focus_handle(),
            diff: None,
            diff_error: None,
            loading: None,
            reload_queued: false,
            _load: Task::ready(()),
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            message,
            commit_target: None,
            unchecked: HashSet::new(),
            unchecked_hunks: HashSet::new(),
            rewording: None,
            checks_project: None,
            composing: false,
            cursor: Cursor::default(),
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
        self.loading = Some(key.clone());
        let source = key.source;
        self._load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || backend::load_diff(&path, source)).await;
            this.update(cx, |this, cx| {
                if this.loading.as_ref() != Some(&key) {
                    return;
                }
                this.loading = None;
                match result {
                    Ok(files) => {
                        let folded = this.folded_paths(&key, &files, cx);
                        let rows = diff::rows(&files, |file| folded.contains(&file.file.path));
                        let same = this.diff.as_ref().is_some_and(|diff| diff.key == key);
                        if same {
                            let top = this.list.logical_scroll_top();
                            this.list.reset(rows.len());
                            this.list.scroll_to(top);
                        } else {
                            this.list.reset(rows.len());
                        }
                        if same {
                            this.cursor = this.cursor_in(&files);
                        }
                        if key.source == DiffSource::Worktree {
                            let present: HashSet<&str> =
                                files.iter().map(|f| f.file.path.as_str()).collect();
                            this.unchecked
                                .retain(|path| present.contains(path.as_str()));
                            let hunks: HashSet<(&str, HunkKey)> = files
                                .iter()
                                .flat_map(|f| {
                                    hunk_keys(f)
                                        .into_iter()
                                        .map(|key| (f.file.path.as_str(), key))
                                })
                                .collect();
                            this.unchecked_hunks
                                .retain(|(path, key)| hunks.contains(&(path.as_str(), *key)));
                        }
                        this.diff = Some(LoadedDiff { key, files, rows });
                        this.diff_error = None;
                    }
                    Err(err) => this.diff_error = Some(format!("{err:#}").into()),
                }
                if std::mem::take(&mut this.reload_queued) {
                    this.refresh_diff(false, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// What dragging `project`'s uncommitted changes carries: the checked files when
    /// they're shown here, otherwise all of them.
    pub fn files_to_move(&self, project: &ProjectId, cx: &App) -> Vec<FileChange> {
        if self.selection(cx) == Some(Selection::Uncommitted(project.clone())) {
            return self.checked_uncommitted(cx);
        }
        self.store
            .read(cx)
            .workspace(project)
            .map(|ws| ws.uncommitted.clone())
            .unwrap_or_default()
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
    fn rebuild_rows(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            cx.notify();
            return;
        };
        let folded = self.folded_paths(&diff.key, &diff.files, cx);
        let rows = diff::rows(&diff.files, |file| folded.contains(&file.file.path));
        if rows != diff.rows {
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
        let Some(file) = files.iter().position(|f| f.file.path == old.file.path) else {
            return Cursor::default();
        };
        let hunk = self.cursor.hunk.and_then(|ix| {
            let key = *hunk_keys(old).get(ix)?;
            hunk_keys(&files[file]).iter().position(|k| *k == key)
        });
        Cursor { file, hunk }
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
        cx.notify();
    }

    /// Uncommitted files that are checked for the next commit.
    pub fn checked_uncommitted(&self, cx: &App) -> Vec<FileChange> {
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
        files: &[FileChange],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_target = Some(branch);
        if let Some(project) = self.selected_project(cx)
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let keep: HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
            self.unchecked = workspace
                .uncommitted
                .iter()
                .filter(|f| !keep.contains(f.path.as_str()))
                .map(|f| f.path.clone())
                .collect();
        }
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
                            move |path| backend::reword(path, id, &message),
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
        let files = self.selected_changes(cx);
        if files.is_empty() {
            self.store
                .update(cx, |store, cx| store.snack("Nothing checked to commit", cx));
            return;
        }
        let target = self.effective_target(&project, cx);
        let message_input = self.message.clone();
        let detail = cx.entity().downgrade();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Commit",
                move |path| {
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
                    .child(self.render_diff(&theme, cx))
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
