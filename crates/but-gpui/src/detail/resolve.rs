//! Resolving a conflicted commit where it's shown: its conflicts sit in its diff, each
//! picked, written or suggested in place, and the picks apply together, as one step to undo.
//! Once the commit has none left, the next conflicted commit of its stack comes up.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use bstr::ByteSlice as _;
use gpui::{
    AnyElement, App, Context, Entity, Focusable, Subscription, Task, WeakEntity, Window, actions,
    div, prelude::*,
};

use super::{Detail, DiffSource};
use crate::app::AppView;
use crate::backend::conflicts::{self, Keep};
use crate::backend::{self, Branch, Checkout, Commit, CommitId, Conflict, FullName, Workspace, ai};
use crate::state::{OpKind, Overlay, ProjectId, Rereads, Selection, snack};
use crate::theme::Theme;
use crate::ui::conflict::{self, Actions, Kept, Name, Part, Pick};
use crate::ui::diff::{DiffRow, Layout, OnToggle};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, Tooltip, button, commit_title, dp, h_flex, plural, small_icon};

actions!(
    conflict,
    [
        /// Go to the oldest conflicted commit of the selected branch or commit's stack.
        ResolveConflicts,
        KeepNewBase,
        KeepCommit,
        KeepBoth,
        EditConflict,
        SuggestMerge,
        ApplyResolutions,
    ]
);

/// What the conflicts shown belong to.
#[derive(Clone, PartialEq)]
pub(super) enum Conflicted {
    Commit(CommitId),
    /// Uncommitted files a merge left conflict markers in, of a checkout.
    Files(Checkout),
}

/// What picks are kept for: a commit by its change ID, so they last through a rebase or a
/// reword, or a checkout's uncommitted files.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Of {
    Commit(String),
    Files(Checkout),
}

/// What's kept of a conflict until the picks are applied.
enum Keeping {
    Pick(Pick),
    Text { text: String, suggested: bool },
}

/// A conflict by its file's path and its id. The id alone is the same for the same conflict
/// in two files, as a version bump in two manifests is.
type Key = (String, String);

fn key(path: &str, c: &Conflict) -> Key {
    (path.to_owned(), c.id.clone())
}

fn is(key: &Key, path: &str, c: &Conflict) -> bool {
    key.0 == path && key.1 == c.id
}

/// The conflicts of the commit shown, as they're being resolved, by [`Key`].
#[derive(Default)]
pub(super) struct Resolving {
    keeping: HashMap<Key, Keeping>,
    /// The conflict being written by hand, its editor, and what listens to it.
    editing: Option<(Key, Entity<TextInput>, Subscription)>,
    /// Suggestions streaming in: the text so far, and what writes it.
    suggesting: HashMap<Key, (String, Task<()>)>,
    /// Conflicts showing the lines before either side changed them.
    ancestors: HashSet<Key>,
    /// The conflict the keyboard's picks go to.
    current: Option<Key>,
    /// What the picks are of, and those kept of the others looked at since, for coming back
    /// to them, as a resolution typed by hand is work.
    of: Option<(ProjectId, Of)>,
    others: HashMap<(ProjectId, Of), Resolving>,
}

impl Detail {
    /// What the diff shown has conflicted, and its project: a commit, or uncommitted files.
    pub(super) fn conflicted(&self) -> Option<(ProjectId, Conflicted)> {
        let diff = self.diff.as_ref()?;
        let conflicted = match &diff.key.source {
            DiffSource::Commit(id) => Conflicted::Commit(*id),
            _ => Conflicted::Files(diff.key.checkout()?),
        };
        (diff.files.iter())
            .any(|file| file.file.conflicted)
            .then(|| (diff.key.project.clone(), conflicted))
    }

    /// The conflicted commit shown, and its project.
    pub(super) fn conflicted_commit(&self) -> Option<(ProjectId, CommitId)> {
        match self.conflicted()? {
            (project, Conflicted::Commit(id)) => Some((project, id)),
            _ => None,
        }
    }

    /// Whether the diff shown has conflicts to pick.
    pub fn shows_conflicts(&self) -> bool {
        self.conflicts().next().is_some()
    }

