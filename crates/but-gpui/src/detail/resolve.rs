//! Resolving a conflicted commit where it's shown: its conflicts sit in its diff, each
//! picked, written or suggested in place, and the picks apply together, as one step to undo.
//! Once the commit has none left, the next conflicted commit of its stack comes up.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, Entity, Focusable, KeyBinding, Subscription, Task, WeakEntity,
    Window, actions, div, prelude::*, px,
};

use super::{Detail, DiffSource};
use crate::app::AppView;
use crate::backend::conflicts::{self, Keep};
use crate::backend::{self, CommitId, Conflict, Workspace, ai};
use crate::state::{Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::conflict::{self, Actions, Kept, Name, Part, Pick};
use crate::ui::diff::{DiffRow, Layout, OnToggle};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, button, h_flex, plural, small_icon, v_flex};

actions!(
    resolve,
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

pub fn bind_keys(cx: &mut App) {
    let c = Some("Conflicts && !TextInput");
    cx.bind_keys([
        KeyBinding::new("1", KeepNewBase, c),
        KeyBinding::new("2", KeepCommit, c),
        KeyBinding::new("3", KeepBoth, c),
        KeyBinding::new("enter", EditConflict, c),
        KeyBinding::new("a", SuggestMerge, c),
        // Only with something to apply, so it still starts a commit otherwise.
        KeyBinding::new("cmd-enter", ApplyResolutions, Some("Picked && !TextInput")),
    ]);
}

/// What the conflicts shown belong to.
#[derive(Clone, PartialEq)]
pub(super) enum Conflicted {
    Commit(CommitId),
    /// Uncommitted files a merge left conflict markers in, of the main checkout or the
    /// linked worktree named.
    Files(Option<Arc<str>>),
}

/// What's kept of a conflict until the picks are applied.
enum Keeping {
    Pick(Pick),
    Text { text: String, suggested: bool },
}

/// The conflicts of the commit shown, as they're being resolved, by each conflict's id.
#[derive(Default)]
pub(super) struct Resolving {
    keeping: HashMap<String, Keeping>,
    /// The conflict being written by hand, its editor, and what listens to it.
    editing: Option<(String, Entity<TextInput>, Subscription)>,
    /// Suggestions streaming in: the text so far, and what writes it.
    suggesting: HashMap<String, (String, Task<()>)>,
    /// Conflicts showing the lines before either side changed them.
    ancestors: HashSet<String>,
    /// The conflict the keyboard's picks go to.
    current: Option<String>,
}

impl Detail {
    /// What the diff shown has conflicted, and its project: a commit, or uncommitted files.
    pub(super) fn conflicted(&self) -> Option<(ProjectId, Conflicted)> {
        let diff = self.diff.as_ref()?;
        let conflicted = match &diff.key.source {
            DiffSource::Commit(id) => Conflicted::Commit(*id),
            DiffSource::Worktree => Conflicted::Files(None),
            DiffSource::Linked(name) => Conflicted::Files(Some(name.clone())),
            _ => return None,
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

    /// The key context of the detail: with conflicts to pick, and with picks to apply.
    pub(super) fn key_context(&self) -> &'static str {
        match (self.conflicts().next(), self.resolving.keeping.is_empty()) {
            (None, _) => "Detail",
            (Some(_), true) => "Detail Conflicts",
            (Some(_), false) => "Detail Conflicts Picked",
        }
    }

    /// The shown diff's conflicts in file order, each with its file's place in the diff.
    fn conflicts(&self) -> impl Iterator<Item = (usize, &Conflict)> {
        (self.diff.iter())
            .flat_map(|diff| diff.files.iter().enumerate())
            .flat_map(|(ix, file)| file.conflicts().iter().map(move |c| (ix, c)))
    }

    pub(super) fn conflict_view(&self, conflict: &Conflict) -> conflict::View {
        let r = &self.resolving;
        let id = &conflict.id;
        conflict::View {
            settled: r.keeping.contains_key(id)
                || r.suggesting.contains_key(id)
                || r.editing
                    .as_ref()
                    .is_some_and(|(editing, ..)| editing == id),
            ancestor: r.ancestors.contains(id),
        }
    }

    /// What `conflict`'s sides are called, alone and in a sentence. A commit sits on the
    /// target when it's its stack's oldest, else on the commit below. An update's markers
    /// put the uncommitted changes first, as Git's do, then what the update brought; any
    /// other merge's markers name the sides themselves.
    fn side_names(&self, conflict: &Conflict, cx: &App) -> [(String, String); 2] {
        let Some((project, conflicted)) = self.conflicted() else {
            return Default::default();
        };
        let store = self.store.read(cx);
        let workspace = store.workspace(&project);
        let target = (workspace.and_then(|ws| ws.target.as_ref())).map(|target| {
            let name = target.name.rsplit('/').next().unwrap_or(&target.name);
            (name.to_owned(), name.to_owned())
        });
        let named = |label: &str, phrase: &str| (label.to_owned(), phrase.to_owned());
        match conflicted {
            Conflicted::Commit(id) => {
                let oldest = workspace.is_some_and(|ws| oldest_first(ws, id).first() == Some(&id));
                [
                    target
                        .filter(|_| oldest)
                        .unwrap_or_else(|| named("Commit below", "the commit below")),
                    named("This commit", "this commit"),
                ]
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

    /// Forget what was kept of conflicts no longer shown, as after applying them.
    pub(super) fn prune_resolving(&mut self) {
        let shown: HashSet<String> = self.conflicts().map(|(_, c)| c.id.clone()).collect();
        let r = &mut self.resolving;
        r.keeping.retain(|id, _| shown.contains(id));
        r.suggesting.retain(|id, _| shown.contains(id));
        r.ancestors.retain(|id| shown.contains(id));
        if r.editing
            .as_ref()
            .is_some_and(|(id, ..)| !shown.contains(id))
        {
            r.editing = None;
        }
        if r.current.as_ref().is_some_and(|id| !shown.contains(id)) {
            r.current = None;
        }
    }

    /// The conflict the keyboard acts on: the one picked last, else the first undecided.
    fn current_conflict(&self) -> Option<String> {
        let r = &self.resolving;
        r.current.clone().or_else(|| self.next_undecided(None))
    }

    /// The first conflict after `after` with nothing kept yet, wrapping around.
    fn next_undecided(&self, after: Option<&str>) -> Option<String> {
        let ids: Vec<&str> = self.conflicts().map(|(_, c)| c.id.as_str()).collect();
        let start = after
            .and_then(|after| ids.iter().position(|id| *id == after))
            .map_or(0, |at| at + 1);
        (ids[start..].iter().chain(&ids[..start]))
            .find(|id| (self.find_conflict(id)).is_none_or(|(_, c)| !self.conflict_view(c).settled))
            .map(|id| id.to_string())
    }

    /// Conflict `id`, with its file's place in the diff.
    fn find_conflict(&self, id: &str) -> Option<(usize, &Conflict)> {
        self.conflicts().find(|(_, c)| c.id == id)
    }

    /// The row showing `part` of conflict `id`.
    fn conflict_row_ix(&self, id: &str, part: Part) -> Option<usize> {
        let (file, found) = self.find_conflict(id)?;
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

    /// Bring `id` up as the one the keyboard acts on, scrolled into view.
    fn focus_conflict(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if let Some(row) = id
            .as_deref()
            .and_then(|id| self.conflict_row_ix(id, Part::Bar))
        {
            self.list.scroll_to_reveal_item(row);
        }
        self.resolving.current = id;
        cx.notify();
    }

    /// Keep `pick` for conflict `id`, then go on to the next one without a pick.
    pub(super) fn keep(&mut self, id: String, pick: Pick, cx: &mut Context<Self>) {
        self.resolving.suggesting.remove(&id);
        if self
            .resolving
            .editing
            .as_ref()
            .is_some_and(|(editing, ..)| *editing == id)
        {
            self.resolving.editing = None;
        }
        self.resolving
            .keeping
            .insert(id.clone(), Keeping::Pick(pick));
        self.rebuild_rows(cx);
        let next = self.next_undecided(Some(&id)).or(Some(id));
        self.focus_conflict(next, cx);
    }

    /// Back to the conflict's two sides.
    fn change(&mut self, id: &str, cx: &mut Context<Self>) {
        let r = &mut self.resolving;
        r.keeping.remove(id);
        r.suggesting.remove(id);
        if r.editing
            .as_ref()
            .is_some_and(|(editing, ..)| editing == id)
        {
            r.editing = None;
        }
        r.current = Some(id.to_owned());
        self.rebuild_rows(cx);
    }

    fn toggle_ancestor(&mut self, id: &str, cx: &mut Context<Self>) {
        let ancestors = &mut self.resolving.ancestors;
        if !ancestors.remove(id) {
            ancestors.insert(id.to_owned());
        }
        self.rebuild_rows(cx);
    }

    /// Write conflict `id`'s lines by hand, from what's kept so far or the user's own: the
    /// commit's, or the uncommitted changes, the first side of a merge's markers.
    fn edit(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let files = matches!(self.conflicted(), Some((_, Conflicted::Files(_))));
        let Some((_, found)) = self.find_conflict(&id) else {
            return;
        };
        let text = match self.resolving.keeping.get(&id) {
            Some(Keeping::Text { text, .. }) => text.clone(),
            Some(Keeping::Pick(pick)) => kept_text(found, *pick),
            None if files => found.new_base_text.clone(),
            None => found.commit_text.clone(),
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("The lines to keep", true, cx);
            input.set_text(text.trim_end_matches('\n'), cx);
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
    fn suggest(&mut self, id: String, cx: &mut Context<Self>) {
        let Some((file, found)) = self.find_conflict(&id) else {
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
        let path = (self.diff.as_ref())
            .map(|diff| diff.files[file].file.path.clone())
            .unwrap_or_default();
        let lines = |lines: &[backend::DiffLine]| {
            (lines.iter().map(|line| line.text.as_ref()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let prompt = ai::resolve_prompt(
            &path,
            [&base_name.1, &own],
            [&found.new_base_text, &found.commit_text],
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
                            store.error("Couldn't suggest a merge", format!("{err:#}"), cx)
                        }),
                    }
                    this.rebuild_rows(cx);
                })
                .ok();
            }
        });
        self.resolving.keeping.remove(&id);
        self.resolving.current = Some(id.clone());
        self.resolving
            .suggesting
            .insert(id, (String::new(), writing));
        self.rebuild_rows(cx);
    }

    /// Measure the rows of conflict `id` again, as its suggestion grows.
    fn remeasure_conflict(&self, id: &str) {
        if let Some(row) = self.conflict_row_ix(id, Part::Result) {
            self.list.remeasure_items(row..row + 1);
        }
    }

    /// Resolve every conflict something is kept for, as one step. The rest stay.
    pub(super) fn apply_resolutions(&mut self, cx: &mut Context<Self>) {
        let Some((project, conflicted)) = self.conflicted() else {
            return;
        };
        let Some(diff) = &self.diff else {
            return;
        };
        let keep: Vec<(String, &Conflict, Keep)> = (diff.files.iter())
            .flat_map(|file| file.conflicts().iter().map(move |c| (file, c)))
            .filter_map(|(file, c)| {
                let keep = match self.resolving.keeping.get(&c.id)? {
                    Keeping::Pick(Pick::NewBase) => Keep::NewBase,
                    Keeping::Pick(Pick::Commit) => Keep::Commit,
                    Keeping::Pick(Pick::Both) => Keep::Text(kept_text(c, Pick::Both)),
                    Keeping::Text { text, .. } => Keep::Text(text.clone()),
                };
                Some((file.file.path.clone(), c, keep))
            })
            .collect();
        if keep.is_empty() {
            return;
        }
        let commit = match conflicted {
            Conflicted::Commit(commit) => commit,
            Conflicted::Files(worktree) => {
                let decided = keep.len();
                let keep: Vec<(String, String, Keep)> = (keep.into_iter())
                    .map(|(path, c, keep)| (path, c.id.clone(), keep))
                    .collect();
                return self.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Resolve",
                        Overlay {
                            files_only: true,
                            ..Overlay::default()
                        },
                        {
                            let worktree = worktree.clone();
                            move |path, _| {
                                conflicts::resolve_markers(path, worktree.as_deref(), keep)
                            }
                        },
                        move |store, files, cx| {
                            let resolved = plural(decided, "conflict");
                            let message = match files {
                                0 => format!("Resolved {resolved}"),
                                n => format!(
                                    "Resolved {resolved}, {} ready to commit",
                                    plural(n, "file")
                                ),
                            };
                            // The oplog can't bring a linked worktree back.
                            match worktree {
                                None => store.snack_undoable(message, cx),
                                Some(_) => store.snack(message, cx),
                            }
                        },
                        cx,
                    )
                });
            }
        };
        let keep: Vec<(String, usize, Keep)> = (keep.into_iter())
            .map(|(path, c, keep)| (path, c.index, keep))
            .collect();
        let next =
            (self.store.read(cx).workspace(&project)).and_then(|ws| next_conflicted(ws, commit));
        let shown = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Resolve",
                Overlay::default(),
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
                    let next =
                        next.map(|(id, title)| (rewritten.get(&id).copied().unwrap_or(id), title));
                    let message = match (outcome.remaining, outcome.emptied, next) {
                        (0, true, _) => {
                            store.follow(&rewritten, cx);
                            "Resolved. Nothing of the commit's change is left".into()
                        }
                        // On to the stack's next conflicted commit, rebuilt on this one.
                        (0, false, Some((next, title))) => {
                            store.select(Some(Selection::Commit(shown, next)), cx);
                            format!("Resolved. Next: {title}")
                        }
                        (0, false, None) => {
                            store.follow(&rewritten, cx);
                            "Resolved. No conflicts left".into()
                        }
                        (left, ..) => {
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
    }

    /// One of the conflict's rows, acting on it through `entity`.
    pub(super) fn conflict_row(
        &self,
        found: &Conflict,
        part: Part,
        layout: &Layout,
        last: bool,
        entity: WeakEntity<Detail>,
        cx: &App,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let r = &self.resolving;
        let id = found.id.clone();
        let number = (self.conflicts().position(|(_, c)| c.id == id)).map_or(1, |at| at + 1);
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
        let act = |f: fn(&mut Detail, String, &mut Window, &mut Context<Detail>)| -> OnToggle {
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
                Some((_, Conflicted::Files(_))) => [Icon::FileDiff, Icon::Branch],
                _ => [Icon::Branch, Icon::Commit],
            },
            kept,
            current: self.current_conflict().as_deref() == Some(id.as_str()),
            ancestor: r.ancestors.contains(&id),
            last,
            layout,
            actions: Actions {
                pick,
                edit: act(|this, id, window, cx| this.edit(id, window, cx)),
                suggest: Some(act(|this, id, _, cx| this.suggest(id, cx))),
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
        let total = self.conflicts().count();
        let decided = (self.conflicts())
            .filter(|(_, c)| self.resolving.keeping.contains_key(&c.id))
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
            .map(|(_, c)| self.side_names(c, cx))
            .unwrap_or_default();
        let apply = (matches!(conflicted, Conflicted::Files(_)) && decided > 0).then(|| {
            banner_action("apply-resolutions", "Apply", theme)
                .tooltip_with_key(
                    format!("Resolve {}", plural(decided, "conflict")),
                    crate::ui::shortcut(&ApplyResolutions, cx),
                )
                .on_click(move |_, _, cx| {
                    entity
                        .update(cx, |this, cx| this.apply_resolutions(cx))
                        .ok();
                })
                .into_any_element()
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
                "Its menu marks a file resolved, taking it as it is.".into(),
                None,
            ),
            None if decided == total => (
                format!("{} ready to apply", plural(total, "conflict")),
                match total {
                    1 => "Apply resolves it in one step you can undo.".into(),
                    _ => "Apply resolves them in one step you can undo.".into(),
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
            (n, Conflicted::Files(_)) => format!(
                "{detail} {} has no lines to pick: its menu marks it resolved as it is.",
                plural(n, "file")
            ),
        };
        Some(warn_banner(title, detail, action, theme).into_any_element())
    }
}

/// The text keeping `pick` gives conflict `c`: a side, or both, the new base's first.
fn kept_text(c: &Conflict, pick: Pick) -> String {
    match pick {
        Pick::NewBase => c.new_base_text.clone(),
        Pick::Commit => c.commit_text.clone(),
        Pick::Both => {
            let mut both = c.new_base_text.clone();
            if !both.is_empty() && !both.ends_with('\n') {
                both.push('\n');
            }
            both + &c.commit_text
        }
    }
}

/// The commits of `commit`'s stack, oldest first: the order conflicts resolve in, as each
/// commit is rebuilt on the one below it.
fn oldest_first(workspace: &Workspace, commit: CommitId) -> Vec<CommitId> {
    let Some((branch, _)) = workspace.find_commit(commit) else {
        return Vec::new();
    };
    let Some((stack, _)) = workspace.stack_of(&branch.ref_name) else {
        return vec![commit];
    };
    (stack.branches.iter().rev())
        .flat_map(|branch| branch.commits.iter().rev().map(|c| c.id))
        .collect()
}

/// The next conflicted commit of `commit`'s stack after it, oldest first, with its title.
fn next_conflicted(workspace: &Workspace, commit: CommitId) -> Option<(CommitId, String)> {
    let order = oldest_first(workspace, commit);
    let at = order.iter().position(|id| *id == commit)?;
    order[at + 1..].iter().find_map(|id| {
        let (_, found) = workspace.find_commit(*id)?;
        found.conflicted.then(|| (*id, found.title.clone()))
    })
}

/// A condition on the warn ground, as ui-react's `Banner`: its line, a sentence under it, and
/// the one fix.
pub(super) fn warn_banner(
    title: impl Into<gpui::SharedString>,
    detail: impl Into<gpui::SharedString>,
    action: Option<AnyElement>,
    theme: &Theme,
) -> gpui::Div {
    h_flex()
        .items_start()
        .gap(px(8.))
        .pl(px(10.))
        .pr(px(8.))
        .py(px(8.))
        .rounded(px(6.))
        .bg(theme.warn.opacity(0.1))
        .text_color(theme.warn)
        .child(
            div()
                .pt(px(3.))
                .child(small_icon(Icon::Warning).text_color(theme.warn)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(4.))
                .py(px(2.))
                .text_size(px(13.))
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title.into()),
                )
                .child(div().text_size(px(12.)).child(detail.into())),
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
        let conflicted = self.oldest_conflict(cx).is_some();
        el.when(conflicted, |el| {
            el.on_action(cx.listener(|this, _: &ResolveConflicts, window, cx| {
                if let Some((project, id)) = this.oldest_conflict(cx) {
                    this.resolve_commit(project, id, window, cx);
                }
            }))
        })
    }

    /// The oldest conflicted commit of the selected branch or commit's stack.
    fn oldest_conflict(&self, cx: &App) -> Option<(ProjectId, CommitId)> {
        let store = self.store.read(cx);
        let selection = store.selection.as_ref()?;
        let workspace = store.workspace(selection.project())?;
        let commit = match selection {
            Selection::Commit(_, id) => *id,
            Selection::Branch(_, ref_name) => {
                let (stack, _) = workspace.stack_of(ref_name)?;
                stack.branches.first()?.commits.first()?.id
            }
            _ => return None,
        };
        let id = oldest_first(workspace, commit).into_iter().find(|id| {
            workspace
                .find_commit(*id)
                .is_some_and(|(_, c)| c.conflicted)
        })?;
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
        .on_action(cx.listener(|this, _: &ApplyResolutions, _, cx| this.apply_resolutions(cx)))
}

/// The Apply button for the commit's header, while something is kept to apply.
pub(super) fn apply_button(
    detail: &Detail,
    entity: WeakEntity<Detail>,
    cx: &App,
) -> Option<AnyElement> {
    detail.conflicted_commit()?;
    let kept = detail.resolving.keeping.len();
    (kept > 0).then(|| {
        button("apply-resolutions", "Apply")
            .style(ButtonStyle::Pop)
            .tooltip_with_key(
                format!("Resolve {}", plural(kept, "conflict")),
                crate::ui::shortcut(&ApplyResolutions, cx),
            )
            .on_click(move |_, _, cx| {
                entity
                    .update(cx, |this, cx| this.apply_resolutions(cx))
                    .ok();
            })
            .into_any_element()
    })
}
