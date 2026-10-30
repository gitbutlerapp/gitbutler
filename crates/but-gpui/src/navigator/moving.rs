//! Moving commits with the keys: the places they can land as the rows list them, stepping
//! through those, the copies of the commits that show where they'd land, and landing them.

use gpui::{AnyElement, App, Context, Div, Window, div, prelude::*};

use super::rows::{Fold, Row, Rows};
use super::{CARD_MARGIN, Navigator, RowStyle};
use crate::app::{MoveHere, MoveSelection};
use crate::backend::{Branch, CommitId, Destination, FullName, Workspace, short_ref};
use crate::state::{ProjectId, Section, Selection, Store, snack};
use crate::theme::Theme;
use crate::transfer::{Move, Stop};
use crate::ui::fold::on_top;
use crate::ui::{Icon, ROW_HEIGHT, dp, fixed, h_flex, small_icon};

/// The move under way in `project`, when it lands its commits somewhere at a stop `at` picks
/// out: the rows show copies of them there.
pub(super) fn landing_at<'a>(
    store: &'a Store,
    project: &ProjectId,
    at: impl Fn(&Stop) -> bool,
) -> Option<&'a Move> {
    (store.moving.as_ref())
        .filter(|moving| moving.project == *project && at(&moving.at) && moving.lands(store))
}

/// Whether the move under way in `project` parts stacks `upper` and `lower`, given by their
/// branches, `None` for a new stack after them all: when it would land in a new branch under
/// `upper`'s last branch or on `lower`'s first, which are the same place but for that.
pub(super) fn parts(
    store: &Store,
    project: &ProjectId,
    upper: &[Branch],
    lower: Option<&[Branch]>,
) -> bool {
    let last = upper.last().and_then(|branch| branch.ref_name.as_ref());
    let first = (lower.and_then(|lower| lower.first())).and_then(|branch| branch.ref_name.as_ref());
    landing_at(store, project, |at| match at {
        Stop::NewBranch { beside, below } => match below {
            true => last == Some(beside),
            false => first == Some(beside),
        },
        Stop::NewStack => lower.is_none(),
        Stop::In(_) => false,
    })
    .is_some()
}

/// Whether `item`, a commit being moved, would land elsewhere: `false` while the move is at
/// where it is, and `None` for a row that isn't moving.
pub(super) fn moving_away(store: &Store, item: &Selection) -> Option<bool> {
    let Selection::Commit(project, id) = item else {
        return None;
    };
    let moving = (store.moving.as_ref())
        .filter(|moving| moving.project == *project && moving.commits.contains(id))?;
    Some(moving.lands(store))
}

impl Rows {
    /// Copies of `moving`'s commits, under the new branch they'd land in with `new_branch`.
    pub(super) fn push_ghosts(&mut self, moving: &Move, new_branch: bool) {
        let ghosts = (moving.commits.iter()).map(|id| Row::Ghost(moving.project.clone(), *id));
        if !new_branch {
            return self.extend(ghosts);
        }
        self.push(Row::GhostBranch(moving.project.clone()));
        self.open();
        self.extend(ghosts);
        self.close();
    }
}

