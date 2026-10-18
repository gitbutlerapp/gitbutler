//! The commit composer: which changes go in, the message, and the branch they land on.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash as _, Hasher as _};

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
    ButtonStyle, Icon, Tooltip, button, faint, h_flex, icon_button, plural, shortcut, small_icon,
    v_flex,
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
    pub(super) fn close_composer(&mut self, cx: &mut Context<Self>) {
        self.composing = false;
        // A message still being written would land in whatever the box shows next.
        self.writing = None;
        self.checks.clear();
        cx.emit(ChecksChanged);
        cx.notify();
    }

    /// The checked uncommitted changes: checked files, narrowed to their checked hunks and
    /// lines.
    pub fn selected_changes(&self, cx: &App) -> Vec<Selected> {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return Vec::new();
        };
        let Some(workspace) = self.store.read(cx).workspace(&project) else {
            return Vec::new();
        };
        (workspace.uncommitted.iter())
            .filter_map(|file| self.selected_change(file))
            .collect()
    }

    /// What of uncommitted `file` is checked: the file narrowed to its checked hunks and
    /// lines, unless none of it is.
    pub(super) fn selected_change(&self, file: &FileChange) -> Option<Selected> {
        if self.checks.unchecked.contains(&file.path) {
            return None;
        }
        let hunks = self.worktree_file_hunks(&file.path);
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
        (self.selected_changes(cx).into_iter())
            .filter(|s| !store.hides_file(project, &s.file.path))
            .filter_map(|mut s| {
                if let Some(hunks) = &mut s.hunks {
                    hunks.retain(|hunk| !store.hides_hunk(project, &s.file.path, *hunk));
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
    fn worktree_file_hunks(&self, path: &str) -> &[Hunk] {
        self.worktree_diff()
            .map_or(&[], |diff| file_hunks(&diff.files, path))
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
        let out = (self.worktree_file_hunks(path).iter())
            .all(|hunk| (self.checks.unchecked_hunks).contains(&(path.to_owned(), hunk.key)));
        if out {
            let checks = &mut self.checks;
            checks
                .unchecked_hunks
                .retain(|(hunk_path, _)| hunk_path != path);
            checks.unchecked.insert(path.to_owned());
        }
        cx.emit(ChecksChanged);
        cx.notify();
    }

    pub fn focus_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The composer sits under the changes; with none, as when an agent committed them
        // all while it was open, there's nowhere to write. A commit being edited isn't
        // committed but finished.
        if let Some(Selection::Uncommitted(project)) = self.selection(cx)
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let refused = match workspace.editing {
                Some(_) => Some("Finish or cancel editing the commit first"),
                None => workspace
                    .uncommitted
                    .is_empty()
                    .then_some("No changes to commit"),
            };
            if let Some(refused) = refused {
                self.store.update(cx, |store, cx| store.snack(refused, cx));
                return;
            }
        }
        // A file a merge left conflicted can't be committed until it's marked resolved, so
        // it starts out of the commit.
        if !self.composing
            && let Some(workspace) = (self.checks_project.as_ref())
                .and_then(|project| self.store.read(cx).workspace(project))
        {
            let conflicted = (workspace.uncommitted.iter())
                .filter(|file| file.conflicted)
                .map(|file| file.path.clone());
            self.checks.unchecked.extend(conflicted.collect::<Vec<_>>());
        }
        let message = self.message.focus_handle(cx);
        if !self.composing {
            self.composer_back = window.focused(cx).filter(|focused| *focused != message);
        }
        self.composing = true;
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
        if let Some(project) = self.selected_project(cx)
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
                        let entry = (path.to_owned(), hunk.key);
                        match self.checks.line_picks.get(&entry) {
                            _ if picked.contains(&hunk.key) => {}
                            Some(picks)
                                if hunk.picked(&picks.0).iter().all(|k| picked.contains(k)) =>
                            {
                                lines.insert(entry, picks.clone());
                            }
                            _ => {
                                out.insert(entry);
                            }
                        }
                    }
                }
                self.checks.unchecked_hunks = out;
                self.checks.line_picks = lines;
            } else {
                // Hunks unknown until the diff loads: keep the choices the drop left out,
                // and committing waits for the diff to tell them apart.
                let checks = &mut self.checks;
                (checks.unchecked_hunks).retain(|(path, key)| left_out(path, key));
                (checks.line_picks).retain(|(path, key), _| left_out(path, key));
            }
        }
        cx.emit(ChecksChanged);
        self.focus_message(window, cx);
    }

    pub fn start_reword(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection @ Selection::Commit(..)) = self.selection(cx) else {
            return;
        };
        if (self.store).update(cx, |store, cx| store.changes_elsewhere(&selection, cx)) {
            return;
        }
        let Selection::Commit(project, id) = selection else {
            return;
        };
        let store = self.store.read(cx);
        // A rewording still running is where the next one starts.
        let Some(message) = (store.find_commit(&project, id))
            .map(|(_, commit)| store.message_of(&project, commit).1.to_owned())
        else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Commit message", true, cx);
            input.set_text(message, cx);
            input
        });
        // Back where editing started, like the navigator, when it's done.
        let back = window.focused(cx).unwrap_or(self.focus.clone());
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, input, event, window, cx| match event {
                InputEvent::Submit => {
                    let message = input.read(cx).text().trim().to_owned();
                    this.rewording = None;
                    // The commit as it is now, should an agent have rewritten it meanwhile.
                    let id = match this.selection(cx) {
                        Some(Selection::Commit(_, now)) => now,
                        _ => id,
                    };
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        let reworded = store.key_of(&project, id).map(|key| (key, message.clone()));
                        let overlay = Overlay {
                            reworded,
                            commits_only: true,
                            ..Overlay::default()
                        };
                        store.run(
                            &project,
                            "Edit message",
                            overlay,
                            move |path, remap| backend::reword(path, remap.get(id)?, &message),
                            |store, rewritten, cx| {
                                store.follow(&rewritten, cx);
                                store.snack_undoable("Message updated", cx)
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
        self.rewording = Some((input, subscription));
        cx.notify();
    }

    /// The branch the composer commits to.
    pub fn effective_target(&self, project: &ProjectId, cx: &App) -> Option<String> {
        let workspace = self.store.read(cx).workspace(project)?;
        // A branch merged upstream is done with, even before an update takes it out.
        let open = |branch: &Branch| branch.push != PushState::Integrated;
        // One of its own branches: a linked worktree's are committed to there.
        if let Some(target) = &self.commit_target
            && (workspace.branches()).any(|branch| branch.ref_name == *target && open(branch))
        {
            return Some(target.clone());
        }
        (workspace.stacks.iter())
            .filter_map(|stack| stack.branches.first())
            .find(|branch| !branch.ref_name.is_empty() && open(branch))
            .map(|branch| branch.ref_name.clone())
    }

    pub(super) fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
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
        if files.is_empty() {
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
        let picked = self.commit_target.clone();
        let run_hooks = self.store.read(cx).runs_hooks(&project);
        let detail = cx.entity().downgrade();
        let window = window.window_handle();
        let (finished, committed) = (project.clone(), message.clone());
        // The files leave the list and the commit shows on its branch right away.
        let overlay = Overlay {
            new_commit: target.clone().map(|branch| {
                let title = message.lines().next().unwrap_or_default().to_owned();
                (NewAt::Top(branch), title.into())
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
                    let committed = backend::commit(
                        path,
                        &Destination::Branch(branch.clone()),
                        &files,
                        None,
                        &message,
                        run_hooks,
                    )?;
                    Ok((branch, committed))
                },
                move |store, (branch, (_id, left_out, hook_failed)), cx| {
                    // The box is shared by projects, and may hold the next commit's message
                    // by now.
                    let target = branch.clone();
                    after_landing(&detail, window, &finished, cx, |detail, window, cx| {
                        // The next commit goes where this one did, not to whichever stack is
                        // first now, as one an agent just made, unless another was picked
                        // meanwhile.
                        if detail.commit_target == picked {
                            detail.commit_target = Some(target);
                        }
                        if detail.message.read(cx).text().trim() == committed {
                            (detail.message).update(cx, |input, cx| input.set_text("", cx));
                            detail.finish_composing(window, cx);
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

    /// Close the composer once what it committed landed. Focus stays in the pane if it was
    /// in the message box, which goes away.
    fn finish_composing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let branch = (workspace.branches()).find(|branch| branch.ref_name == target)?;
        let top = branch.commits.first()?;
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
            && (self.worktree_diff()).is_some_and(|diff| diff.key.project == *project)
            && (self.store.read(cx).workspace(project))
                .is_some_and(|ws| self.shows_worktree(&ws.uncommitted));
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
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
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
        let finished = project.clone();
        // The composer stays open until it lands, so a failed amend keeps the choices.
        let landed = move |_: &mut Store, cx: &mut Context<Store>| {
            after_landing(&detail, window, &finished, cx, |detail, window, cx| {
                detail.finish_composing(window, cx)
            })
        };
        (self.store).update(cx, |store, cx| {
            store.amend(&project, into, files.into(), None, landed, cx)
        });
    }

    /// The checked changes as a unified diff, for the AI to read.
    fn checked_diff_text(&self) -> String {
        let Some(diff) = self.worktree_diff() else {
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
        self.writing = Some(task);
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

    pub(super) fn render_composer(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            return div().into_any_element();
        };
        let (checked, whole) = (workspace.uncommitted.iter())
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
                        div()
                            .id("message-box")
                            .min_h(px(84.))
                            .max_h(px(260.))
                            .overflow_y_scroll()
                            .track_scroll(&self.message_scroll)
                            .pl(px(10.))
                            // Clear of the AI button in the corner.
                            .pr(px(30.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(if message_focused {
                                theme.pop
                            } else {
                                theme.border_2
                            })
                            .bg(theme.bg_1)
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .cursor_text()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.focus_message(window, cx)),
                            )
                            .child(self.message.clone()),
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
                            match workspace.uncommitted.len() {
                                _ if self.files_width(window, cx) < 280. => String::new(),
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
                            .disabled_because((checked == 0).then_some("Check some changes first"))
                            .tooltip_with_key("Commit the checked changes", Some("⌘↩".into()))
                            .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                    ),
            )
            .into_any_element()
    }

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
            .hover(|el| el.bg(theme.fill_gray.opacity(0.14)))
            .font_family(theme.mono_font.clone())
            .text_color(theme.blue)
            .child(small_icon(Icon::Branch).text_color(theme.blue))
            .child(div().min_w(px(0.)).truncate().child(label))
            .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
            .tooltip(move |_, cx| {
                Tooltip::with_key("Pick the branch to commit to".into(), key.clone(), cx)
            })
            .on_click(cx.listener(|this, _, window, cx| this.pick_commit_target(window, cx)))
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
        let mut choices: Vec<Choice> = (workspace.branches())
            .filter(|branch| !branch.ref_name.is_empty())
            .map(|branch| {
                let (detail, ref_name) = (detail.clone(), branch.ref_name.clone());
                let choice = Choice::new(branch.name.clone(), move |_, _, cx| {
                    (detail.update(cx, |detail, cx| {
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
                project,
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
            app.open_picker("Commit to a branch", "Pick", choices, window, cx)
        }))
        .ok();
    }
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
}
