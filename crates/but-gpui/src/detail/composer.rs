//! The commit composer: which changes go in, the message, and the branch they land on.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash as _, Hasher as _};

use gpui::{
    AnyElement, AnyWindowHandle, App, Context, Focusable as _, Toggled, WeakEntity, Window, div,
    prelude::*,
};

use super::{ChecksChanged, Detail, DiffKey, PickCommitTarget};
use crate::backend::{
    self, Branch, Checkout, CommitId, Destination, DiffBody, DiffSource, FileChange, FileDiff,
    FullName, Hunk, HunkKey, LineKind, PushState, Selected, ai, short_ref,
};
use crate::palette::Choice;
use crate::state::{Cut, NewAt, Overlay, ProjectId, Selection, Store, snack};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, button, dp, faint, field, h_flex, icon_button, plural, shortcut,
    small_icon, v_flex,
};

/// Why the commit box, opened on changes something else committed since, as an agent can,
/// doesn't make an empty commit instead.
const COMMITTED_MEANWHILE: &str = "What was here to commit was committed meanwhile";

/// What the next commit leaves out of the uncommitted changes; the rest goes in.
#[derive(Default)]
pub(super) struct Checks {
    /// Uncommitted paths left out of the next commit.
    unchecked: HashSet<String>,
    /// Hunks left out of the next commit, in files that are otherwise in.
    unchecked_hunks: HashSet<(String, HunkKey)>,
    /// Hunks only some of whose changed lines go in the next commit: those lines by their
    /// index in the hunk, and the [`fingerprint`] of the lines they index.
    line_picks: HashMap<(String, HunkKey), (BTreeSet<usize>, u64)>,
    /// The uncommitted paths listed since the composer opened, `None` while it's closed. A
    /// file that turns up after, as an agent writes one, starts out: nobody picked it.
    known: Option<HashSet<String>>,
}

impl Checks {
    pub(super) fn file_check(&self, path: &str) -> Toggled {
        if self.unchecked.contains(path) || self.is_new(path) {
            Toggled::False
        } else if (self.unchecked_hunks.iter()).any(|(p, _)| p == path)
            || self.line_picks.keys().any(|(p, _)| p == path)
        {
            Toggled::Mixed
        } else {
            Toggled::True
        }
    }

    pub(super) fn hunk_check(&self, path: &str, key: HunkKey) -> Toggled {
        let entry = (path.to_owned(), key);
        if self.unchecked.contains(path) || self.unchecked_hunks.contains(&entry) {
            Toggled::False
        } else if self.line_picks.contains_key(&entry) {
            Toggled::Mixed
        } else {
            Toggled::True
        }
    }

    /// Whether any of a hunk goes in the next commit.
    pub(super) fn hunk_checked(&self, path: &str, key: HunkKey) -> bool {
        self.hunk_check(path, key) != Toggled::False
    }

    /// Whether the line at index `line` of a hunk goes in the next commit.
    pub(super) fn line_checked(&self, path: &str, key: HunkKey, line: usize) -> bool {
        match self.hunk_check(path, key) {
            Toggled::Mixed => (self.line_picks.get(&(path.to_owned(), key)))
                .is_some_and(|(lines, _)| lines.contains(&line)),
            check => check == Toggled::True,
        }
    }

    /// Whether `path` turned up since the composer opened, and isn't in the next commit yet.
    fn is_new(&self, path: &str) -> bool {
        (self.known.as_ref()).is_some_and(|known| !known.contains(path))
    }

    fn is_empty(&self) -> bool {
        self.unchecked.is_empty() && self.unchecked_hunks.is_empty() && self.line_picks.is_empty()
    }

    /// Put everything back in.
    fn clear(&mut self) {
        self.unchecked.clear();
        self.unchecked_hunks.clear();
        self.line_picks.clear();
        self.known = None;
    }

    /// Carry what's left out over to a reload of the worktree diff, `files`.
    pub(super) fn reconcile(&mut self, files: &[FileDiff]) {
        // A file left out stays out while it's gone, as an agent's `git stash` and `git stash
        // pop` take it away and bring it back.
        let present: HashSet<&str> = files.iter().map(|f| f.file.path.as_str()).collect();
        if let Some(known) = &mut self.known {
            for path in &present {
                if known.insert((*path).to_owned()) {
                    self.unchecked.insert((*path).to_owned());
                }
            }
        }
        let find_hunk =
            |path: &str, key: HunkKey| file_hunks(files, path).iter().find(|h| h.key == key);
        // A hunk left out stays out when an edit renumbers it: what's out now is every
        // hunk over the same committed lines.
        let renumbered = |path: &str, key: HunkKey| {
            (file_hunks(files, path).iter())
                .map(|hunk| hunk.key)
                .filter(move |now| covers_same_lines(*now, key))
        };
        self.unchecked_hunks = (self.unchecked_hunks.iter())
            .flat_map(|(path, key)| match find_hunk(path, *key) {
                Some(_) => vec![(path.clone(), *key)],
                None => renumbered(path, *key)
                    .map(|now| (path.clone(), now))
                    .collect(),
            })
            .collect();
        // Picked lines hold only while their hunk's lines stay the same. A hunk whose lines
        // changed is left out rather than taken whole: committing lines that weren't
        // picked is the worse mistake.
        let current = |path: &str, key: HunkKey| find_hunk(path, key).map(fingerprint);
        let stale: Vec<(String, HunkKey)> = (self.line_picks.iter())
            .filter(|((path, key), (_, print))| current(path, *key) != Some(*print))
            .map(|(entry, _)| entry.clone())
            .collect();
        for (path, key) in stale {
            self.line_picks.remove(&(path.clone(), key));
            let out: Vec<_> = renumbered(&path, key)
                .map(|now| (path.clone(), now))
                .collect();
            self.unchecked_hunks.extend(out);
        }
        let partly: HashSet<String> = (self.unchecked_hunks.iter())
            .map(|(path, _)| path.clone())
            .filter(|path| present.contains(path.as_str()))
            .collect();
        for path in partly {
            self.settle(&path, file_hunks(files, &path));
        }
    }

    /// A file with every one of its `hunks` left out is left out whole, as unchecking it does.
    /// No hunks say nothing, as while another checkout's diff is still shown.
    fn settle(&mut self, path: &str, hunks: &[Hunk]) {
        if hunks.is_empty() {
            return;
        }
        let out =
            (hunks.iter()).all(|hunk| self.unchecked_hunks.contains(&(path.to_owned(), hunk.key)));
        if out {
            self.unchecked_hunks
                .retain(|(hunk_path, _)| hunk_path != path);
            self.unchecked.insert(path.to_owned());
        }
    }
}

/// The hunks of `path` among `files`.
fn file_hunks<'a>(files: &'a [FileDiff], path: &str) -> &'a [Hunk] {
    (files.iter().find(|f| f.file.path == path)).map_or(&[], FileDiff::hunks)
}