/// The places `commits` of `project` can land, in the order `rows` list them, and which of
/// them is where they are: the one just above the first of them. A folded branch has only its
/// top and the new branches either side of it, and a folded stack none, so what's folded is
/// stepped over and stays folded. `main_open` says whether the main checkout's stacks show,
/// which a new stack goes after.
pub(super) fn stops<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    project: &ProjectId,
    commits: &[CommitId],
    workspace: &Workspace,
    main_open: bool,
) -> (Vec<Stop>, usize) {
    /// A stack's bottom branch, once its rows are walked, takes a new branch under it.
    fn end_stack(stops: &mut Vec<Stop>, bottom: &mut Option<&FullName>) {
        if let Some(beside) = bottom.take() {
            stops.push(Stop::NewBranch {
                beside: beside.clone(),
                below: true,
            });
        }
    }
    let mut stops = Vec::new();
    let mut start = None;
    let mut bottom = None;
    // The commits listed now are in a branch they can land in.
    let mut in_branch = false;
    // The main checkout's stacks are listed now, which a new stack goes after.
    let mut in_main = false;
    for row in rows {
        match row {
            Row::Item(Selection::Uncommitted(p)) if p == project => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
                in_main = main_open;
            }
            Row::Item(Selection::Stack(p, _)) if p == project => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
            }
            // Not an unnamed branch, which has no ref to land on, nor the default branch,
            // which work stays off.
            Row::Item(Selection::Branch(p, Some(name)))
                if p == project && workspace.target_branch.as_ref() != Some(name) =>
            {
                end_stack(&mut stops, &mut bottom);
                // Only the workspace's stacks take new branches.
                let stack = (workspace.stack_of(Some(name))).filter(|_| workspace.managed);
                if let Some((stack, at)) = stack {
                    stops.push(Stop::NewBranch {
                        beside: name.clone(),
                        below: false,
                    });
                    if at + 1 == stack.branches.len() {
                        bottom = Some(name);
                    }
                }
                stops.push(Stop::In(Destination::Branch(name.clone())));
                in_branch = true;
            }
            Row::Item(Selection::Branch(p, _)) if p == project => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
            }
            Row::Item(Selection::Commit(p, id)) if p == project => {
                if commits.contains(id) {
                    start = start.or(stops.len().checked_sub(1));
                } else if in_branch {
                    stops.push(Stop::In(Destination::BelowCommit(*id)));
                }
            }
            // What a branch lists beside its own commits: its remote's, the folds of its
            // commits, stand-ins for commits on their way, and the copies of those moving.
            Row::Item(
                Selection::Upstream(..) | Selection::Section(_, Section::Upstream(Some(_))),
            )
            | Row::More(..)
            | Row::Fewer(..)
            | Row::Pending(..)
            | Row::Ghost(..)
            | Row::GhostBranch(_)
            | Row::Between(_) => {}
            _ => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
                if std::mem::take(&mut in_main) && workspace.managed {
                    stops.push(Stop::NewStack);
                }
            }
        }
    }
    end_stack(&mut stops, &mut bottom);
    if in_main && workspace.managed {
        stops.push(Stop::NewStack);
    }
    (stops, start.unwrap_or_default())
}

impl Navigator {
    /// The rows as last drawn, but those a closing fold takes away.
    fn shown_rows(&self) -> impl Iterator<Item = &Row> {
        let closing = |fold: &Option<super::rows::Moving>| {
            fold.as_ref()
                .is_some_and(|fold| self.folds.closing(&fold.fold))
        };
        (self.rows.iter().zip(&self.moving))
            .filter(move |(_, fold)| !closing(fold))
            .map(|(row, _)| row)
    }

    /// Where `commits` of `project` can land as the rows are now, as [`stops`] says.
    fn stops_for(
        &self,
        project: &ProjectId,
        commits: &[CommitId],
        cx: &App,
    ) -> Option<(Vec<Stop>, usize)> {
        let workspace = self.store.read(cx).workspace(project)?;
        let main = Fold::Under(Selection::Uncommitted(project.clone()));
        // The filter shows what it finds, folds aside.
        let main_open = self.folds_seen.get(&main).copied().unwrap_or(true);
        Some(stops(
            self.shown_rows(),
            project,
            commits,
            workspace,
            main_open,
        ))
    }

    /// The stops of the move under way, and which of them it's at: where its commits are, if
    /// the one it was at is gone, as when it's folded away.
    fn move_stops(&self, cx: &App) -> Option<(Vec<Stop>, usize)> {
        let moving = self.store.read(cx).moving.as_ref()?;
        let (stops, start) = self.stops_for(&moving.project, &moving.commits, cx)?;
        let at = (stops.iter().position(|stop| *stop == moving.at)).unwrap_or(start);
        Some((stops, at))
    }

    /// Start moving the selected commit, with those picked with it, from where they are. The
    /// keys that step through the rows step through where they can land until Return lands
    /// them or Escape lets go.
    pub(super) fn start_move(
        &mut self,
        _: &MoveSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some(Selection::Commit(project, id)) = store.selection.clone() else {
            return;
        };
        let commits = store.with_picks(&project, id);
        let start = (self.stops_for(&project, &commits, cx))
            .and_then(|(stops, start)| stops.into_iter().nth(start));
        let Some(at) = start else {
            return snack(&self.store, "Nowhere to move it", cx);
        };
        let moving = Move {
            project,
            commits,
            at,
        };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        window.focus(&self.focus, cx);
    }

