//! The commit composer: which changes go in, the message, and the branch they land on.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash as _, Hasher as _};
use std::sync::Arc;

use gpui::{
    AnyElement, AnyWindowHandle, App, Context, Focusable as _, Toggled, WeakEntity, Window,
    actions, div, prelude::*, px,
};

use super::{ChecksChanged, Detail, DiffKey};
use crate::backend::{
    self, Branch, CommitId, Destination, DiffBody, DiffSource, FileChange, FileDiff, Hunk, HunkKey,
    LineKind, PushState, Selected, ai, short_ref,
};
use crate::palette::Choice;
use crate::state::{NewAt, Overlay, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, button, faint, field, h_flex, icon_button, plural, shortcut,
    small_icon, v_flex,
};

actions!(composer, [PickCommitTarget]);

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
}

impl Checks {
    pub(super) fn file_check(&self, path: &str) -> Toggled {
        if self.unchecked.contains(path) {
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
            Toggled::Mixed => self.line_picks[&(path.to_owned(), key)].0.contains(&line),
            check => check == Toggled::True,
        }
    }

    fn is_empty(&self) -> bool {
        self.unchecked.is_empty() && self.unchecked_hunks.is_empty() && self.line_picks.is_empty()
    }

    /// Put everything back in.
    fn clear(&mut self) {
        self.unchecked.clear();
        self.unchecked_hunks.clear();
        self.line_picks.clear();
    }

    /// Carry what's left out over to a reload of the worktree diff, `files`.
    pub(super) fn reconcile(&mut self, files: &[FileDiff]) {
        let present: HashSet<&str> = files.iter().map(|f| f.file.path.as_str()).collect();
        self.unchecked
            .retain(|path| present.contains(path.as_str()));
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
        worktree: Option<Arc<str>>,
        cx: &mut Context<Self>,
    ) {
        let left = (self.checks_project.take(), self.checks_worktree.take());
        let next = (self.other_checks)
            .remove(&(project.clone(), worktree.clone()))
            .unwrap_or_default();
        let checks = std::mem::replace(&mut self.checks, next);
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
            .and_then(|ws| ws.uncommitted_in(worktree.as_deref()))
            .unwrap_or_default();
        // Only the open composer checks files; the checks of one left, as on coming back from
        // another checkout, wait for it.
        if !self.composing {
            return files.iter().map(Selected::whole).collect();
        }
        // By path: finding each file in the diff in turn takes time squared in the files, as
        // after an `npm install` with nothing ignored.
        let hunks: HashMap<&str, &[Hunk]> = (self.checks_diff().into_iter())
            .flat_map(|diff| diff.files.iter())
            .map(|f| (f.file.path.as_str(), f.hunks()))
            .collect();
        (files.iter())
            .filter_map(|file| {
                let hunks = hunks.get(file.path.as_str()).copied().unwrap_or_default();
                self.selected_change(file, hunks)
            })
            .collect()
    }