/// Once an operation the composer started lands: `f` on the pane in `window`, unless it has
/// moved on from `project` by then.
fn after_landing(
    detail: &WeakEntity<Detail>,
    window: AnyWindowHandle,
    project: &ProjectId,
    cx: &mut App,
    f: impl FnOnce(&mut Detail, &mut Window, &mut Context<Detail>),
) {
    window
        .update(cx, |_, window, cx| {
            detail.update(cx, |detail, cx| {
                if detail.checks_project.as_ref() == Some(project) {
                    f(detail, window, cx)
                }
            })
        })
        .ok();
}

/// Once what the composer committed from `project`'s checkout `worktree` landed, with the
/// pane moved on to another checkout, the checks kept for coming back to it go. The one
/// shown has none kept.
fn forget_checks_left(
    detail: &WeakEntity<Detail>,
    project: &ProjectId,
    worktree: &Checkout,
    cx: &mut App,
) {
    (detail.update(cx, |detail, _| {
        (detail.other_checks).remove(&(Some(project.clone()), worktree.clone()))
    }))
    .ok();
}

/// The commit whose message is being edited: its project, its change ID, and what a plain
/// Git amend keeps of it when it gives it a new ID, as the selection follows it by.
pub(super) type Reworded = (ProjectId, String, String);

/// Whether `commit` of `project` is the one being reworded: by its change ID, or by what an
/// amend keeps of it once no commit has that ID, as the selection follows it, and not a
/// cherry-picked copy, which keeps that too.
pub(super) fn rewords(
    reworded: &Reworded,
    project: &ProjectId,
    commit: &backend::Commit,
    store: &Store,
) -> bool {
    let (of, key, authored) = reworded;
    let gone = || (store.workspace(project)).is_some_and(|ws| ws.commits().all(|c| c.key != *key));
    of == project && (*key == commit.key || (*authored == commit.authored && gone()))
}

/// A hunk's lines, as kinds and text, hashed: what line picks index into.
fn fingerprint(hunk: &Hunk) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    for line in &hunk.lines {
        (line.kind, &*line.text).hash(&mut hasher);
    }
    hasher.finish()
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

impl Detail {
    /// Close the composer for `project`'s checkout `worktree` to show next. The checks of the
    /// one left wait for coming back, as looking elsewhere shouldn't put back in what was
    /// left out of the next commit.
    pub(super) fn switch_checkout(
        &mut self,
        project: Option<ProjectId>,
        worktree: Checkout,
        cx: &mut Context<Self>,
    ) {
        let left = (self.checks_project.take(), self.checks_worktree.clone());
        let next = (self.other_checks)
            .remove(&(project.clone(), worktree.clone()))
            .unwrap_or_default();
        let mut checks = std::mem::replace(&mut self.checks, next);
        // Opened again, the composer starts from what's there then.
        checks.known = None;
        if !checks.is_empty() {
            self.other_checks.insert(left.clone(), checks);
        }
        // A message the AI was still writing goes back to what was written by hand.
        if let (Some((_, notes)), (Some(project), worktree)) = (self.writing.take(), left) {
            (self.store).update(cx, |store, _| {
                store.drafts.insert((project, worktree), notes)
            });
        }
        (self.checks_project, self.checks_worktree) = (project, worktree);
        // The message being written there, as checks are.
        let key = (self.checks_project.clone()).map(|p| (p, self.checks_worktree.clone()));
        let drafts = &self.store.read(cx).drafts;
        let draft = key
            .and_then(|key| drafts.get(&key).cloned())
            .unwrap_or_default();
        self.message.update(cx, |input, cx| input.load(draft, cx));
        self.composing = false;
        // A filter typed for the files left would hide the next checkout's, out of sight
        // while the list is hidden.
        self.filtering = false;
        self.file_filter
            .update(cx, |input, cx| input.set_text("", cx));
        cx.emit(ChecksChanged);
        cx.notify();
    }

    pub(super) fn close_composer(&mut self, cx: &mut Context<Self>) {
        self.composing = false;
        // A message still being written would land in whatever the box shows next.
        if let Some((_, notes)) = self.writing.take() {
            self.message
                .update(cx, |input, cx| input.set_text(notes, cx));
        }
        self.checks.clear();
        cx.emit(ChecksChanged);
        cx.notify();
    }

    /// The checked uncommitted changes: checked files, narrowed to their checked hunks and
    /// lines. All of them while the composer is closed.
    pub fn selected_changes(&self, cx: &App) -> Vec<Selected> {
        let Some((project, worktree)) = self.selected_checkout(cx) else {
            return Vec::new();
        };
        let store = self.store.read(cx);
        let files = (store.workspace(&project))
            .and_then(|ws| ws.uncommitted_in(&worktree))
            .unwrap_or_default();
        // Only the open composer checks files; the checks of one left, as on coming back from
        // another checkout, wait for it.
        if !self.composing {
            return files.iter().map(Selected::whole).collect();
        }
        // By path: finding each file in the diff in turn takes time squared in the files, as
        // after an `npm install` with nothing ignored.
        let loaded: HashMap<&str, &FileDiff> = (self.checks_diff().into_iter())
            .flat_map(|diff| diff.files.iter())
            .map(|f| (f.file.path.as_str(), f))
            .collect();
        (files.iter())
            .filter_map(|file| self.selected_change(file, loaded.get(file.path.as_str()).copied()))
            .collect()
    }

    /// What of uncommitted `file`, as `loaded` in the diff, is checked: the file narrowed to
    /// its checked hunks and lines, unless none of it is.
    pub(super) fn selected_change(
        &self,
        file: &FileChange,
        loaded: Option<&FileDiff>,
    ) -> Option<Selected> {
        let hunks = loaded.map_or(&[][..], FileDiff::hunks);
        if self.checks.unchecked.contains(&file.path) || self.checks.is_new(&file.path) {
            return None;
        }
        let partial = self.checks.file_check(&file.path) == Toggled::Mixed;
        // Hunks left out but not loaded to tell which: leave the file alone rather than take
        // all of it.
        if hunks.is_empty() && partial {
            return None;
        }
        let picked: Vec<HunkKey> = (hunks.iter())
            .flat_map(|hunk| match self.checks.hunk_check(&file.path, hunk.key) {
                Toggled::True => vec![hunk.key],
                Toggled::Mixed => (self.checks.line_picks.get(&(file.path.clone(), hunk.key)))
                    .map(|(lines, _)| hunk.picked(lines))
                    .unwrap_or_default(),
                Toggled::False => Vec::new(),
            })
            .collect();
        if picked.is_empty() && !hunks.is_empty() {
            return None;
        }
        Some(Selected {
            // Picked hunks go with the file as the diff read it, whose lines place them, so a
            // write since is refused rather than taking whatever lies there now.
            file: (loaded.filter(|_| partial))
                .map_or(file, |loaded| &loaded.file)
                .clone(),
            hunks: partial.then_some(picked),
        })
    }