    /// Step the move `delta` places on, stopping at either end.
    pub(super) fn step_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some((stops, at)) = self.move_stops(cx) else {
            return;
        };
        let last = stops.len().saturating_sub(1) as isize;
        let next = (at as isize).saturating_add(delta).clamp(0, last) as usize;
        self.move_to(stops, next, cx);
    }

    /// Jump the move to the top of the next branch, or with `forward` false the one before, or
    /// to a new stack, as ⇧J and ⇧K jump between sections.
    pub(super) fn jump_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some((stops, at)) = self.move_stops(cx) else {
            return;
        };
        let heads = |(_, stop): &(usize, &Stop)| {
            matches!(stop, Stop::In(Destination::Branch(_)) | Stop::NewStack)
        };
        let next = match forward {
            true => stops.iter().enumerate().skip(at + 1).find(heads),
            false => stops.iter().enumerate().take(at).rev().find(heads),
        };
        if let Some((next, _)) = next {
            self.move_to(stops, next, cx);
        }
    }

    fn move_to(&mut self, stops: Vec<Stop>, ix: usize, cx: &mut Context<Self>) {
        let Some(at) = stops.into_iter().nth(ix) else {
            return;
        };
        self.reveal_selection = true;
        self.store.update(cx, |store, cx| {
            if let Some(moving) = &mut store.moving
                && moving.at != at
            {
                moving.at = at;
                cx.notify();
            }
        });
    }

    /// Land the commits being moved where the move is at, as dropping them there would.
    pub(super) fn move_here(&mut self, _: &MoveHere, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(moving) = store.moving.clone() else {
            return;
        };
        let lands = moving.lands(store);
        self.store
            .update(cx, |store, cx| store.set_moving(None, cx));
        // Where they already are, there's nothing to do.
        if !lands {
            return;
        }
        if let Some((payload, target, place)) = moving.drop() {
            return self.app.update_later(window, cx, move |app, window, cx| {
                app.transfer(&payload, &target, place, window, cx)
            });
        }
        let beside = match moving.at {
            Stop::NewBranch { beside, below } => Some((beside, below)),
            Stop::In(_) | Stop::NewStack => None,
        };
        let Move {
            project, commits, ..
        } = moving;
        self.store.update(cx, |store, cx| {
            store.move_to_new_branch(&project, commits, beside, cx)
        });
    }
}

/// A copy of commit `id` being moved, where landing would put it, drawn as the commit's own
/// row is while the move is at where it is, set in by `inset` as an item row is.
pub(super) fn ghost_row(
    project: &ProjectId,
    id: CommitId,
    inset: f32,
    store: &Store,
    theme: &Theme,
) -> AnyElement {
    let style = RowStyle {
        held: true,
        ..RowStyle::plain(theme, inset)
    };
    match store.find_commit(project, id) {
        Some((_, commit)) => {
            (style.commit(format!("ghost-{project}-{}", id.0), project, commit, store))
                .into_any_element()
        }
        None => div().h(dp(ROW_HEIGHT)).into_any_element(),
    }
}

/// Commit `id` as a move left it, dimmed as its own row is, while its row goes back over it,
/// as the move returns there or ends: `at` from where the row's drawn, and set in by `inset` as
/// an item row is. Drawn over what the card's slices stretch over, so they don't cover it.
pub(super) fn stand_in(
    project: &ProjectId,
    id: CommitId,
    inset: f32,
    at: f32,
    store: &Store,
    theme: &Theme,
) -> Option<Div> {
    let (_, commit) = store.find_commit(project, id)?;
    let style = RowStyle {
        cut: true,
        ..RowStyle::plain(theme, inset)
    };
    let row = style.commit(
        format!("stand-in-{project}-{}", id.0),
        project,
        commit,
        store,
    );
    // Inside the card's edges, which its slice draws a hairline in, on its ground.
    let at = div()
        .absolute()
        .top(dp(at))
        .left(dp(CARD_MARGIN))
        .right(dp(CARD_MARGIN));
    Some(
        at.px(fixed(1.))
            .child(on_top(div().bg(theme.bg_1).child(row))),
    )
}