    /// What of uncommitted `file`, whose loaded `hunks` these are, is checked: the file
    /// narrowed to its checked hunks and lines, unless none of it is.
    pub(super) fn selected_change(&self, file: &FileChange, hunks: &[Hunk]) -> Option<Selected> {
        if self.checks.unchecked.contains(&file.path) {
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
                Toggled::Mixed => {
                    hunk.picked(&self.checks.line_picks[&(file.path.clone(), hunk.key)].0)
                }
                Toggled::False => Vec::new(),
            })
            .collect();
        if picked.is_empty() && !hunks.is_empty() {
            return None;
        }
        Some(Selected {
            file: file.clone(),
            hunks: partial.then_some(picked),
        })
    }

    /// The checked changes, less the files and hunks a pending operation already takes, as
    /// after a quick second click.
    fn changes_not_pending(&self, project: &ProjectId, cx: &App) -> Vec<Selected> {
        let store = self.store.read(cx);
        let worktree = self.checks_worktree.as_deref();
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
        self.checks_diff()
            .map_or(&[], |diff| file_hunks(&diff.files, path))
    }

    /// The loaded diff when it's of the checkout the checks are about: another's stays on
    /// screen until this one's loads, and shares its paths.
    fn checks_diff(&self) -> Option<&super::LoadedDiff> {
        let worktree = self.checks_worktree.as_deref();
        self.worktree_diff().filter(|diff| {
            self.checks_project.as_ref() == Some(&diff.key.project)
                && diff.key.checkout() == Some(worktree)
        })
    }

    /// Whether the open composer's checks own the diff shown: not while another checkout's
    /// is still on screen.
    pub(super) fn composes(&self) -> bool {
        self.composing && self.checks_diff().is_some()
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
        // A commit being edited isn't committed but finished, and a linked worktree with no
        // branch has nowhere to commit. With no changes, the commit is an empty one.
        if let Some((project, worktree)) = self.selected_checkout(cx)
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let refused = if worktree.is_none() && workspace.editing.is_some() {
                Some("Finish or cancel editing the commit first")
            } else if worktree.is_some() && self.effective_target(&project, cx).is_none() {
                Some("Check out a branch of its own in the worktree to commit")
            } else {
                None
            };
            if let Some(refused) = refused {
                self.store.update(cx, |store, cx| store.snack(refused, cx));
                return;
            }
        }
        // A file a merge left conflicted can't be committed until it's marked resolved, so
        // it starts out of the commit.
        if !self.composing {
            let files = (self.checks_project.as_ref())
                .and_then(|project| self.store.read(cx).workspace(project))
                .and_then(|ws| ws.uncommitted_in(self.checks_worktree.as_deref()))
                .unwrap_or_default();
            let conflicted = (files.iter())
                .filter(|file| file.conflicted)
                .map(|file| file.path.clone());
            self.checks.unchecked.extend(conflicted.collect::<Vec<_>>());
            self.composing_on_changes = !files.is_empty();
        }
        let message = self.message.focus_handle(cx);
        if !self.composing {
            self.composer_back = window.focused(cx).filter(|focused| *focused != message);
        }
        self.composing = true;
        // Its checks take over the line numbers.
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
        branch: String,
        files: &[Selected],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_target = Some(branch);
        // The checks are the main checkout's from here: switched now, as the selection
        // event that would do it comes after.
        let project = self.selected_project(cx);
        if (&self.checks_project, &self.checks_worktree) != (&project, &None) {
            self.switch_checkout(project.clone(), None, cx);
        }
        if let Some(project) = project
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let keep: HashMap<&str, Option<&[HunkKey]>> = files
                .iter()
                .map(|f| (f.file.path.as_str(), f.hunks.as_deref()))
                .collect();
            self.checks.unchecked = workspace
                .uncommitted
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
                source: DiffSource::Worktree,
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
        self.focus_message(window, cx);
    }

    pub fn start_reword(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Commit(project, id)) = self.selection(cx) else {
            return;
        };
        let store = self.store.read(cx);
        // The oplog can't take back what rewrites a linked worktree's branch.
        let undoable = !store.in_worktree(&project, id);
        // A rewording still running is where the next one starts.
        let Some((key, message)) = (store.find_commit(&project, id)).map(|(_, commit)| {
            let message = store.message_of(&project, commit).1.to_owned();
            (commit.key.clone(), message)
        }) else {
            return;
        };
        // Already open, as after a click elsewhere: back to what's written there.
        if let Some((editing, input, _)) = &self.rewording
            && *editing == key
        {
            return window.focus(&input.focus_handle(cx), cx);
        }
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Commit message", true, cx);
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
                                store.snack_after(undoable, "Message updated", cx);
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
    pub fn effective_target(&self, project: &ProjectId, cx: &App) -> Option<String> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project)?;
        if let Some(name) = &self.checks_worktree {
            let branch = workspace.find_worktree(name)?.branch.as_ref()?;
            let branch = format!("refs/heads/{branch}");
            // Not the default branch, which a worktree can have checked out.
            return (workspace.target_branch.as_ref() != Some(&branch)).then_some(branch);
        }
        // One of its own branches: a linked worktree's are committed to there.
        if let Some(target) = &self.commit_target
            && (workspace.branches())
                .any(|branch| branch.ref_name == *target && takes_commits(store, project, branch))
        {
            return Some(target.clone());
        }
        (workspace.stacks.iter())
            .filter_map(|stack| stack.branches.first())
            .find(|branch| takes_commits(store, project, branch))
            .map(|branch| branch.ref_name.clone())
    }

    pub(super) fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, worktree)) = self.selected_checkout(cx) else {
            return;
        };
        if self.writing.is_some() {
            self.store.update(cx, |store, cx| {
                store.snack("The message is still being written", cx)
            });
            return;
        }
        let message = self.message.read(cx).text().trim().to_owned();
        if message.is_empty() {
            self.store.update(cx, |store, cx| {
                store.snack("Write a commit message first", cx)
            });
            self.focus_message(window, cx);
            return;
        }
        if self.choices_loading(&project, cx) {
            return;
        }
        let files = self.changes_not_pending(&project, cx);
        // A checkout with no changes at all commits an empty commit.
        let nothing_changed = (self.store.read(cx).workspace(&project))
            .and_then(|ws| ws.uncommitted_in(worktree.as_deref()))
            .is_none_or(<[_]>::is_empty);
        // Opened on changes that all went meanwhile, as an agent committed them, an empty
        // commit isn't what was being written.
        if nothing_changed && self.composing_on_changes {
            let why = "What was here to commit was committed meanwhile";
            self.store.update(cx, |store, cx| store.snack(why, cx));
            return;
        }
        if files.is_empty() && !nothing_changed {
            // Changes checked but none left are what an operation still running is taking,
            // most likely the last commit.
            let why = match self.selected_changes(cx).is_empty() {
                true => "Nothing checked to commit",
                false => "Wait for the running operation to finish",
            };
            self.store.update(cx, |store, cx| store.snack(why, cx));
            return;
        }
        let target = self.effective_target(&project, cx);
        // A linked worktree's commit goes on its own branch, so the next one in the main
        // checkout still goes where it would have.
        let linked = worktree.is_some();
        let picked = (!linked).then(|| self.commit_target.clone());
        let run_hooks = self.store.read(cx).runs_hooks(&project);
        let detail = cx.entity().downgrade();
        let window = window.window_handle();
        let (finished, committed, from) = (project.clone(), message.clone(), worktree.clone());
        // The files leave the list and the commit shows on its branch right away.
        let overlay = Overlay {
            new_commit: target.clone().map(|branch| {
                let title = message.lines().next().unwrap_or_default().to_owned();
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
                    let committed = backend::commit(
                        path,
                        &Destination::Branch(branch.clone()),
                        &files,
                        worktree.as_deref(),
                        &message,
                        run_hooks,
                    );
                    // A commit a hook refused leaves no empty branch made for it behind.
                    if committed.is_err() && created {
                        backend::remove_branch(path, &branch).ok();
                    }
                    Ok((branch, committed?))
                },
                move |store, (branch, (_id, left_out, hook_failed)), cx| {
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
                        // Moved on to another checkout, what was checked for this one goes too.
                        let here = detail.message.read(cx).text().trim() == committed;
                        if here {
                            (detail.message).update(cx, |input, cx| input.set_text("", cx));
                        }
                        if here || detail.checks_worktree != from {
                            detail.finish_composing(&from, window, cx);
                        }
                    });
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

    /// Close the composer once what it committed from the checkout `worktree` landed. Focus
    /// stays in the pane if it was in the message box, which goes away. Moved on to another
    /// checkout meanwhile, only the checks waiting for that one go.
    fn finish_composing(
        &mut self,
        worktree: &Option<Arc<str>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.checks_worktree != *worktree {
            self.other_checks
                .remove(&(self.checks_project.clone(), worktree.clone()));
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

    /// The newest commit on the branch new commits go to, which Amend adds to.
    fn amend_target(&self, project: &ProjectId, cx: &App) -> Option<(CommitId, String)> {
        if self.commit_pending(project, cx) {
            return None;
        }
        let target = self.effective_target(project, cx)?;
        let workspace = self.store.read(cx).workspace(project)?;
        // The engine won't amend a conflicted commit.
        let top = (workspace.find_branch(&target)?.commits.first()).filter(|c| !c.conflicted)?;
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
        let worktree = self.checks_worktree.as_deref();
        let current = self.loading.is_none()
            && (self.checks_diff()).is_some_and(|diff| diff.key.project == *project)
            && (self.store.read(cx).workspace(project))
                .and_then(|ws| ws.uncommitted_in(worktree))
                .is_some_and(|files| self.shows_worktree(files));
        if !current {
            self.store.update(cx, |store, cx| {
                store.snack("Still loading the diff, try again in a moment", cx)
            });
        }
        !current
    }

    /// Add the checked changes to the newest commit of the target branch, keeping its
    /// message.
    pub(super) fn amend_top(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, worktree)) = self.selected_checkout(cx) else {
            return;
        };
        if self.commit_pending(&project, cx) {
            self.store.update(cx, |store, cx| {
                store.snack("Wait for the commit to finish", cx)
            });
            return;
        }
        let Some((into, _)) = self.amend_target(&project, cx) else {
            return;
        };
        if self.choices_loading(&project, cx) {
            return;
        }
        let files = self.changes_not_pending(&project, cx);
        if files.is_empty() {
            self.store
                .update(cx, |store, cx| store.snack("Nothing checked to amend", cx));
            return;
        }
        let detail = cx.entity().downgrade();
        let window = window.window_handle();
        let (finished, from) = (project.clone(), worktree.clone());
        // The composer stays open until it lands, so a failed amend keeps the choices.
        let landed = move |_: &mut Store, cx: &mut Context<Store>| {
            after_landing(&detail, window, &finished, cx, |detail, window, cx| {
                detail.finish_composing(&from, window, cx)
            })
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
            let written = ai::write(ai::COMMIT_SYSTEM, prompt, |text| {
                let shown = text.trim_start().to_owned();
                input.update(cx, |input, cx| input.set_text(shown, cx));
            })
            .await;
            this.update(cx, |this, cx| {
                this.writing = None;
                match written {
                    Ok(message) => {
                        let message = backend::wrapped_message(message.trim());
                        this.message
                            .update(cx, |input, cx| input.set_text(message, cx));
                    }
                    Err(err) => {
                        this.message
                            .update(cx, |input, cx| input.set_text(notes, cx));
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't write the message", format!("{err:#}"), cx)
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
        let checks = &mut self.checks;
        let check = checks.file_check(path);
        checks
            .unchecked_hunks
            .retain(|(hunk_path, _)| hunk_path != path);
        checks
            .line_picks
            .retain(|(hunk_path, _), _| hunk_path != path);
        match check {
            Toggled::True | Toggled::Mixed => {
                checks.unchecked.insert(path.to_owned());
            }
            Toggled::False => {
                checks.unchecked.remove(path);
            }
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
            .on_action(cx.listener(|this, _: &PickCommitTarget, window, cx| {
                this.pick_commit_target(window, cx)
            }))
            .p(px(12.))
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.border_3)
            .bg(theme.bg_1)
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child("Commit to")
                    .child(self.render_target_picker(target.as_deref(), theme, cx)),
            )
            .child(
                div()
                    .relative()
                    .child(
                        field(&self.message, message_focused, theme)
                            .pl(px(10.))
                            // Clear of the AI button in the corner.
                            .pr(px(30.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .line_height(px(19.))
                            .cursor_text()
                            .id("message-box")
                            .min_h(px(84.))
                            .max_h(px(260.))
                            .overflow_y_scroll()
                            .track_scroll(&self.message_scroll)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.focus_message(window, cx)),
                            ),
                    )
                    .child(
                        div().absolute().top(px(4.)).right(px(4.)).child(
                            icon_button("write-message", Icon::Ai)
                                .small()
                                .disabled_because(if self.writing.is_some() {
                                    Some("Writing…")
                                } else if checked == 0 {
                                    Some("Check some changes to write about")
                                } else {
                                    None
                                })
                                .tooltip(if self.writing.is_some() {
                                    "Writing…"
                                } else {
                                    "Write the message with AI, guided by what's in the box"
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.write_message(cx))),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(6.))
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
                        .min_w(px(0.))
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
                    .when_some(self.amend_target(project, cx), |el, (_, title)| {
                        el.child(
                            button("amend", "Amend")
                                .small()
                                .style(ButtonStyle::Outline)
                                .disabled_because(
                                    (checked == 0).then_some("Check some changes first"),
                                )
                                .tooltip_with_key(
                                    format!("Add the checked changes to \"{title}\""),
                                    shortcut(&super::AmendTop, cx),
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.amend_top(window, cx)),
                                ),
                        )
                    })
                    .child(
                        button("commit", "Commit")
                            .small()
                            .style(ButtonStyle::Pop)
                            .disabled_because(
                                (checked == 0 && !files.is_empty())
                                    .then_some("Check some changes first"),
                            )
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
        target: Option<&str>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let label = target.map_or("a new branch", short_ref).to_owned();
        let key = shortcut(&PickCommitTarget, cx);
        h_flex()
            .id("commit-target")
            .min_w(px(0.))
            .h(px(22.))
            .px(px(8.))
            .gap(px(4.))
            .rounded(px(4.))
            .bg(theme.fill_gray.opacity(0.08))
            .font_family(theme.mono_font.clone())
            .text_color(theme.blue)
            .child(small_icon(Icon::Branch).text_color(theme.blue))
            .child(div().min_w(px(0.)).truncate().child(label))
            .when(self.checks_worktree.is_none(), |el| {
                el.hover(|el| el.bg(theme.fill_gray.opacity(0.14)))
                    .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
                    .tooltip(Tooltip::keyed("Pick the branch to commit to", key))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.pick_commit_target(window, cx)),
                    )
            })
    }

    /// Pick the branch the next commit goes to, or make a new one for it, typing to filter.
    fn pick_commit_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let mut choices: Vec<Choice> = (workspace.branches())
            .filter(|branch| takes_commits(store, &project, branch))
            .map(|branch| {
                let (detail, ref_name) = (detail.clone(), branch.ref_name.clone());
                let (picked, name) = (project.clone(), branch.name.clone());
                let choice = Choice::new(branch.name.clone(), move |_, _, cx| {
                    (detail.update(cx, |detail, cx| {
                        // Gone since the picker opened, as an agent renamed it, the commit
                        // would go to another branch.
                        let store = detail.store.read(cx);
                        if (store.workspace(&picked))
                            .is_none_or(|ws| ws.find_branch(&ref_name).is_none())
                        {
                            let gone = format!("{name} was renamed or deleted meanwhile");
                            return detail.store.update(cx, |store, cx| store.snack(gone, cx));
                        }
                        detail.commit_target = Some(ref_name);
                        cx.notify();
                    }))
                    .ok();
                });
                if current.as_ref() == Some(&branch.ref_name) {
                    choice.detail("Current")
                } else {
                    choice
                }
            })
            .collect();
        choices.push(Choice::new("New branch…", move |this, window, cx| {
            let (targeted, detail) = (project.clone(), detail.clone());
            // The new branch becomes the target, rather than the page shown.
            let handle = window.window_handle();
            this.prompt_branch(
                Selection::Project(project),
                "New branch",
                None,
                window,
                cx,
                |path, _, name| backend::create_branch(path, Some(name)),
                move |_, ref_name, cx| {
                    after_landing(&detail, handle, &targeted, cx, |detail, window, cx| {
                        detail.commit_target = Some(ref_name);
                        if detail.composing {
                            window.focus(&detail.message.focus_handle(cx), cx);
                        }
                        cx.notify();
                    })
                },
            );
        }));
        (self.app.update(cx, |app, cx| {
            app.open_picker(
                Some(place),
                "Commit to a branch",
                "Pick",
                choices,
                window,
                cx,
            )
        }))
        .ok();
    }
}

/// Whether the composer commits to `branch`. Not to the default branch, as new work goes on a
/// branch of its own, as `but commit` does; nor to one merged upstream, even before an update
/// takes it out, or being unapplied.
fn takes_commits(store: &Store, project: &ProjectId, branch: &Branch) -> bool {
    !branch.ref_name.is_empty()
        && branch.push != PushState::Integrated
        && !store.hides_branch(project, &branch.ref_name)
        && (store.workspace(project))
            .is_none_or(|ws| ws.target_branch.as_ref() != Some(&branch.ref_name))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use gpui::Toggled;

    use super::{Checks, covers_same_lines, fingerprint};
    use crate::backend::{
        ChangeKind, DiffBody, DiffLine, FileChange, FileDiff, Hunk, HunkKey, LineKind,
    };

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
        let line = |kind, text: &str| DiffLine {
            kind,
            old: Some(10),
            new: Some(10),
            text: text.into(),
            spans: Vec::new(),
            emph: Vec::new(),
        };
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
        let state = but_core::ChangeState {
            id: gix::ObjectId::null(gix::hash::Kind::Sha1),
            kind: gix::object::tree::EntryKind::Blob,
        };
        let file = |hunk| FileDiff {
            file: FileChange {
                path: "a.rs".into(),
                previous_path: None,
                kind: ChangeKind::Modified,
                modified: None,
                conflicted: false,
                lines: None,
                change: but_core::TreeChange {
                    path: "a.rs".into(),
                    status: but_core::TreeStatus::Modification {
                        previous_state: state,
                        state,
                        flags: None,
                    },
                },
            },
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