    /// The checked changes, less the files and hunks a pending operation already takes, as
    /// after a quick second click.
    fn changes_not_pending(&self, project: &ProjectId, cx: &App) -> Vec<Selected> {
        let store = self.store.read(cx);
        let worktree = &self.checks_worktree;
        (self.selected_changes(cx).into_iter())
            .filter(|s| !store.hides_file(project, worktree, &s.file.path))
            .filter_map(|mut s| {
                if let Some(hunks) = &mut s.hunks {
                    hunks.retain(|hunk| !store.hides_hunk(project, worktree, &s.file.path, *hunk));
                    // No hunks would take the whole file.
                    if hunks.is_empty() {
                        return None;
                    }
                }
                Some(s)
            })
            .collect()
    }

    /// The hunks of `path` in the loaded worktree diff.
    pub(super) fn worktree_file_hunks(&self, path: &str) -> &[Hunk] {
        self.loaded_file(path).map_or(&[], FileDiff::hunks)
    }

    /// `path` as the diff of the composer's checkout loaded it.
    pub(super) fn loaded_file(&self, path: &str) -> Option<&FileDiff> {
        (self.checks_diff()?.files.iter()).find(|f| f.file.path == path)
    }

    /// The loaded diff when it's of the checkout the checks are about: another's stays on
    /// screen until this one's loads, and shares its paths.
    fn checks_diff(&self) -> Option<&super::LoadedDiff> {
        self.worktree_diff().filter(|diff| {
            self.checks_project.as_ref() == Some(&diff.key.project)
                && diff.key.checkout().as_ref() == Some(&self.checks_worktree)
        })
    }

    /// Whether the open composer's checks own the diff shown: not while another checkout's
    /// is still on screen.
    pub(super) fn composes(&self) -> bool {
        self.composing && self.checks_diff().is_some()
    }

    /// Check or uncheck, as a click on its checkbox does, the file under the cursor in the
    /// file list, or in the diff the hunk under it, else its file.
    pub(super) fn toggle_cursor_check(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(file) = self
            .checks_diff()
            .and_then(|diff| diff.files.get(self.cursor.file))
        else {
            return;
        };
        let path = file.file.path.clone();
        let hunk = (self.cursor.hunk)
            .filter(|_| !self.files_focus.is_focused(window))
            .and_then(|hunk| Some(file.hunks().get(hunk)?.key));
        match hunk {
            Some(key) => self.toggle_hunk(&path, key, cx),
            // The files selected with it go as it goes.
            None if self.file_picks.contains(&path) => {
                let check = self.checks.file_check(&path) == Toggled::False;
                self.set_checked(self.file_picks.to_vec(), check, cx)
            }
            None => self.toggle_checked(&path, cx),
        }
    }

    pub(super) fn toggle_hunk(&mut self, path: &str, key: HunkKey, cx: &mut Context<Self>) {
        let entry = (path.to_owned(), key);
        if self.checks.line_picks.remove(&entry).is_some() {
            // Partly in: all of it now.
        } else if self.checks.unchecked.contains(path) {
            self.bring_back_only(path, key);
        } else if !self.checks.unchecked_hunks.remove(&entry) {
            self.checks.unchecked_hunks.insert(entry);
        }
        self.settle_checks(path, cx);
    }

    /// Put one changed line of a hunk in the next commit, or take it out. Into a hunk
    /// that's out, it brings just that line.
    pub(super) fn toggle_line(
        &mut self,
        path: &str,
        key: HunkKey,
        line: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(hunk) = self.worktree_file_hunks(path).iter().find(|h| h.key == key) else {
            return;
        };
        let print = fingerprint(hunk);
        let changed: BTreeSet<usize> = (hunk.lines.iter().enumerate())
            .filter(|(_, l)| l.kind != LineKind::Context)
            .map(|(ix, _)| ix)
            .collect();
        let entry = (path.to_owned(), key);
        let mut picks = match self.checks.hunk_check(path, key) {
            Toggled::True => changed.clone(),
            Toggled::Mixed => {
                (self.checks.line_picks.remove(&entry)).map_or_else(BTreeSet::new, |(p, _)| p)
            }
            Toggled::False => BTreeSet::new(),
        };
        if !picks.remove(&line) {
            picks.insert(line);
        }
        if self.checks.unchecked.contains(path) {
            self.bring_back_only(path, key);
        }
        self.checks.unchecked_hunks.remove(&entry);
        if picks.is_empty() {
            self.checks.unchecked_hunks.insert(entry);
        } else if picks != changed {
            self.checks.line_picks.insert(entry, (picks, print));
        }
        self.settle_checks(path, cx);
    }

    /// Put back a file that was out as a whole, with only hunk `key` of it in.
    fn bring_back_only(&mut self, path: &str, key: HunkKey) {
        self.checks.unchecked.remove(path);
        let others: Vec<_> = (self.worktree_file_hunks(path).iter())
            .filter(|hunk| hunk.key != key)
            .map(|hunk| (path.to_owned(), hunk.key))
            .collect();
        self.checks.unchecked_hunks.extend(others);
    }

    /// A file none of whose hunks are in is out as a whole.
    fn settle_checks(&mut self, path: &str, cx: &mut Context<Self>) {
        let hunks = self.worktree_file_hunks(path).to_vec();
        self.checks.settle(path, &hunks);
        cx.emit(ChecksChanged);
        cx.notify();
    }