/// The new branch commits being moved would land in, over their copies, set in by `inset` as
/// an item row is. It names the branch it goes above or below in their stack, as between two
/// stacks the new branch under one's last branch and the one on the next's first are in the
/// same place.
pub(super) fn ghost_branch_row(inset: f32, store: &Store, theme: &Theme) -> AnyElement {
    let beside = match store.moving.as_ref().map(|moving| &moving.at) {
        Some(Stop::NewBranch { beside, below }) => {
            let side = if *below { "below" } else { "above" };
            Some((side, short_ref(beside).into_owned()))
        }
        Some(Stop::In(_) | Stop::NewStack) | None => None,
    };
    let said = match &beside {
        Some((side, _)) => format!("New branch {side}"),
        None => "New branch".into(),
    };
    h_flex()
        .min_h(dp(ROW_HEIGHT))
        .mx(dp(3.))
        .pl(dp(8. + inset))
        .pr(dp(12.))
        .gap(dp(8.))
        .rounded(dp(4.))
        .border_1()
        .border_dashed()
        .border_color(theme.pop)
        .text_color(theme.pop)
        .child(small_icon(Icon::Branch).text_color(theme.pop))
        .child(
            // One phrase, a space apart, the branch's name as it's written in its row.
            h_flex()
                .min_w_0()
                .gap(dp(4.))
                .child(div().flex_none().child(said))
                .children(beside.map(|(_, name)| {
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.text_1)
                        .child(name)
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Row, stops};
    use crate::backend::model::build::{branch, full, id, workspace};
    use crate::backend::{CommitId, Destination, Workspace, short_ref};
    use crate::state::{ProjectId, Selection};
    use crate::transfer::Stop;

    /// Each stop on a line, commits by their number, `<` at `start`.
    fn describe(stops: &[Stop], start: usize) -> String {
        let commit = |c: &CommitId| (1..=9).find(|n| id(*n) == *c).unwrap_or_default();
        let short = |name| short_ref(name).into_owned();
        (stops.iter().enumerate())
            .map(|(ix, stop)| {
                let said = match stop {
                    Stop::NewBranch { beside, below } => {
                        let side = if *below { "below" } else { "above" };
                        format!("new branch {side} {}", short(beside))
                    }
                    Stop::In(Destination::Branch(name)) => format!("top of {}", short(name)),
                    Stop::In(Destination::BelowCommit(c)) => format!("below {}", commit(c)),
                    Stop::In(Destination::AboveCommit(c)) => format!("above {}", commit(c)),
                    Stop::NewStack => "new stack".into(),
                };
                let here = if ix == start { " <" } else { "" };
                format!("{said}{here}\n")
            })
            .collect()
    }

    /// Two stacks, `ui` on `api` and `typo` alone, and the rows the navigator lists for them,
    /// with `typo` folded.
    fn listed(project: &ProjectId) -> (Workspace, Vec<Row>) {
        let ws = Workspace {
            managed: true,
            ..workspace(
                vec![
                    vec![branch("ui", &[3]), branch("api", &[2, 1])],
                    vec![branch("typo", &[4])],
                ],
                vec![],
            )
        };
        let p = || project.clone();
        let named = |name: &str| Some(full(&format!("refs/heads/{name}")));
        let rows = vec![
            Row::Item(Selection::Project(p())),
            Row::Item(Selection::Uncommitted(p())),
            Row::Item(Selection::Stack(p(), named("api"))),
            Row::Item(Selection::Branch(p(), named("ui"))),
            Row::Item(Selection::Commit(p(), id(3))),
            Row::Item(Selection::Branch(p(), named("api"))),
            Row::Item(Selection::Commit(p(), id(2))),
            Row::Item(Selection::Commit(p(), id(1))),
            // As a move opens room between stacks, which parts nothing it walks.
            Row::Between(p()),
            Row::Item(Selection::Branch(p(), named("typo"))),
            Row::WorktreeDivider,
            Row::Item(Selection::Branches(p())),
        ];
        (ws, rows)
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_commit_stops_between_every_branch_and_commit_but_in_what_is_folded() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (stops, start) = stops(&rows, &project, &[id(2)], &ws, true);
        // A new branch above every branch of a stack and below its last, each branch's top
        // and the place below each of its commits but the one moving, which starts below 3 at
        // the top of api; then a new stack once the main checkout's stacks end. Folded, typo
        // has only its top.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new branch above ui
top of ui
below 3
new branch above api
top of api <
below 1
new branch below api
new branch above typo
top of typo
new branch below typo
new stack

"#]]
        );
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn a_folded_main_checkout_and_a_workspace_of_one_branch_take_no_new_branches() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (folded, _) = stops(rows.iter().take(2), &project, &[id(2)], &ws, false);
        assert!(
            folded.is_empty(),
            "nothing shows under the folded heading, and no new stack goes after it"
        );
        let unmanaged = Workspace {
            managed: false,
            ..ws
        };
        let (stops, start) = stops(&rows, &project, &[id(1)], &unmanaged, true);
        // Outside the workspace there's no stack to put a branch in.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
top of ui
below 3
top of api
below 2 <
top of typo

"#]]
        );
    }
}