    /// The key context of the detail: with conflicts to pick, and with picks to apply.
    pub(super) fn key_context(&self) -> &'static str {
        match (self.conflicts().next(), self.resolving.keeping.is_empty()) {
            (None, _) => "Diff",
            (Some(_), true) => "Diff Conflicts",
            (Some(_), false) => "Diff Conflicts Picked",
        }
    }

    /// Whether applying what's picked resolves every conflict of the shown file at `path`.
    pub(super) fn picks_settle(&self, path: &str) -> bool {
        (self.diff.iter().flat_map(|diff| diff.files.iter()))
            .find(|file| file.file.path == path)
            .is_some_and(|file| {
                let conflicts = file.conflicts();
                !conflicts.is_empty()
                    && (conflicts.iter())
                        .all(|c| self.resolving.keeping.contains_key(&key(path, c)))
            })
    }

    /// The shown diff's conflicts in file order, each with its file's place in the diff and
    /// its path.
    fn conflicts(&self) -> impl Iterator<Item = (usize, &str, &Conflict)> {
        (self.diff.iter())
            .flat_map(|diff| diff.files.iter().enumerate())
            .flat_map(|(ix, file)| {
                let path = file.file.path.as_str();
                file.conflicts().iter().map(move |c| (ix, path, c))
            })
    }

    /// How `conflict`, of the file at `path`, lays out.
    pub(super) fn conflict_view(&self, path: &str, conflict: &Conflict) -> conflict::View {
        let r = &self.resolving;
        let key = key(path, conflict);
        conflict::View {
            settled: r.keeping.contains_key(&key)
                || r.suggesting.contains_key(&key)
                || r.editing
                    .as_ref()
                    .is_some_and(|(editing, ..)| *editing == key),
            ancestor: r.ancestors.contains(&key),
        }
    }

    /// Whether the conflicts shown are a commit's checked out for editing: between it and
    /// the commit it now sits on, as a conflicted commit's are, though they're in its files.
    fn editing_commit(&self, cx: &App) -> bool {
        matches!(self.conflicted(), Some((project, Conflicted::Files(Checkout::Main)))
            if (self.store.read(cx).workspace(&project)).is_some_and(|ws| ws.editing.is_some()))
    }

    /// What `conflict`'s sides are called, alone and in a sentence. A commit sits on the
    /// target when it's its stack's oldest, else on the commit below, as a commit checked out
    /// for editing does, and then its lines can come from the target too. An update's markers put the uncommitted changes first, as Git's do,
    /// then what the update brought; any other merge's markers name the sides themselves.
    fn side_names(&self, conflict: &Conflict, cx: &App) -> [(String, String); 2] {
        let Some((project, conflicted)) = self.conflicted() else {
            return Default::default();
        };
        let store = self.store.read(cx);
        let workspace = store.workspace(&project);
        let named = |label: &str, phrase: &str| (label.to_owned(), phrase.to_owned());
        if self.editing_commit(cx) {
            return [
                named("Commit below", "the commit below"),
                named("This commit", "this commit"),
            ];
        }
        let target = (workspace.and_then(|ws| ws.target.as_ref()))
            .map(|target| (target.branch.clone(), target.branch.clone()));
        match conflicted {
            Conflicted::Commit(id) => {
                let oldest = workspace.is_some_and(|ws| oldest_first(ws, id).first() == Some(&id));
                let base = match target {
                    Some(target) if oldest => target,
                    Some((name, _)) => (
                        format!("{name} and below"),
                        format!("{name} and the commits below"),
                    ),
                    None => named("Commit below", "the commit below"),
                };
                [base, named("This commit", "this commit")]
            }
            Conflicted::Files(_) => match &conflict.labels {
                [ours, theirs] if ours.is_empty() || (ours == "ours" && theirs == "theirs") => [
                    named("Your changes", "your changes"),
                    target.unwrap_or_else(|| named("Upstream", "upstream")),
                ],
                [ours, theirs] => [named(ours, ours), named(theirs, theirs)],
            },
        }
    }

    /// Forget what was kept of conflicts no longer shown, as after applying them. Those of
    /// another commit or checkout wait for it. Returns whether the conflicts shown are another's
    /// than before, whose picks the rows built before don't show.
    pub(super) fn prune_resolving(&mut self, cx: &App) -> bool {
        let now = self.conflicted().map(|(project, conflicted)| {
            let of = match conflicted {
                Conflicted::Commit(id) => Of::Commit(
                    (self.store.read(cx).key_of(&project, id)).unwrap_or_else(|| id.0.to_string()),
                ),
                Conflicted::Files(worktree) => Of::Files(worktree),
            };
            (project, of)
        });
        let switched = now != self.resolving.of;
        if switched {
            let mut others = std::mem::take(&mut self.resolving.others);
            let left = std::mem::take(&mut self.resolving);
            if let Some(of) = left.of.clone().filter(|_| !left.keeping.is_empty()) {
                others.insert(of, left);
            }
            self.resolving = (now.as_ref())
                .and_then(|now| others.remove(now))
                .unwrap_or_default();
            self.resolving.of = now;
            self.resolving.others = others;
        }
        let shown: HashSet<Key> = (self.conflicts())
            .map(|(_, path, c)| key(path, c))
            .collect();
        let r = &mut self.resolving;
        r.keeping.retain(|id, _| shown.contains(id));
        r.suggesting.retain(|id, _| shown.contains(id));
        r.ancestors.retain(|id| shown.contains(id));
        r.editing.take_if(|(id, ..)| !shown.contains(id));
        r.current.take_if(|id| !shown.contains(id));
        switched && r.of.is_some()
    }

    /// The conflict the keyboard acts on: the one picked last, else the first undecided.
    fn current_conflict(&self) -> Option<Key> {
        let r = &self.resolving;
        r.current.clone().or_else(|| self.next_undecided(None))
    }

    /// The first conflict after `after` with nothing kept yet, wrapping around.
    fn next_undecided(&self, after: Option<&Key>) -> Option<Key> {
        let conflicts: Vec<(&str, &Conflict)> = self.conflicts().map(|(_, p, c)| (p, c)).collect();
        let start = after
            .and_then(|after| conflicts.iter().position(|(path, c)| is(after, path, c)))
            .map_or(0, |at| at + 1);
        let (before, from) = conflicts.split_at_checked(start)?;
        (from.iter().chain(before))
            .find(|(path, c)| !self.conflict_view(path, c).settled)
            .map(|(path, c)| key(path, c))
    }

    /// Conflict `key`, with its file's place in the diff.
    fn find_conflict(&self, key: &Key) -> Option<(usize, &Conflict)> {
        (self.conflicts())
            .find(|(_, path, c)| is(key, path, c))
            .map(|(ix, _, c)| (ix, c))
    }

    /// The row showing `part` of conflict `key`.
    fn conflict_row_ix(&self, key: &Key, part: Part) -> Option<usize> {
        let (file, found) = self.find_conflict(key)?;
        let row = DiffRow::Conflict {
            file,
            conflict: found.index,
            part,
        };
        self.diff
            .as_ref()?
            .rows
            .iter()
            .position(|shown| *shown == row)
    }

    /// Bring `key` up as the one the keyboard acts on, scrolled into view.
    fn focus_conflict(&mut self, key: Option<Key>, cx: &mut Context<Self>) {
        if let Some(row) = (key.as_ref()).and_then(|key| self.conflict_row_ix(key, Part::Bar)) {
            self.reveal_row(row);
        }
        self.resolving.current = key;
        cx.notify();
    }

    /// Keep `pick` for conflict `id`, then go on to the next one without a pick.
    pub(super) fn keep(&mut self, id: Key, pick: Pick, cx: &mut Context<Self>) {
        self.resolving.suggesting.remove(&id);
        self.resolving
            .editing
            .take_if(|(editing, ..)| *editing == id);
        self.resolving
            .keeping
            .insert(id.clone(), Keeping::Pick(pick));
        self.rebuild_rows(cx);
        let next = self.next_undecided(Some(&id)).or(Some(id));
        self.focus_conflict(next, cx);
    }

    /// Back to the conflict's two sides.
    fn change(&mut self, id: &Key, cx: &mut Context<Self>) {
        let r = &mut self.resolving;
        r.keeping.remove(id);
        r.suggesting.remove(id);
        r.editing.take_if(|(editing, ..)| editing == id);
        r.current = Some(id.clone());
        self.rebuild_rows(cx);
    }

    fn toggle_ancestor(&mut self, id: &Key, cx: &mut Context<Self>) {
        let ancestors = &mut self.resolving.ancestors;
        if !ancestors.remove(id) {
            ancestors.insert(id.clone());
        }
        self.rebuild_rows(cx);
    }

    /// Write conflict `id`'s lines by hand, from what's kept so far or the user's own: the
    /// commit's, or the uncommitted changes, the first side of a merge's markers.
    fn edit(&mut self, id: Key, window: &mut Window, cx: &mut Context<Self>) {
        // Already open, as after a click on the diff: back to what's written there.
        if let Some((editing, input, _)) = &self.resolving.editing
            && *editing == id
        {
            return window.focus(&input.focus_handle(cx), cx);
        }
        // A commit being edited starts from its own side, as a commit's conflict does.
        let files = matches!(self.conflicted(), Some((_, Conflicted::Files(_))))
            && !self.editing_commit(cx);
        let Some((_, found)) = self.find_conflict(&id) else {
            return;
        };
        let text = match self.resolving.keeping.get(&id) {
            Some(Keeping::Text { text, .. }) => text.clone(),
            Some(Keeping::Pick(pick)) => kept_text(found, *pick).to_str_lossy().into_owned(),
            None if files => found.new_base_text.to_str_lossy().into_owned(),
            None => found.commit_text.to_str_lossy().into_owned(),
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("The lines to keep", true, cx);
            // Without the last line's ending, which keeping it puts back: blank lines stay.
            input.set_text(text.strip_suffix('\n').unwrap_or(&text), cx);
            input
        });
        let listen = cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            InputEvent::Submit => this.finish_edit(window, cx),
            InputEvent::Cancel => this.cancel_edit(window, cx),
            InputEvent::Changed => {}
        });
        window.focus(&input.focus_handle(cx), cx);
        self.resolving.suggesting.remove(&id);
        self.resolving.current = Some(id.clone());
        self.resolving.editing = Some((id, input, listen));
        self.rebuild_rows(cx);
    }

    fn finish_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, input, _)) = self.resolving.editing.take() else {
            return;
        };
        let text = input.read(cx).text().to_owned();
        let text = match text.is_empty() {
            true => text,
            false => text + "\n",
        };
        self.resolving.keeping.insert(
            id.clone(),
            Keeping::Text {
                text,
                suggested: false,
            },
        );
        self.rebuild_rows(cx);
        window.focus(&self.focus, cx);
        let next = self.next_undecided(Some(&id)).or(Some(id));
        self.focus_conflict(next, cx);
    }

    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.resolving.editing.take().is_some() {
            self.rebuild_rows(cx);
            window.focus(&self.focus, cx);
        }
    }

    /// Ask the AI set up in Settings for a merge of conflict `id`, streaming it in place, to
    /// keep or change like any other pick.
    fn suggest(&mut self, id: Key, cx: &mut Context<Self>) {
        let Some((_, found)) = self.find_conflict(&id) else {
            return;
        };
        let found = found.clone();
        let Some((project, conflicted)) = self.conflicted() else {
            return;
        };
        let [base_name, own_name] = self.side_names(&found, cx);
        let own = match conflicted {
            Conflicted::Commit(commit) => (self.store.read(cx).find_commit(&project, commit))
                .map_or(own_name.1, |(_, c)| format!("the commit \"{}\"", c.title)),
            Conflicted::Files(_) => own_name.1,
        };
        let lines = |lines: &[backend::DiffLine]| {
            (lines.iter().map(|line| line.text.as_ref()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let prompt = ai::resolve_prompt(
            &id.0,
            [&base_name.1, &own],
            [
                &found.new_base_text.to_str_lossy(),
                &found.commit_text.to_str_lossy(),
            ],
            found.ancestor.as_deref().map(lines).as_deref(),
            [&lines(&found.before), &lines(&found.after)],
        );
        let writing = cx.spawn({
            let id = id.clone();
            async move |this, cx| {
                let shown = this.clone();
                let streamed = id.clone();
                let mut acx = cx.clone();
                let written = ai::write(ai::RESOLVE_SYSTEM, prompt, move |text| {
                    let text = ai::strip_fence(text).to_owned();
                    shown
                        .update(&mut acx, |this, cx| {
                            if let Some((so_far, _)) = this.resolving.suggesting.get_mut(&streamed)
                            {
                                *so_far = text;
                                this.remeasure_conflict(&streamed);
                                cx.notify();
                            }
                        })
                        .ok();
                })
                .await;
                this.update(cx, |this, cx| {
                    if this.resolving.suggesting.remove(&id).is_none() {
                        return;
                    }
                    match written {
                        Ok(text) => {
                            let mut text = ai::strip_fence(&text).to_owned();
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            this.resolving.keeping.insert(
                                id.clone(),
                                Keeping::Text {
                                    text,
                                    suggested: true,
                                },
                            );
                        }
                        Err(err) => this.store.update(cx, |store, cx| {
                            store.failed("Couldn't suggest a merge", &err, cx)
                        }),
                    }
                    this.rebuild_rows(cx);
                })
                .ok();
            }
        });
        self.resolving.keeping.remove(&id);
        self.resolving
            .editing
            .take_if(|(editing, ..)| *editing == id);
        self.resolving.current = Some(id.clone());
        self.resolving
            .suggesting
            .insert(id, (String::new(), writing));
        self.rebuild_rows(cx);
    }

    /// Measure the rows of conflict `id` again, as its suggestion grows.
    fn remeasure_conflict(&self, id: &Key) {
        if let Some(row) = self.conflict_row_ix(id, Part::Result) {
            self.list.remeasure_items(row..row + 1);
        }
    }

    /// Resolve every conflict something is kept for, as one step. The rest stay. With
    /// `finish`, the commit checked out for editing is finished once that worked. Whether
    /// there was anything to resolve.
    pub(super) fn apply_resolutions(&mut self, finish: bool, cx: &mut Context<Self>) -> bool {
        let Some((project, conflicted)) = self.conflicted() else {
            return false;
        };
        // Once the one under way is done, the conflicts left are numbered anew, which picks
        // applied again, or along with a finish, would mistake.
        if self.store.read(cx).resolving(&project) {
            snack(&self.store, "Wait for the resolve under way", cx);
            return true;
        }
        let Some(diff) = &self.diff else {
            return false;
        };
        let keep: Vec<(&backend::FileChange, &Conflict, Keep)> = (diff.files.iter())
            .flat_map(|file| file.conflicts().iter().map(move |c| (file, c)))
            .filter_map(|(file, c)| {
                let keep = match self.resolving.keeping.get(&key(&file.file.path, c))? {
                    Keeping::Pick(Pick::NewBase) => Keep::NewBase,
                    Keeping::Pick(Pick::Commit) => Keep::Commit,
                    Keeping::Pick(Pick::Both) => Keep::Text(kept_text(c, Pick::Both)),
                    Keeping::Text { text, .. } => Keep::Text(text.as_str().into()),
                };
                Some((&file.file, c, keep))
            })
            .collect();
        if keep.is_empty() {
            return false;
        }
        // Once applied, those conflicts are gone, and the ones left are numbered again: an
        // id applied could be another's by then, which mustn't take its pick.
        let applied: Vec<Key> = (keep.iter())
            .map(|(file, c, _)| key(&file.path, c))
            .collect();
        let detail = cx.entity().downgrade();
        let forget = move |cx: &mut App| {
            (detail.update(cx, |detail, _| {
                for id in &applied {
                    detail.resolving.keeping.remove(id);
                    detail.resolving.suggesting.remove(id);
                    detail.resolving.ancestors.remove(id);
                }
            }))
            .ok();
        };
        let commit = match conflicted {
            Conflicted::Commit(commit) => commit,
            Conflicted::Files(worktree) => {
                let decided = keep.len();
                let editing = (self.store.read(cx).workspace(&project))
                    .is_some_and(|ws| ws.editing.is_some());
                let keep: Vec<(bstr::BString, String, Keep)> = (keep.into_iter())
                    .map(|(file, c, keep)| (file.change.path.clone(), c.id.clone(), keep))
                    .collect();
                let project_done = project.clone();
                self.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Resolve",
                        Overlay {
                            kind: OpKind::Resolve,
                            rereads: Rereads::Files,
                            files_in: worktree.clone(),
                            ..Overlay::default()
                        },
                        {
                            let worktree = worktree.clone();
                            move |path, _| conflicts::resolve_markers(path, worktree.linked(), keep)
                        },
                        move |store, files, cx| {
                            forget(cx);
                            if finish {
                                return store.finish_editing(&project_done, cx);
                            }
                            let resolved = plural(decided, "conflict");
                            // A commit being edited is finished, not committed.
                            let message = match files {
                                n if n == 0 || editing => format!("Resolved {resolved}"),
                                n => format!(
                                    "Resolved {resolved}, {} ready to commit",
                                    plural(n, "file")
                                ),
                            };
                            // The oplog can't bring back a linked worktree, nor a commit
                            // checked out for editing.
                            store.snack_after(!worktree.is_linked() && !editing, message, cx)
                        },
                        cx,
                    )
                });
                return true;
            }
        };
        let keep: Vec<(String, usize, Keep)> = (keep.into_iter())
            .map(|(file, c, keep)| (file.path.clone(), c.index, keep))
            .collect();
        let after = (self.store.read(cx).workspace(&project))
            .map(|ws| stack_after(ws, commit))
            .unwrap_or_default();
        let shown = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Resolve",
                Overlay {
                    kind: OpKind::Resolve,
                    ..Overlay::default()
                },
                move |path, remap| {
                    // The picks are by each conflict's place in the commit as it was shown.
                    let now = remap.get(commit)?;
                    anyhow::ensure!(
                        now == commit,
                        "The commit changed since its conflicts were shown. Pick again."
                    );
                    conflicts::resolve(path, now, keep)
                },
                move |store, (rewritten, outcome), cx| {
                    forget(cx);
                    // The commits above it were rebuilt on it, conflicted now or not
                    // whatever they were; those below stay as they were.
                    let next = after.into_iter().find_map(|(id, title, was)| {
                        let now = rewritten.get(&id).copied();
                        let conflicted = now.map_or(was, |now| outcome.conflicted.contains(&now));
                        conflicted.then_some((now.unwrap_or(id), title))
                    });
                    let resolved = match outcome.emptied {
                        true => "Resolved, leaving the commit empty",
                        false => "Resolved",
                    };
                    let message = match (outcome.remaining, next) {
                        // On to the stack's next conflicted commit.
                        (0, Some((next, title))) => {
                            store.select(Some(Selection::Commit(shown, next)), cx);
                            format!("{resolved}, on to {title}")
                        }
                        (0, None) => {
                            store.follow(&rewritten, cx);
                            match outcome.emptied {
                                true => resolved.into(),
                                false => "Resolved, no conflicts left".into(),
                            }
                        }
                        (left, _) => {
                            store.follow(&rewritten, cx);
                            format!(
                                "Resolved {}, {left} left",
                                plural(outcome.resolved, "conflict")
                            )
                        }
                    };
                    store.snack_undoable(message, cx)
                },
                cx,
            )
        });
        true
    }

    /// One of the rows of `found`, a conflict of the file at `path`, acting on it through
    /// `entity`.
    #[expect(clippy::too_many_arguments)]
    pub(super) fn conflict_row(
        &self,
        path: &str,
        found: &Conflict,
        part: Part,
        layout: &Layout,
        last: bool,
        entity: WeakEntity<Detail>,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let r = &self.resolving;
        let id = key(path, found);
        let number = (self.conflicts().position(|(_, p, c)| is(&id, p, c))).map_or(1, |at| at + 1);
        let total = self.conflicts().count();
        let kept = match (
            r.editing.as_ref(),
            r.suggesting.get(&id),
            r.keeping.get(&id),
        ) {
            (Some((editing, input, _)), ..) if *editing == id => Some(Kept::Editing(input.clone())),
            (_, Some((so_far, _)), _) => Some(Kept::Text {
                text: so_far,
                suggested: true,
                writing: true,
            }),
            (_, _, Some(Keeping::Pick(pick))) => Some(Kept::Pick(*pick)),
            (_, _, Some(Keeping::Text { text, suggested })) => Some(Kept::Text {
                text,
                suggested: *suggested,
                writing: false,
            }),
            _ => None,
        };
        let names = self.side_names(found, cx);
        let act = |f: fn(&mut Detail, Key, &mut Window, &mut Context<Detail>)| -> OnToggle {
            let (entity, id) = (entity.clone(), id.clone());
            Rc::new(move |window: &mut Window, cx: &mut App| {
                entity
                    .update(cx, |this, cx| f(this, id.clone(), window, cx))
                    .ok();
            })
        };
        let pick = {
            let (entity, id) = (entity.clone(), id.clone());
            Rc::new(move |pick, _: &mut Window, cx: &mut App| {
                entity
                    .update(cx, |this, cx| this.keep(id.clone(), pick, cx))
                    .ok();
            })
        };
        let row = conflict::Row {
            conflict: found,
            number,
            total,
            sides: names
                .each_ref()
                .map(|(label, phrase)| Name { label, phrase }),
            icons: match self.conflicted() {
                Some((_, Conflicted::Files(_))) if !self.editing_commit(cx) => {
                    [Icon::FileDiff, Icon::Branch]
                }
                _ => [Icon::Branch, Icon::Commit],
            },
            kept,
            current: self.current_conflict().as_ref() == Some(&id),
            focused: (r.editing.as_ref()).is_some_and(|(editing, input, _)| {
                *editing == id && input.focus_handle(cx).is_focused(window)
            }),
            ancestor: r.ancestors.contains(&id),
            last,
            layout,
            actions: Actions {
                pick,
                edit: act(|this, id, window, cx| this.edit(id, window, cx)),
                suggest: act(|this, id, _, cx| this.suggest(id, cx)),
                change: act(|this, id, _, cx| this.change(&id, cx)),
                ancestor: act(|this, id, _, cx| this.toggle_ancestor(&id, cx)),
                done: act(|this, _, window, cx| this.finish_edit(window, cx)),
                cancel: act(|this, _, window, cx| this.cancel_edit(window, cx)),
            },
        };
        conflict::row(part, row, theme)
    }

    /// The conflicts shown as a condition of the page, with what helps: how many are left
    /// and between what, the older conflicted commit to start from, and for uncommitted files,
    /// Apply, which a commit has in its header.
    pub(super) fn conflict_banner(
        &self,
        entity: WeakEntity<Detail>,
        cx: &App,
    ) -> Option<AnyElement> {
        let theme = Theme::get(cx);
        let (project, conflicted) = self.conflicted()?;
        let store = self.store.read(cx);
        let workspace = store.workspace(&project)?;
        let no_undo = match conflicted {
            Conflicted::Files(Checkout::Linked(_)) => Some("A linked worktree can't undo it."),
            Conflicted::Files(Checkout::Main) if workspace.editing.is_some() => {
                Some("Finish then commits the result.")
            }
            _ => None,
        };
        let total = self.conflicts().count();
        let decided = (self.conflicts())
            .filter(|(_, path, c)| self.resolving.keeping.contains_key(&key(path, c)))
            .count();
        let files = || self.diff.iter().flat_map(|diff| diff.files.iter());
        // Conflicted files with nothing to pick between, which another way resolves.
        let other = match conflicted {
            Conflicted::Commit(_) => files()
                .filter(|file| matches!(file.body, backend::DiffBody::Unresolvable(_)))
                .count(),
            Conflicted::Files(_) => files()
                .filter(|file| file.file.conflicted && file.conflicts().is_empty())
                .count(),
        };
        let older = match conflicted {
            Conflicted::Commit(commit) => (oldest_first(workspace, commit).into_iter())
                .take_while(|id| *id != commit)
                .find(|id| {
                    workspace
                        .find_commit(*id)
                        .is_some_and(|(_, c)| c.conflicted)
                }),
            Conflicted::Files(_) => None,
        };
        let [base, own] = (self.conflicts().next())
            .map(|(_, _, c)| self.side_names(c, cx))
            .unwrap_or_default();
        let apply = (matches!(conflicted, Conflicted::Files(_)) && decided > 0).then(|| {
            let style = banner_action("apply-resolutions", "Apply", theme);
            apply_with(style, decided, entity)
        });
        let (title, detail, action): (String, String, Option<AnyElement>) = match older {
            Some(older) => {
                let store_entity = self.store.clone();
                (
                    "An older commit conflicts too".into(),
                    "Resolve it first: this one is rebuilt on top of it, which can settle some \
                     of these."
                        .into(),
                    Some(
                        banner_action("go-older", "Go to it", theme)
                            .on_click(move |_, _, cx| {
                                store_entity.update(cx, |store, cx| {
                                    store
                                        .select(Some(Selection::Commit(project.clone(), older)), cx)
                                })
                            })
                            .into_any_element(),
                    ),
                )
            }
            None if total == 0 => (
                format!("{} left conflicted", plural(other, "file")),
                match conflicted {
                    // A commit's files have no Mark resolved in their menu.
                    Conflicted::Commit(_) => match other {
                        1 => "It can only be resolved in a terminal, with `but resolve`.".into(),
                        _ => "They can only be resolved in a terminal, with `but resolve`.".into(),
                    },
                    Conflicted::Files(_) => {
                        "Right-click a file to mark it resolved as it is.".into()
                    }
                },
                None,
            ),
            None if decided == total => (
                format!("{} ready to apply", plural(total, "conflict")),
                // The oplog only records the main checkout, and not while a commit's edited.
                match (total, no_undo) {
                    (1, None) => "Apply resolves it in one step you can undo.".into(),
                    (_, None) => "Apply resolves them in one step you can undo.".into(),
                    (1, Some(why)) => format!("Apply resolves it. {why}"),
                    (_, Some(why)) => format!("Apply resolves them. {why}"),
                },
                apply,
            ),
            None => (
                format!("{} to resolve", plural(total - decided, "conflict")),
                format!(
                    "{} and {} both changed these lines. Pick what each keeps.",
                    own.0, base.1
                ),
                apply,
            ),
        };
        let detail = match (other, conflicted) {
            (0, _) => detail,
            (_, _) if total == 0 => detail,
            (n, Conflicted::Commit(_)) => format!(
                "{detail} {} can only be resolved in a terminal, with `but resolve`.",
                plural(n, "file")
            ),
            (1, Conflicted::Files(_)) => format!(
                "{detail} 1 file has no lines to pick: its menu marks it resolved as it is."
            ),
            (n, Conflicted::Files(_)) => format!(
                "{detail} {n} files have no lines to pick: each one's menu marks it resolved as \
                 it is."
            ),
        };
        Some(warn_banner(title, detail, action, theme).into_any_element())
    }
}