    pub fn focus_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.compose(true, window, cx)
    }

    /// Open the composer. With `picks`, files and lines picked, or the files a filter shows,
    /// are what it commits; without, as after a drop on a branch, the checks are already set.
    fn compose(&mut self, picks: bool, window: &mut Window, cx: &mut Context<Self>) {
        // A commit being edited isn't committed but finished, and a linked worktree with no
        // branch has nowhere to commit. With no changes, the commit is an empty one.
        if let Some((project, worktree)) = self.selected_checkout(cx)
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let refused = if !worktree.is_linked() && workspace.editing.is_some() {
                Some("Finish or cancel editing the commit first")
            } else if worktree.is_linked() && self.effective_target(&project, cx).is_none() {
                Some("Check out a branch of its own in the worktree to commit")
            } else {
                None
            };
            if let Some(refused) = refused {
                snack(&self.store, refused, cx);
                return;
            }
        }
        if !self.composing {
            // Its checks take over the line numbers, and lines or files picked in its
            // checkout's diff are what it commits, as dropped on a branch they would be.
            let picks = if !picks {
                None
            } else if self.line_picks.is_some() {
                self.line_picks_cut(cx)
            } else if !self.file_picks.is_empty() {
                self.owned_cut(self.picked_changes(), cx)
            } else if !self.filter_terms(cx).is_empty() {
                // A filter narrowing the list commits what it shows, as picking those would.
                self.owned_cut(self.shown_changes(), cx)
            } else {
                None
            };
            if let Some(Cut::Files {
                project,
                from: None,
                worktree,
                files,
            }) = picks
                && (Some(&project), &worktree)
                    == (self.checks_project.as_ref(), &self.checks_worktree)
            {
                self.check_only(Some(project), worktree, &files, cx);
                self.file_picks.clear();
            }
            // A file a merge left conflicted can't be committed until it's marked resolved,
            // and a repository cloned inside this one isn't a submodule yet, so they start out
            // of the commit.
            let files = (self.checks_project.as_ref())
                .and_then(|project| self.store.read(cx).workspace(project))
                .and_then(|ws| ws.uncommitted_in(&self.checks_worktree))
                .unwrap_or_default();
            let left_out = (files.iter())
                .filter(|file| file.conflicted || file.nested_repository())
                .map(|file| file.path.clone());
            self.checks.unchecked.extend(left_out.collect::<Vec<_>>());
            self.checks.known = Some(files.iter().map(|file| file.path.clone()).collect());
            self.composing_on_changes = !files.is_empty();
            self.composing_target = (self.checks_project.clone())
                .and_then(|project| self.effective_target(&project, cx));
        }
        let message = self.message.focus_handle(cx);
        if !self.composing {
            self.composer_back = window.focused(cx).filter(|focused| *focused != message);
        }
        self.composing = true;
        self.line_picks = None;
        // The composer sits under the file list, so a hidden list comes back with it.
        self.files_hidden = false;
        window.focus(&message, cx);
        cx.notify();
    }

    /// Close the composer, giving the focus back to where writing the message started:
    /// the navigator, say, so its keys go on working after a commit.
    pub(super) fn leave_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_composer(cx);
        let back = self
            .composer_back
            .take()
            .unwrap_or_else(|| self.focus.clone());
        window.focus(&back, cx);
    }

    /// Get `files` ready to commit to `branch`, after they were dropped on it.
    pub fn stage(
        &mut self,
        branch: FullName,
        files: &[Selected],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_target = Some(branch);
        // The checks are the main checkout's from here: switched now, as the selection
        // event that would do it comes after.
        let project = self.selected_project(cx);
        if (&self.checks_project, &self.checks_worktree) != (&project, &Checkout::Main) {
            self.switch_checkout(project.clone(), Checkout::Main, cx);
        }
        self.check_only(project, Checkout::Main, files, cx);
        // What was dropped is what goes in, not what's picked.
        self.file_picks.clear();
        self.line_picks = None;
        self.compose(false, window, cx);
    }

    /// Check just `files` of `project`'s `checkout`, down to the hunks and lines they name.
    fn check_only(
        &mut self,
        project: Option<ProjectId>,
        checkout: Checkout,
        files: &[Selected],
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = project
            && let Some(workspace) = self.store.read(cx).workspace(&project)
            && let Some(uncommitted) = workspace.uncommitted_in(&checkout)
        {
            let keep: HashMap<&str, Option<&[HunkKey]>> = files
                .iter()
                .map(|f| (f.file.path.as_str(), f.hunks.as_deref()))
                .collect();
            self.checks.unchecked = uncommitted
                .iter()
                .filter(|f| !keep.contains_key(f.path.as_str()))
                .map(|f| f.path.clone())
                .collect();
            // Of a file staged in part, just its picked hunks go in, and of a hunk it has
            // picked lines of, just those lines; the rest stay out. Every other hunk and line
            // choice goes.
            let left_out = |path: &str, key: &HunkKey| {
                keep.get(path)
                    .is_some_and(|picked| picked.is_some_and(|picked| !picked.contains(key)))
            };
            // The worktree's diff as last loaded: a drop or paste comes from another page,
            // whose diff stays on screen while this one's loads again.
            let worktree = DiffKey {
                project,
                source: match checkout {
                    Checkout::Main => DiffSource::Worktree,
                    Checkout::Linked(name) => DiffSource::Linked(name),
                },
            };
            if let Some(files) = self.recent_files(&worktree) {
                let mut out = HashSet::new();
                let mut lines = HashMap::new();
                for (path, picked) in keep.iter().filter_map(|(p, picked)| Some((*p, (*picked)?))) {
                    for hunk in file_hunks(&files, path) {
                        if picked.contains(&hunk.key) {
                            continue;
                        }
                        // Of a hunk only some of whose lines were taken, as lines dragged or
                        // cut from the diff, those lines.
                        let taken: BTreeSet<usize> = (hunk.lines.iter().enumerate())
                            .filter(|(_, line)| picked.iter().any(|key| key.takes(hunk, line)))
                            .map(|(ix, _)| ix)
                            .collect();
                        let entry = (path.to_owned(), hunk.key);
                        if taken.is_empty() {
                            out.insert(entry);
                        } else {
                            lines.insert(entry, (taken, fingerprint(hunk)));
                        }
                    }
                }
                self.checks.unchecked_hunks = out;
                self.checks.line_picks = lines;
            } else {
                // Hunks unknown until the diff loads: a file only some of which was dropped
                // is left out rather than taken whole, and the rest keep what they had.
                let checks = &mut self.checks;
                (checks.unchecked_hunks).retain(|(path, key)| left_out(path, key));
                (checks.line_picks).retain(|(path, key), _| left_out(path, key));
                let partly = (keep.iter()).filter(|(_, picked)| picked.is_some());
                checks
                    .unchecked
                    .extend(partly.map(|(path, _)| path.to_string()));
            }
        }
        cx.emit(ChecksChanged);
    }

    pub fn start_reword(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Commit(project, id)) = self.selection(cx) else {
            return;
        };
        let store = self.store.read(cx);
        // A rewording still running is where the next one starts.
        let Some((_, commit)) = store.find_commit(&project, id) else {
            return;
        };
        if let Some((editing, input, _)) = &self.rewording {
            // Already open, as after a click elsewhere: back to what's written there.
            if rewords(editing, &project, commit, store) {
                return window.focus(&input.focus_handle(cx), cx);
            }
            // Open on another commit with changes, which this one would throw away unsaved.
            let other = (store.workspace(&editing.0).into_iter())
                .flat_map(|ws| ws.commits())
                .find(|c| rewords(editing, &editing.0, c, store));
            if let Some(other) = other
                && store.message_of(&editing.0, other).1.trim() != input.read(cx).text().trim()
            {
                let why = format!(
                    "Save or cancel the new message for \"{}\" first",
                    other.title
                );
                return snack(&self.store, why, cx);
            }
        }
        let message = store.message_of(&project, commit).1.to_owned();
        let key = (project.clone(), commit.key.clone(), commit.authored.clone());
        let opened = message.trim().to_owned();
        let scroll = self.reword_scroll.clone();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Commit message", true, cx).scrolled_by(scroll);
            input.set_text(message, cx);
            input
        });
        // Back where editing started, like the navigator, when it's done.
        let back = window.focused(cx).unwrap_or(self.focus.clone());
        let editing = key.clone();
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, input, event, window, cx| match event {
                InputEvent::Submit => {
                    let message = input.read(cx).text().trim().to_owned();
                    // Unchanged, rewording would only give it and the commits above it new ids.
                    if message == opened {
                        this.rewording = None;
                        window.focus(&back, cx);
                        return cx.notify();
                    }
                    // The commit as it is now, should an agent have rewritten it meanwhile.
                    let id = match this.selection(cx) {
                        Some(Selection::Commit(_, now)) => now,
                        _ => id,
                    };
                    let project = project.clone();
                    let (detail, editing) = (cx.entity().downgrade(), editing.clone());
                    let submitted = message.clone();
                    this.store.update(cx, |store, cx| {
                        let reworded = store.key_of(&project, id).map(|key| (key, message.clone()));
                        let overlay = Overlay {
                            reworded,
                            ..Overlay::default()
                        };
                        store.run(
                            &project,
                            "Edit message",
                            overlay,
                            move |path, remap| backend::reword(path, remap.get(id)?, &message),
                            move |store, rewritten, cx| {
                                store.follow(&rewritten, cx);
                                store.snack_undoable("Message updated", cx);
                                // Open until it lands, so a reword that fails keeps what was
                                // written, and so does one written on meanwhile.
                                (detail.update(cx, |detail, cx| {
                                    if (detail.rewording.as_ref()).is_some_and(|(k, input, _)| {
                                        *k == editing && input.read(cx).text().trim() == submitted
                                    }) {
                                        detail.rewording = None;
                                        cx.notify();
                                    }
                                }))
                                .ok();
                            },
                            cx,
                        )
                    });
                    window.focus(&back, cx);
                }
                InputEvent::Cancel => {
                    this.rewording = None;
                    window.focus(&back, cx);
                    cx.notify();
                }
                InputEvent::Changed => {}
            },
        );
        window.focus(&input.focus_handle(cx), cx);
        self.rewording = Some((key, input, subscription));
        cx.notify();
    }

    /// The branch the composer commits to. A linked worktree's changes go on the branch it
    /// has checked out.
    pub fn effective_target(&self, project: &ProjectId, cx: &App) -> Option<FullName> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project)?;
        if let Some(name) = self.checks_worktree.linked() {
            let branch = workspace.find_worktree(name)?.branch.clone()?;
            // Not the default branch, which a worktree can have checked out.
            return (workspace.target_branch.as_ref() != Some(&branch)).then_some(branch);
        }
        // One of its own branches: a linked worktree's are committed to there.
        if let Some(target) = &self.commit_target
            && (workspace.branches_in(&Checkout::Main)).any(|branch| {
                branch.ref_name.as_ref() == Some(target) && takes_commits(store, project, branch)
            })
        {
            return Some(target.clone());
        }
        (workspace.lanes_in(&Checkout::Main))
            .filter_map(|stack| stack.branches.first())
            .find(|branch| takes_commits(store, project, branch))
            .and_then(|branch| branch.ref_name.clone())
    }

    pub(super) fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, worktree)) = self.selected_checkout(cx) else {
            return;
        };
        if self.writing.is_some() {
            snack(
                &self.store,
                "The message is still being written: the sparkle keeps what's written so far",
                cx,
            );
            return;
        }
        let message = self.message.read(cx).text().trim().to_owned();
        if self.choices_loading(&project, cx) {
            return;
        }
        let files = self.changes_not_pending(&project, cx);
        // A checkout with no changes at all commits an empty commit.
        let nothing_changed = (self.store.read(cx).workspace(&project))
            .and_then(|ws| ws.uncommitted_in(&worktree))
            .is_none_or(<[_]>::is_empty);
        // Opened on changes that all went meanwhile, as an agent committed them, an empty
        // commit isn't what was being written.
        if nothing_changed && self.composing_on_changes {
            snack(&self.store, COMMITTED_MEANWHILE, cx);
            return;
        }
        if files.is_empty() && !nothing_changed {
            // Changes checked but none left are what an operation still running is taking,
            // most likely the last commit.
            let why = match self.selected_changes(cx).is_empty() {
                true => "Nothing checked to commit",
                false => "Wait for the running operation to finish",
            };
            snack(&self.store, why, cx);
            return;
        }
        let target = self.effective_target(&project, cx);
        if self.target_went(&project, &target, cx) {
            return;
        }
        // A linked worktree's commit goes on its own branch, so the next one in the main
        // checkout still goes where it would have.
        let linked = worktree.is_linked();
        let picked = (!linked).then(|| self.commit_target.clone());
        let run_hooks = self.store.read(cx).runs_hooks(&project);
        let detail = cx.entity().downgrade();
        let window = window.window_handle();
        let (finished, committed, from) = (project.clone(), message.clone(), worktree.clone());
        // The files leave the list and the commit shows on its branch right away.
        let overlay = Overlay {
            new_commit: target.clone().map(|branch| {
                let title = crate::backend::subject_and_body(&message).0;
                (NewAt::Top(branch), title.into())
            }),
            files_in: worktree.clone(),
            ..Overlay::files(&files)
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Commit",
                overlay,
                move |path, _| {
                    let (branch, created) = match target {
                        Some(branch) => (branch, false),
                        None if linked => {
                            anyhow::bail!("The worktree has no branch of its own checked out")
                        }
                        None => (backend::create_branch(path, None)?, true),
                    };
                    // Undone apart from the commit, so a second ⌘Z takes the branch away.
                    let made = created
                        .then(|| backend::history::oplog_head(path))
                        .flatten();
                    let committed = backend::commit(
                        path,
                        &Destination::Branch(branch.clone()),
                        &files,
                        worktree.linked(),
                        &message,
                        run_hooks,
                    );
                    // A commit a hook refused leaves no empty branch made for it behind.
                    if committed.is_err() && created {
                        backend::remove_branch(path, &branch, None).ok();
                    }
                    Ok((branch, made, committed?))
                },
                move |store, (branch, made, (_id, left_out, hook_failed)), cx| {
                    if let Some(made) = made {
                        store.push_earlier_step(made);
                    }
                    // The message is done with, even kept for a checkout left meanwhile.
                    let key = (finished.clone(), from.clone());
                    if (store.drafts.get(&key)).is_some_and(|d| d.trim() == committed) {
                        store.drafts.remove(&key);
                    }
                    // The box may hold the next commit's message by now, or another
                    // checkout's.
                    let target = branch.clone();
                    after_landing(&detail, window, &finished, cx, |detail, window, cx| {
                        // The next commit goes where this one did, not to whichever stack is
                        // first now, as one an agent just made, unless another was picked
                        // meanwhile.
                        if picked.as_ref() == Some(&detail.commit_target) {
                            detail.commit_target = Some(target);
                        }
                        if detail.message.read(cx).text().trim() == committed {
                            (detail.message).update(cx, |input, cx| input.set_text("", cx));
                            detail.finish_composing(&from, window, cx);
                        }
                    });
                    forget_checks_left(&detail, &finished, &from, cx);
                    store.report_left_out(
                        &format!("Committed to {}", short_ref(&branch)),
                        &left_out,
                        cx,
                    );
                    if let Some(output) = hook_failed {
                        store.error("Committed, but the post-commit hook failed", output, cx);
                    }
                },
                cx,
            )
        });
    }

    /// Close the composer once what it committed from the checkout `worktree` landed, unless
    /// it's moved on to another. Focus stays in the pane if it was in the message box, which
    /// goes away.
    fn finish_composing(
        &mut self,
        worktree: &Checkout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.checks_worktree != *worktree {
            return;
        }
        match self.message.focus_handle(cx).is_focused(window) {
            true => self.leave_composer(window, cx),
            false => self.close_composer(cx),
        }
    }

    /// Whether a commit to the branch new commits go to is still pending, so the newest
    /// commit shown there isn't the newest.
    fn commit_pending(&self, project: &ProjectId, cx: &App) -> bool {
        (self.effective_target(project, cx)).is_some_and(|target| {
            let pending = self.store.read(cx).new_commits(
                project,
                |at| matches!(at, NewAt::Top(branch) if *branch == target),
            );
            !pending.is_empty()
        })
    }

    /// Whether Amend has a commit to add to, as the status bar says.
    pub fn can_amend(&self, cx: &App) -> bool {
        (self.selected_project(cx)).is_some_and(|project| self.amend_target(&project, cx).is_some())
    }

    /// Whether the branch the composer showed went, as an agent can unapply or rename it, so
    /// `target` is elsewhere than picked: says where, once, rather than go there unseen.
    fn target_went(
        &mut self,
        project: &ProjectId,
        target: &Option<FullName>,
        cx: &mut Context<Self>,
    ) -> bool {
        let shown = std::mem::replace(&mut self.composing_target, target.clone());
        let Some(gone) = shown.filter(|shown| {
            Some(shown) != target.as_ref()
                && self.commit_target != *target
                && (self.store.read(cx).workspace(project))
                    .is_none_or(|ws| ws.find_branch(Some(shown)).is_none())
        }) else {
            return false;
        };
        let to = target.as_ref().map_or("a new branch".into(), short_ref);
        let why = format!(
            "{} was renamed or left the workspace meanwhile. Try again for {to}",
            short_ref(&gone)
        );
        snack(&self.store, why, cx);
        true
    }

    /// The newest commit on the branch new commits go to, which Amend adds to.
    fn amend_target(&self, project: &ProjectId, cx: &App) -> Option<(CommitId, String)> {
        if self.commit_pending(project, cx) {
            return None;
        }
        let target = self.effective_target(project, cx)?;
        let workspace = self.store.read(cx).workspace(project)?;
        // The engine won't amend a conflicted commit.
        let top =
            (workspace.find_branch(Some(&target))?.commits.first()).filter(|c| !c.conflicted)?;
        Some((top.id, top.title.clone()))
    }

    /// Whether hunk or line choices can't be told apart yet, as `project`'s worktree diff
    /// is loading or older than its files; says so when they can't. Choices index into the
    /// diff as loaded, so committing them from another would take other lines.
    fn choices_loading(&self, project: &ProjectId, cx: &mut Context<Self>) -> bool {
        let checks = &self.checks;
        if checks.unchecked_hunks.is_empty() && checks.line_picks.is_empty() {
            return false;
        }
        let current = self.loading.is_none()
            && (self.checks_diff()).is_some_and(|diff| diff.key.project == *project)
            && (self.store.read(cx).workspace(project))
                .and_then(|ws| ws.uncommitted_in(&self.checks_worktree))
                .is_some_and(|files| self.shows_worktree(files));
        if !current {
            let why = "Still loading the diff, try again in a moment";
            snack(&self.store, why, cx);
        }
        !current
    }

    /// Add the checked changes to the newest commit of the target branch, keeping its
    /// message.
    pub(crate) fn amend_top(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, worktree)) = self.selected_checkout(cx) else {
            return;
        };
        if self.commit_pending(&project, cx) {
            snack(&self.store, "Wait for the commit to finish", cx);
            return;
        }
        let Some((into, _)) = self.amend_target(&project, cx) else {
            return;
        };
        if self.target_went(&project, &self.effective_target(&project, cx), cx) {
            return;
        }
        if self.choices_loading(&project, cx) {
            return;
        }
        let files = self.changes_not_pending(&project, cx);
        if files.is_empty() {
            snack(&self.store, "Nothing checked to amend", cx);
            return;
        }
        let detail = cx.entity().downgrade();
        let window = window.window_handle();
        let (finished, from) = (project.clone(), worktree.clone());
        // The composer stays open until it lands, so a failed amend keeps the choices.
        let landed = move |_: &mut Store, cx: &mut Context<Store>| {
            after_landing(&detail, window, &finished, cx, |detail, window, cx| {
                detail.finish_composing(&from, window, cx)
            });
            forget_checks_left(&detail, &finished, &from, cx);
        };
        (self.store).update(cx, |store, cx| {
            store.amend(&project, into, files.into(), worktree, landed, cx)
        });
    }

    /// The checked changes as a unified diff, for the AI to read.
    fn checked_diff_text(&self) -> String {
        let Some(diff) = self.checks_diff() else {
            return String::new();
        };
        let mut text = String::new();
        for file in diff.files.iter() {
            let path = &file.file.path;
            if self.checks.file_check(path) == Toggled::False {
                continue;
            }
            text.push_str(&format!("File: {path} ({})\n", file.file.kind.word()));
            let DiffBody::Hunks { hunks, .. } = &file.body else {
                text.push_str("Binary or too large to show.\n");
                continue;
            };
            for hunk in hunks
                .iter()
                .filter(|h| self.checks.hunk_checked(path, h.key))
            {
                text.push_str(&hunk.header);
                text.push('\n');
                for (ix, line) in hunk.lines.iter().enumerate() {
                    // A line left out reads as it stays: a removal kept, an addition absent.
                    let picked = self.checks.line_checked(path, hunk.key, ix);
                    text.push(match line.kind {
                        LineKind::Added if picked => '+',
                        LineKind::Added => continue,
                        LineKind::Removed if picked => '-',
                        LineKind::Removed | LineKind::Context => ' ',
                    });
                    text.push_str(&line.text);
                    text.push('\n');
                }
            }
            text.push('\n');
        }
        text
    }

    /// Write the commit message with AI, streaming it into the box. What was in the box
    /// guides it.
    pub(super) fn write_message(&mut self, cx: &mut Context<Self>) {
        let diff = self.checked_diff_text();
        if diff.trim().is_empty() || self.writing.is_some() {
            return;
        }
        let notes = self.message.read(cx).text().to_owned();
        let prompt = ai::commit_prompt(&notes, &diff);
        let kept = notes.clone();
        let input = self.message.clone();
        input.update(cx, |input, _| input.break_undo_run());
        let task = cx.spawn(async move |this, cx| {
            // Without a code fence around it, as a model can put one.
            let written = ai::write(ai::COMMIT_SYSTEM, prompt, |text| {
                let shown = ai::strip_fence(text).trim_start().to_owned();
                input.update(cx, |input, cx| input.set_text(shown, cx));
            })
            .await;
            this.update(cx, |this, cx| {
                this.writing = None;
                match written {
                    Ok(message) => {
                        let message = backend::wrapped_message(ai::strip_fence(&message).trim());
                        this.message
                            .update(cx, |input, cx| input.set_text(message, cx));
                    }
                    Err(err) => {
                        this.message
                            .update(cx, |input, cx| input.set_text(notes, cx));
                        this.store.update(cx, |store, cx| {
                            store.failed("Couldn't write the message", &err, cx)
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.writing = Some((task, kept));
        cx.notify();
    }

    pub(super) fn toggle_checked(&mut self, path: &str, cx: &mut Context<Self>) {
        let check = self.checks.file_check(path) == Toggled::False;
        self.set_checked(vec![path.to_owned()], check, cx);
    }

    /// Put the whole of each of `paths` in the next commit, or with `check` false, leave it out.
    fn set_checked(&mut self, paths: Vec<String>, check: bool, cx: &mut Context<Self>) {
        let checks = &mut self.checks;
        for path in paths {
            checks
                .unchecked_hunks
                .retain(|(hunk_path, _)| *hunk_path != path);
            checks
                .line_picks
                .retain(|(hunk_path, _), _| *hunk_path != path);
            if let Some(known) = &mut checks.known {
                known.insert(path.clone());
            }
            match check {
                true => checks.unchecked.remove(&path),
                false => checks.unchecked.insert(path),
            };
        }
        cx.emit(ChecksChanged);
        cx.notify();
    }

    /// The commit box under the uncommitted `files` of `project`'s checkout.
    pub(super) fn render_composer(
        &self,
        project: &ProjectId,
        files: &[FileChange],
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (checked, whole) = (files.iter())
            .map(|f| self.checks.file_check(&f.path))
            .fold((0, true), |(checked, whole), check| {
                (
                    checked + usize::from(check != Toggled::False),
                    whole && check == Toggled::True,
                )
            });
        let target = self.effective_target(project, cx);
        let message_focused = self.message.focus_handle(cx).is_focused(window);
        v_flex()
            .flex_none()
            .p(dp(12.))
            .gap(dp(8.))
            .border_t_1()
            .border_color(theme.border_3)
            .bg(theme.bg_1)
            .child(
                h_flex()
                    .gap(dp(6.))
                    .text_size(dp(12.))
                    .text_color(theme.text_2)
                    .child("Commit to")
                    .child(self.render_target_picker(target.as_ref(), theme, cx))
                    // Saying it's writing, as an answer can take a while to start. Above the
                    // box, as in it the words would run under it.
                    .when(self.writing.is_some(), |el| {
                        el.child(div().flex_1()).child(faint("Writing…", theme))
                    }),
            )
            .child(
                div()
                    .relative()
                    .child(
                        field(&self.message, message_focused, theme)
                            .pl(dp(10.))
                            // Clear of the AI button in the corner.
                            .pr(dp(30.))
                            .py(dp(8.))
                            .rounded(dp(6.))
                            .line_height(dp(19.))
                            .cursor_text()
                            .id("message-box")
                            .min_h(dp(84.))
                            .max_h(dp(260.))
                            .overflow_y_scroll()
                            .track_scroll(&self.message_scroll)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.focus_message(window, cx)),
                            ),
                    )
                    .child(
                        div().absolute().top(dp(4.)).right(dp(4.)).child(
                            icon_button(
                                match self.writing {
                                    Some(_) => "writing-message",
                                    None => "write-message",
                                },
                                Icon::Ai,
                            )
                            .small()
                            .disabled_because(
                                (self.writing.is_none() && checked == 0)
                                    .then_some("Check some changes to write about"),
                            )
                            .tooltip(if self.writing.is_some() {
                                "Writing… Click to keep what's written so far"
                            } else {
                                "Write the message with AI, guided by what's in the box"
                            })
                            // A slow model, as a local one can be, would otherwise hold the
                            // commit until it's done.
                            .on_click(cx.listener(|this, _, _, cx| match this.writing.take() {
                                // Before it wrote anything, what guided it comes back.
                                Some((_, notes)) if this.message.read(cx).text().is_empty() => {
                                    (this.message).update(cx, |input, cx| input.set_text(notes, cx))
                                }
                                Some(_) => cx.notify(),
                                None => this.write_message(cx),
                            })),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .gap(dp(6.))
                    .child(
                        // Every line of them, said shorter, so a narrow pane cuts at most the
                        // noun of a big count, rather than its second number. The narrowest
                        // list has room for nothing but "Al…", so says nothing.
                        faint(
                            match files.len() {
                                _ if self.files_width(window, cx) < 280. => String::new(),
                                0 => "No changes".to_owned(),
                                total if whole && total > 1 => {
                                    format!("All {}", plural(total, "file"))
                                }
                                total => format!("{checked} of {}", plural(total, "file")),
                            },
                            theme,
                        )
                        .flex_1()
                        .min_w_0()
                        .truncate(),
                    )
                    .child(
                        button("close-composer", "Cancel")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key("Close without committing", Some("esc".into()))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.leave_composer(window, cx)),
                            ),
                    )
                    // There without a commit to add to too, so Cancel stays put as the branch
                    // picked changes or a commit goes in.
                    .child({
                        let target = self.amend_target(project, cx);
                        button("amend", "Amend")
                            .small()
                            .disabled_because(match &target {
                                None => Some("No commit to add to"),
                                Some(_) => (checked == 0).then_some("Check some changes first"),
                            })
                            .tooltip_for(
                                match &target {
                                    Some((_, title)) if !title.is_empty() => {
                                        format!("Add the checked changes to \"{title}\"")
                                    }
                                    _ => "Add the checked changes to the newest commit".into(),
                                },
                                &super::AmendTop,
                            )
                            .on_click(cx.listener(|this, _, window, cx| this.amend_top(window, cx)))
                    })
                    .child(
                        button("commit", "Commit")
                            .small()
                            .style(ButtonStyle::Pop)
                            .disabled_because(match files.is_empty() {
                                true => self.composing_on_changes.then_some(COMMITTED_MEANWHILE),
                                false => (checked == 0).then_some("Check some changes first"),
                            })
                            .tooltip_with_key(
                                if files.is_empty() {
                                    "Make an empty commit"
                                } else {
                                    "Commit the checked changes"
                                },
                                Some("⌘↩".into()),
                            )
                            .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                    ),
            )
            .into_any_element()
    }

    /// The branch the commit goes to, which picks another in the main checkout. A linked
    /// worktree's commits go on the branch it has checked out.
    fn render_target_picker(
        &self,
        target: Option<&FullName>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let label = target.map_or("a new branch".into(), short_ref).into_owned();
        let key = shortcut(&PickCommitTarget, cx);
        h_flex()
            .id("commit-target")
            .min_w_0()
            .h(dp(22.))
            .px(dp(8.))
            .gap(dp(4.))
            .rounded(dp(4.))
            .bg(theme.fill_gray.opacity(0.08))
            .font_family(theme.mono_font.clone())
            .text_color(theme.blue)
            .child(small_icon(Icon::Branch).text_color(theme.blue))
            .child(div().min_w_0().truncate().child(label))
            .when(!self.checks_worktree.is_linked(), |el| {
                el.hover(|el| el.bg(theme.fill_gray.opacity(0.14)))
                    .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
                    .tooltip(Tooltip::keyed("Pick the branch to commit to", key))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.pick_commit_target(window, cx)),
                    )
            })
    }

    /// Pick the branch the next commit goes to, or make a new one for it, typing to filter.
    pub(crate) fn pick_commit_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return;
        };
        let current = self.effective_target(&project, cx);
        let detail = cx.entity().downgrade();
        let Some(workspace) = self.store.read(cx).workspace(&project) else {
            return;
        };
        let store = self.store.read(cx);
        let place = store.place(&Selection::Uncommitted(project.clone()));
        let mut choices: Vec<Choice> = (workspace.branches_in(&Checkout::Main))
            .filter(|branch| takes_commits(store, &project, branch))
            .filter_map(|branch| {
                let (detail, ref_name) = (detail.clone(), branch.ref_name.clone()?);
                let (picked, name) = (project.clone(), branch.name.clone());
                let choice = Choice::new(branch.name.clone(), move |_, _, cx| {
                    (detail.update(cx, |detail, cx| {
                        // Gone since the picker opened, as an agent renamed it, the commit
                        // would go to another branch.
                        let store = detail.store.read(cx);
                        if (store.workspace(&picked))
                            .is_none_or(|ws| ws.find_branch(Some(&ref_name)).is_none())
                        {
                            let gone =
                                format!("{name} was renamed or left the workspace meanwhile");
                            return snack(&detail.store, gone, cx);
                        }
                        detail.commit_target = Some(ref_name);
                        cx.notify();
                    }))
                    .ok();
                });
                Some(if current == branch.ref_name {
                    choice.detail("Current")
                } else {
                    choice
                })
            })
            .collect();
        choices.push(Choice::naming("New branch", move |this, name, _, cx| {
            let (targeted, detail) = (project.clone(), detail.clone());
            let name = (!name.is_empty()).then(|| name.to_owned());
            // The new branch becomes the target, rather than the page shown.
            let done = move |store: &mut Store, branch: FullName, cx: &mut Context<Store>| {
                let message = format!("Created {}", short_ref(&branch));
                (detail.update(cx, |detail, cx| {
                    detail.branch_created(&targeted, branch, cx)
                }))
                .ok();
                store.snack_undoable(message, cx)
            };
            this.store.update(cx, |store, cx| {
                let create = move |path: &_, _: &_| backend::create_branch(path, name.as_deref());
                store.run(
                    &project,
                    "Create branch",
                    Overlay::default(),
                    create,
                    done,
                    cx,
                )
            });
        }));
        self.app.update_later(window, cx, move |app, window, cx| {
            app.open_picker(
                Some(place),
                "Commit to a branch",
                "Pick",
                choices,
                window,
                cx,
            )
        });
    }
}

/// Whether the composer commits to `branch`. Not to the default branch, as new work goes on a
/// branch of its own, as `but commit` does; nor to one merged upstream, even before an update
/// takes it out, or being unapplied.
fn takes_commits(store: &Store, project: &ProjectId, branch: &Branch) -> bool {
    let Some(ref_name) = &branch.ref_name else {
        return false;
    };
    branch.push != PushState::Integrated
        && !store.hides_branch(project, ref_name)
        && (store.workspace(project)).is_none_or(|ws| ws.target_branch.as_ref() != Some(ref_name))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use gpui::Toggled;

    use super::{Checks, covers_same_lines, fingerprint};
    use crate::backend::{DiffBody, DiffLine, FileChange, FileDiff, Hunk, HunkKey, LineKind};

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

    #[test]
    fn line_picks_last_only_while_their_hunk_is_unchanged() {
        let line = |kind, text: &str| DiffLine::new(kind, Some(10), Some(10), text);
        let hunk = |key, added: &str| Hunk {
            key,
            header: String::new(),
            lines: vec![
                line(LineKind::Context, "fn main() {"),
                line(LineKind::Removed, "    old();"),
                line(LineKind::Added, added),
            ],
            scope: Vec::new(),
            lead: Vec::new(),
            tail: Vec::new(),
        };
        let file = |hunk| FileDiff {
            file: FileChange::bare("a.rs"),
            body: DiffBody::Hunks {
                hunks: vec![hunk],
                added: 1,
                removed: 1,
                widest: [0, 0],
                conflicts: Vec::new(),
            },
            fingerprint: 0,
            annotated: true,
            scope_lines: None,
            drawing: None,
            commit: None,
        };
        let mut checks = Checks::default();
        let picked = hunk(key(10, 2), "    new();");
        checks.line_picks.insert(
            ("a.rs".into(), picked.key),
            (BTreeSet::from([2]), fingerprint(&picked)),
        );

        checks.reconcile(&[file(picked.clone())]);
        assert_eq!(
            checks.hunk_check("a.rs", key(10, 2)),
            Toggled::Mixed,
            "a reload that leaves the hunk as it was keeps its picks"
        );

        checks.reconcile(&[file(hunk(key(10, 2), "    newer();"))]);
        assert_eq!(
            checks.hunk_check("a.rs", key(10, 2)),
            Toggled::False,
            "a hunk whose lines changed under its picks is left out, not taken whole"
        );
        assert_eq!(
            checks.file_check("a.rs"),
            Toggled::False,
            "with its only hunk out, the file is out, not counted as partly in"
        );

        checks.unchecked.clear();
        checks.unchecked_hunks.clear();
        checks.line_picks.insert(
            ("a.rs".into(), picked.key),
            (BTreeSet::from([2]), fingerprint(&picked)),
        );
        checks.reconcile(&[file(hunk(key(11, 2), "    new();"))]);
        assert_eq!(
            checks.hunk_check("a.rs", key(11, 2)),
            Toggled::False,
            "a hunk an edit above moved is left out under its new header too"
        );
    }

    #[test]
    fn a_file_that_turns_up_while_committing_starts_out() {
        let file = |path: &str| FileDiff {
            file: FileChange::bare(path),
            body: DiffBody::Binary,
            fingerprint: 0,
            annotated: true,
            scope_lines: None,
            drawing: None,
            commit: None,
        };
        let mut checks = Checks {
            known: Some(["picked.rs".to_owned()].into()),
            ..Checks::default()
        };
        assert_eq!(
            checks.file_check("agent.md"),
            Toggled::False,
            "a file the composer didn't list when it opened isn't in, even before the diff has it"
        );
        checks.reconcile(&[file("picked.rs"), file("agent.md")]);
        assert_eq!(checks.file_check("picked.rs"), Toggled::True);
        assert_eq!(
            checks.file_check("agent.md"),
            Toggled::False,
            "an agent's new file stays out of a commit it wasn't picked for"
        );
        checks.clear();
        checks.reconcile(&[file("picked.rs"), file("agent.md")]);
        assert_eq!(
            checks.file_check("agent.md"),
            Toggled::True,
            "with the composer closed, everything goes in again"
        );
    }

    #[test]
    fn settling_without_hunks_leaves_the_file_as_it_was() {
        let mut checks = Checks::default();
        checks.unchecked_hunks.insert(("a.rs".into(), key(10, 2)));
        checks.settle("a.rs", &[]);
        assert_eq!(
            checks.file_check("a.rs"),
            Toggled::Mixed,
            "no hunks to go by, as while another checkout's diff is shown, takes nothing out"
        );
    }
}