/// The text keeping `pick` gives conflict `c`: a side, or both, the new base's first.
fn kept_text(c: &Conflict, pick: Pick) -> bstr::BString {
    match pick {
        Pick::NewBase => c.new_base_text.clone(),
        Pick::Commit => c.commit_text.clone(),
        Pick::Both => {
            let mut both = c.new_base_text.clone();
            if !both.is_empty() && !both.ends_with(b"\n") {
                both.push(b'\n');
            }
            both.extend_from_slice(&c.commit_text);
            both
        }
    }
}

/// The commits of `commit`'s lane and the lanes it rests on, oldest first: the order conflicts
/// resolve in, as each commit is rebuilt on the one below it.
fn oldest_first(workspace: &Workspace, commit: CommitId) -> Vec<CommitId> {
    let Some((lane, _)) = workspace.lane_holding(commit) else {
        return Vec::new();
    };
    let mut order: Vec<CommitId> = (workspace.chain_from(lane, 0))
        .flat_map(|link| link.commits())
        .map(|commit| commit.id)
        .collect();
    order.reverse();
    order
}

/// The oldest conflicted commit from branch `ref_name` down the lanes it rests on, and the
/// branch holding it. None when that's a linked worktree's, as its conflicts resolve in it.
pub(super) fn oldest_conflict<'a>(
    workspace: &'a Workspace,
    ref_name: &FullName,
) -> Option<(&'a Branch, &'a Commit)> {
    let (lane, at) = workspace.lane_of(Some(ref_name))?;
    let history: Vec<&Commit> = (workspace.chain_from(lane, at))
        .flat_map(|link| link.commits())
        .collect();
    let oldest = history.into_iter().rev().find(|commit| commit.conflicted)?;
    let (lane, at) = workspace.lane_holding(oldest.id)?;
    let holder = lane
        .branches
        .get(at)
        .filter(|_| !lane.checkout.is_linked())?;
    Some((holder, oldest))
}

/// The commits of `commit`'s lane and those it rests on after it, oldest first and then
/// wrapping around to older ones, with their titles and whether they're conflicted.
fn stack_after(workspace: &Workspace, commit: CommitId) -> Vec<(CommitId, String, bool)> {
    let order = oldest_first(workspace, commit);
    let at = order
        .iter()
        .position(|id| *id == commit)
        .unwrap_or(order.len());
    let (before, from) = order.split_at(at);
    (from.iter().skip(1).chain(before))
        .filter_map(|id| {
            let (_, found) = workspace.find_commit(*id)?;
            Some((
                *id,
                commit_title(&found.title).to_string(),
                found.conflicted,
            ))
        })
        .collect()
}

/// A condition on the warn ground, as ui-react's `Banner`: its line, beside the one fix.
/// The sentence that explains it shows on hover, as the column it sits in is narrow.
pub(super) fn warn_banner(
    title: impl Into<gpui::SharedString>,
    detail: impl Into<gpui::SharedString>,
    action: Option<AnyElement>,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    let (title, detail) = (title.into(), detail.into());
    h_flex()
        .id(title.clone())
        .gap(dp(8.))
        .pl(dp(10.))
        .pr(dp(8.))
        .py(dp(6.))
        .rounded(dp(6.))
        .bg(theme.warn.opacity(0.1))
        .text_color(theme.warn)
        .tooltip(Tooltip::text(detail))
        .child(small_icon(Icon::Warning).text_color(theme.warn))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(dp(13.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .children(action)
}

/// A banner's fix: a small ghost button in the banner's colour.
pub(super) fn banner_action(
    id: &'static str,
    label: &'static str,
    theme: &Theme,
) -> crate::ui::Button {
    button(id, label)
        .small()
        .style(ButtonStyle::Ghost)
        .tint(theme.warn)
}

impl AppView {
    /// Register [`ResolveConflicts`] while the selected branch or commit's stack has a
    /// conflicted commit.
    pub fn resolve_actions(
        &self,
        el: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let conflicted = self.conflict_to_resolve(cx).is_some();
        el.when(conflicted, |el| {
            el.on_action(cx.listener(|this, _: &ResolveConflicts, window, cx| {
                if let Some((project, id)) = this.conflict_to_resolve(cx) {
                    this.resolve_commit(project, id, window, cx);
                }
            }))
        })
    }

    /// The selected commit when it's conflicted, else the oldest conflicted commit of the
    /// selected branch or commit's stack.
    fn conflict_to_resolve(&self, cx: &App) -> Option<(ProjectId, CommitId)> {
        let store = self.store.read(cx);
        let selection = (store.selection.as_ref()).filter(|s| !store.in_linked(s))?;
        let workspace = store.workspace(selection.project())?;
        let conflicted =
            |id: CommitId| workspace.find_commit(id).is_some_and(|(_, c)| c.conflicted);
        let commit = match selection {
            Selection::Commit(project, id) if conflicted(*id) => {
                return Some((project.clone(), *id));
            }
            Selection::Commit(_, id) => *id,
            Selection::Branch(_, ref_name) => {
                let (lane, _) = workspace.lane_of(ref_name.as_ref())?;
                lane.branches.first()?.commits.first()?.id
            }
            _ => return None,
        };
        let id = oldest_first(workspace, commit)
            .into_iter()
            .find(|id| conflicted(*id))?;
        Some((selection.project().clone(), id))
    }

    /// Show `project`'s conflicted commit `commit`, its first conflict up for the keyboard.
    pub fn resolve_commit(
        &mut self,
        project: ProjectId,
        commit: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.store.update(cx, |store, cx| {
            store.select(Some(Selection::Commit(project, commit)), cx)
        });
        self.detail
            .update(cx, |detail, cx| detail.focus(window, cx));
    }
}

/// The keys of a conflict, on the detail while its commit has conflicts.
pub(super) fn conflict_keys(
    el: gpui::Stateful<gpui::Div>,
    cx: &mut Context<Detail>,
) -> gpui::Stateful<gpui::Div> {
    let on_current = |pick: Pick| {
        move |this: &mut Detail, cx: &mut Context<Detail>| {
            if let Some(id) = this.current_conflict() {
                this.keep(id, pick, cx);
            }
        }
    };
    let new_base = on_current(Pick::NewBase);
    let commit = on_current(Pick::Commit);
    let both = on_current(Pick::Both);
    el.on_action(cx.listener(move |this, _: &KeepNewBase, _, cx| new_base(this, cx)))
        .on_action(cx.listener(move |this, _: &KeepCommit, _, cx| commit(this, cx)))
        .on_action(cx.listener(move |this, _: &KeepBoth, _, cx| both(this, cx)))
        .on_action(cx.listener(|this, _: &EditConflict, window, cx| {
            if let Some(id) = this.current_conflict() {
                this.edit(id, window, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &SuggestMerge, _, cx| {
            if let Some(id) = this.current_conflict() {
                this.suggest(id, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &ApplyResolutions, _, cx| {
            this.apply_resolutions(false, cx);
        }))
}

/// The Apply button for the commit's header, while something is kept to apply.
pub(super) fn apply_button(detail: &Detail, entity: WeakEntity<Detail>) -> Option<AnyElement> {
    detail.conflicted_commit()?;
    let kept = detail.resolving.keeping.len();
    (kept > 0).then(|| {
        let style = button("apply-resolutions", "Apply").style(ButtonStyle::Pop);
        apply_with(style, kept, entity)
    })
}

/// `button`, styled for where it sits, applying the `kept` conflicts' picks.
fn apply_with(button: crate::ui::Button, kept: usize, entity: WeakEntity<Detail>) -> AnyElement {
    button
        .tooltip_for(
            format!("Resolve {}", plural(kept, "conflict")),
            &ApplyResolutions,
        )
        .on_click(move |_, _, cx| {
            (entity.update(cx, |this, cx| this.apply_resolutions(false, cx))).ok();
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{oldest_conflict, oldest_first};
    use crate::backend::model::build::{branch, commit_name, id, workspace, worktree};

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn conflicts_resolve_oldest_first_down_the_lanes_a_lane_rests_on() {
        let ws = workspace(
            vec![vec![
                branch("top", &[1]),
                branch("middle", &[2, 3]),
                branch("bottom", &[4]),
            ]],
            vec![
                worktree("wt", Some(3), vec![branch("wt", &[5])]),
                worktree("wt-stacked", Some(5), vec![branch("wt-stacked", &[6])]),
            ],
        );
        let order = [("top", 1), ("wt-stacked", 6)].map(|(name, n)| {
            let order: Vec<String> = (oldest_first(&ws, id(n)).into_iter())
                .map(|id| commit_name(&ws, id))
                .collect();
            format!("oldest_first({name}): [{}]\n", order.join(", "))
        });
        let oldest = [
            ("wt-stacked", &[2, 3, 5, 6][..]),
            ("wt-stacked", &[2, 5, 6]),
            ("top", &[1, 3]),
        ]
        .map(|(name, conflicted)| {
            let mut ws = ws.clone();
            for commit in (ws.lanes.iter_mut())
                .flat_map(|lane| &mut lane.branches)
                .flat_map(|branch| &mut branch.commits)
            {
                commit.conflicted = conflicted.iter().any(|n| id(*n) == commit.id);
            }
            let oldest = oldest_conflict(
                &ws,
                &crate::backend::model::build::full(&format!("refs/heads/{name}")),
            )
            .map(|(holder, commit)| format!("{} in {}", commit_name(&ws, commit.id), holder.name));
            let conflicted: Vec<String> = (conflicted.iter())
                .map(|n| commit_name(&ws, id(*n)))
                .collect();
            format!(
                "oldest_conflict({name}) of [{}]: {oldest:?}\n",
                conflicted.join(", ")
            )
        });
        snapbox::assert_data_eq!(
            order.into_iter().chain(oldest).collect::<String>(),
            snapbox::str![[r#"
oldest_first(top): [bottom, middle~1, middle, top]
oldest_first(wt-stacked): [bottom, middle~1, wt, wt-stacked]
oldest_conflict(wt-stacked) of [middle, middle~1, wt, wt-stacked]: Some("middle~1 in middle")
oldest_conflict(wt-stacked) of [middle, wt, wt-stacked]: None
oldest_conflict(top) of [top, middle~1]: Some("middle~1 in middle")

"#]]
        );
    }
}
