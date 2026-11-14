//! Moving commits or a branch with the keys, or squashing them or files into a commit: the
//! places they can land as the rows list them, stepping through those, the copies that show
//! where they'd land, and landing them.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use gpui::{AnyElement, App, Context, Div, Hsla, SharedString, Window, div, prelude::*};

use super::drop::{stack_end, stack_start};
use super::order::moving_terminal;
use super::rows::{
    COMMIT_INDENT, Fold, Row, RowKey, Rows, heads, listed_with, lists_changes, span_height,
};
use super::{Navigator, RowStyle};
use crate::app::{
    MoveHere, MoveSelection, SquashIntoSelection, SquashSelection, StartAmend, UseTargetMessage,
};
use crate::backend::{
    Checkout, Commit, CommitId, Destination, FullName, Messages, Workspace, short_ref,
};
use crate::state::{Overlay, ProjectCheckout, ProjectId, Section, Selection, Store, snack};
use crate::terminal::TerminalId;
use crate::theme::{Hue, Theme};
use crate::transfer::{
    Changes, Move, Moving, Op, Slot, Stop, Target, Transfer, group_slots, slots,
};
use crate::ui::fold::{self, on_top};
use crate::ui::{Icon, blink_once, dp, fixed, guide, h_flex, icon, small_icon};

/// The move of commits or a branch shown in `project`, or the squash of those or files, with
/// copies where it'd land: the one under way, or one Return landed, until the reload after it
/// shows where it went. A project's card, a linked worktree and a terminal go where they'd land
/// themselves (see [`Navigator::watched`] and the rows' worktrees and terminals), the last two
/// held while they move ([`Mark::Held`]).
fn shown<'a>(store: &'a Store, project: &ProjectId) -> Option<&'a Move> {
    (store.moving.as_ref().or_else(|| store.landing())).filter(|moving| {
        moving.project == *project
            && matches!(
                moving.what,
                Moving::Commits(_) | Moving::Branch(_) | Moving::Files(_) | Moving::Copies(_)
            )
    })
}

/// The move shown in `project`, when it lands what it moves somewhere at a stop `at` picks
/// out: the rows show copies of it there. What's dragged shows no copies, as they'd push the rows
/// the pointer is aiming at out of its way: a divider shows where it goes instead.
pub(super) fn landing_at<'a>(
    store: &'a Store,
    project: &ProjectId,
    at: impl Fn(&Stop) -> bool,
) -> Option<&'a Move> {
    shown(store, project)
        .filter(|moving| !moving.dragged(store) && at(&moving.at) && moving.shows_landing(store))
}

/// Whether what lands at `at` shows there as copies of itself, between rows, as commits and
/// a branch being moved do, rather than as an outline over the row it goes into, as a squash
/// does ([`squash_outline`]). A card, a worktree and a terminal go there themselves.
fn copied_at(at: &Stop) -> bool {
    match at {
        Stop::In(_) | Stop::NewBranch { .. } | Stop::NewStack { .. } => true,
        Stop::Into(_) | Stop::Card(_) | Stop::Worktree(_) | Stop::Terminal { .. } => false,
    }
}

/// Whether what's moving has copies of itself where it's at, as [`copied_at`] says. Not what's
/// dragged, which shows none ([`landing_at`]).
fn shows_copy(moving: &Move, store: &Store) -> bool {
    !moving.dragged(store) && copied_at(&moving.at)
}

/// How the move shown marks a row, in the colour of its op.
#[derive(Clone, Copy)]
pub(super) enum Mark {
    /// What it's of, dimmed and dashed: the commits, a branch with its
    /// commits, or the row files are listed under. It is, all of it, wherever the move is,
    /// where it started too, so nothing under what shows where it goes changes as that slides
    /// from row to row, and back over it: a copy of it, or the outline of what a squash goes
    /// into ([`squash_outline`]).
    Taken(Op),
    /// As [`Mark::Taken`], while the move is at where it is: its copy is drawn over it there,
    /// which slides out to where it'd land and back over it.
    Copied(Op),
    /// A linked worktree's heading or a terminal being moved, which goes where it'd land
    /// itself: held rather than selected.
    Held(Op),
}

/// How the move shown in `item`'s project marks its row, if it does, as [`Mark`] says.
pub(super) fn mark(store: &Store, item: &Selection) -> Option<Mark> {
    if let Some(moving) = (store.moving.as_ref()).filter(|moving| {
        matches!(
            moving.what,
            Moving::Worktree { .. } | Moving::Terminal { .. }
        ) && moving.holds(item)
    }) {
        return Some(Mark::Held(moving.op));
    }
    let moving = shown(store, item.project())?;
    let holds = moving.holds(item);
    if !holds && !of_branch(moving, store, item) {
        return None;
    }
    let home = shows_copy(moving, store) && !moving.lands(store);
    // A drag's op follows the pointer: what it holds stays as it is meanwhile.
    let op = match moving.dragged(store) {
        true => Op::Move,
        false => moving.op,
    };
    Some(match holds && home {
        true => Mark::Copied(op),
        false => Mark::Taken(op),
    })
}

/// The hue the move shown in `project` marks rows in, as [`Op::hue`] says.
pub(super) fn mark_of(store: &Store, project: &ProjectId) -> Hue {
    shown(store, project).map_or(Hue::Pop, |moving| moving.op.hue())
}

/// Whether `item` is a commit of the branch `moving` moves or squashes.
fn of_branch(moving: &Move, store: &Store, item: &Selection) -> bool {
    match (&moving.what, item) {
        (Moving::Branch(branch), Selection::Commit(project, id)) => {
            *project == moving.project
                && (store.find_commit(project, *id))
                    .is_some_and(|(on, _)| on.ref_name.as_ref() == Some(branch))
        }
        _ => false,
    }
}

/// Whether `item`'s row is what's moving, left where it is while its copy shows where it'd
/// land, or over it while that's where it is. A commit being made is no copy of the row its
/// changes are under.
pub(super) fn left_behind(store: &Store, item: &Selection) -> bool {
    shown(store, item.project()).is_some_and(|moving| {
        matches!(moving.what, Moving::Commits(_) | Moving::Branch(_))
            && shows_copy(moving, store)
            && moving.holds(item)
    })
}

/// Where the outline of the squash shown is: around what it goes into, or back where it
/// started, around the rows held there, drawn with the row it started from. Files dragged onto
/// a branch outline it the same way, as they start a commit in it.
#[derive(Clone)]
pub(super) struct SquashOutline {
    /// The row it's drawn with.
    pub(super) row: Selection,
    /// How far down the rows that row's top and bottom are.
    row_top: f32,
    row_bottom: f32,
    /// How far down the rows its top and bottom are.
    top: f32,
    bottom: f32,
}

impl SquashOutline {
    /// The outline of the squash shown in `rows`: around what it goes into, or with `home`,
    /// back where it started, around the selection's row and those squashed with it, as their
    /// dashed outline goes around them. An item's row is `row` tall.
    pub(super) fn of(rows: &[Row], store: &Store, home: bool, row: f32) -> Option<SquashOutline> {
        let squashed = |ix: &usize| matches!(rows.get(*ix), Some(Row::Item(item)) if matches!(mark(store, item), Some(Mark::Taken(_))));
        let (ix, first, last) = (rows.iter().enumerate()).find_map(|(ix, row)| match row {
            // A branch a branch is stacked with, around all of its stack.
            Row::Item(item)
                if lands_on(store, item)
                    && shown(store, item.project())
                        .is_some_and(|moving| stacks_with(moving, store, item)) =>
            {
                Some((ix, stack_start(rows, ix), stack_end(rows, ix) - 1))
            }
            Row::Item(item) if lands_on(store, item) => Some((ix, ix, ix)),
            Row::Item(item) if home && store.selection.as_ref() == Some(item) => {
                let first = (0..ix).rev().take_while(squashed).last().unwrap_or(ix);
                let last = (ix + 1..rows.len())
                    .take_while(squashed)
                    .last()
                    .unwrap_or(ix);
                Some((ix, first, last))
            }
            _ => None,
        })?;
        let Some(Row::Item(item)) = rows.get(ix) else {
            return None;
        };
        Some(SquashOutline {
            row: item.clone(),
            row_top: span_height(rows, 0..ix, row),
            row_bottom: span_height(rows, 0..ix + 1, row),
            top: span_height(rows, 0..first, row),
            bottom: span_height(rows, 0..last + 1, row),
        })
    }

    /// How far its top and bottom edges slide as it moves from `was` to here.
    pub(super) fn slide(&self, was: &SquashOutline) -> (f32, f32) {
        (was.top - self.top, was.bottom - self.bottom)
    }

    /// Where it's drawn from its row, by its top's and its bottom's distance from the row's,
    /// with its edges `slid` as far as they have yet to slide.
    pub(super) fn edges(&self, slid: (f32, f32)) -> (f32, f32) {
        let (top, bottom) = slid;
        (
            self.top - self.row_top + top,
            self.bottom - self.row_bottom + bottom,
        )
    }
}

/// Whether what's moving lands on `item`'s row, which [`squash_outline`] outlines: what a squash
/// goes into, elsewhere than where it started, the branch dragged files start a commit in, or the
/// branch a dragged branch is stacked beside, from another stack, whose stack it's around.
pub(super) fn lands_on(store: &Store, item: &Selection) -> bool {
    shown(store, item.project()).is_some_and(|moving| {
        (moving.squashes_into(item)
            || heading_takes(moving, store, item)
            || starts_commit_in(moving, store, item)
            || stacks_with(moving, store, item))
            && moving.shows_landing(store)
    })
}

/// Whether `item` is the branch a dragged branch is stacked beside, so in a stack with it that
/// it isn't in yet: a new one, or the one `item`'s in.
fn stacks_with(moving: &Move, store: &Store, item: &Selection) -> bool {
    matches!(item, Selection::Branch(project, Some(name))
        if *project == moving.project && moving.stacks_with(store) == Some(name))
}

/// What the outline around the stack a dragged branch is dropped into says, drawn with `item`,
/// the branch it's dropped on: "New stack" for one on its own, else "Join stack".
pub(super) fn stack_label(store: &Store, item: &Selection) -> Option<&'static str> {
    let Selection::Branch(project, Some(name)) = item else {
        return None;
    };
    let stacks = lands_on(store, item)
        && shown(store, project).is_some_and(|moving| stacks_with(moving, store, item));
    let (lane, _) = store.workspace(project)?.lane_of(Some(name))?;
    stacks.then_some(match lane.branches.len() {
        1 => "New stack",
        _ => "Join stack",
    })
}

/// The outline's `label`, at the right of its top row, `row` tall, in `hue`: on the card's
/// ground, over what that row shows there.
pub(super) fn outline_label(label: &'static str, row: f32, hue: Hue, theme: &Theme) -> Div {
    let colour = theme.hue(hue);
    h_flex()
        .absolute()
        .top_0()
        .right(dp(4.))
        .h(dp(row - 2.))
        .px(dp(6.))
        .gap(dp(4.))
        .rounded(dp(theme.shape.control))
        .bg(theme.bg_1)
        .text_size(dp(12.))
        .text_color(colour)
        .child(small_icon(Icon::Stack).text_color(colour))
        .child(label)
}

/// Whether `item` is the branch that files dragged onto it start a commit in. Its row is
/// outlined, as a line where the commit goes would read as between the branches while it's
/// folded.
fn starts_commit_in(moving: &Move, store: &Store, item: &Selection) -> bool {
    let Selection::Branch(project, Some(name)) = item else {
        return false;
    };
    moving.op == Op::Commit
        && moving.dragged(store)
        && *project == moving.project
        && matches!(&moving.at, Stop::In(Destination::Branch(at)) if at == name)
}

/// Whether `item` is the heading of the checkout a squash uncommits into, or the project's row
/// standing in for it, with no changes listed under it to take the outline.
fn heading_takes(moving: &Move, store: &Store, item: &Selection) -> bool {
    let (project, checkout) = match item {
        Selection::Checkout(project, checkout) => (project, checkout),
        Selection::Project(project) if !store.main_headed(project) => (project, &Checkout::Main),
        _ => return false,
    };
    matches!(&moving.at, Stop::Into(Target::Uncommitted(at)) if at == checkout)
        && *project == moving.project
        && (store.workspace(project)).is_some_and(|ws| !lists_changes(store, ws, project, checkout))
}

/// A row a move took away as it stepped on, landed or ended, still drawn where it was while the
/// rows close over it: `by` below the nearest row above it that stays, `under`, which it moves
/// with, set in by `inset` and with the guides it had.
pub(super) struct StandIn {
    of: Gone,
    pub(super) under: RowKey,
    by: f32,
    inset: f32,
    guides: Vec<f32>,
    /// When it was taken away, which it fades from as the rows close over it, at their pace:
    /// so no sliver of it stays as they come to rest, nor does it brighten if they set off again.
    since: Instant,
    /// How much of it shows in the frame being drawn.
    shown: f32,
    /// What took it away, which marks it in its colour.
    op: Op,
    /// The stand-ins above and below it, taken away with it, are outlined as it is.
    outlined_with: (bool, bool),
}

impl StandIn {
    /// Fade it to the frame drawn at `now`, saying whether any of it still shows.
    pub(super) fn fade(&mut self, now: Instant) -> bool {
        self.shown = fold::openness(false, self.since, now);
        self.shown > 0.
    }
}

/// What a move took away.
enum Gone {
    /// A commit being moved, dimmed where the move left it, as it goes back there or lands, or
    /// squashed away.
    Commit(ProjectId, Box<Commit>),
    /// A branch being moved, as [`branch_copy`] draws it, dimmed where the move left it.
    Branch { id: String, name: String },
    /// The new branch commits being moved would land in at that stop.
    NewBranch(Stop),
}

/// The rows keyed `old` that a move takes away as they go to `new`, the commits where it left
/// them, once it's `landing` them elsewhere, and the new branch they'd land in, each by its
/// index and that of the nearest row above it that stays. As a squash `ended`, every commit
/// that goes too, as it took them into another.
fn taken_away(
    old: &[Option<RowKey>],
    new: &[Option<RowKey>],
    ended: bool,
    landing: bool,
) -> Vec<(usize, usize)> {
    let kept: HashSet<&RowKey> = new.iter().flatten().collect();
    let stays =
        |ix: &usize| (old.get(*ix).and_then(Option::as_ref)).is_some_and(|key| kept.contains(key));
    (old.iter().enumerate())
        .filter(|(_, key)| match key {
            // Unless it lands elsewhere, what's moving takes its own row back where it's left.
            Some(key @ RowKey::Left(own)) => {
                !kept.contains(key) && (landing || !kept.contains(&**own))
            }
            Some(key @ RowKey::GhostBranch(_)) => !kept.contains(key),
            Some(key @ RowKey::Commit(..)) => ended && !kept.contains(key),
            Some(
                RowKey::Group(_)
                | RowKey::Item(_)
                | RowKey::Terminal(_)
                | RowKey::RestsOn(_)
                | RowKey::Fold(..)
                | RowKey::NewBranch(_)
                | RowKey::NewCommit(_)
                | RowKey::SquashTop
                | RowKey::SquashBottom
                | RowKey::After(_),
            )
            | None => false,
        })
        .filter_map(|(ix, _)| Some((ix, (0..ix).rev().find(stays)?)))
        .collect()
}

impl Rows {
    /// Copies of what `moving` moves: its commits, or the commit uncommitted changes being
    /// committed make, under the new branch they'd land in with `new_branch`, or its branch's
    /// row, which heads its commits wherever it goes.
    pub(super) fn push_ghosts(&mut self, moving: &Move, new_branch: bool) {
        let project = &moving.project;
        let copies: Vec<Row> = match &moving.what {
            Moving::Commits(ids) => (ids.iter())
                .map(|id| Row::Ghost(Selection::Commit(project.clone(), *id)))
                .collect(),
            // As the commits they'd be, which [`ghost_row`] draws by their titles.
            Moving::Copies(copies) => (copies.iter())
                .map(|(id, _)| Row::Ghost(Selection::Commit(project.clone(), *id)))
                .collect(),
            Moving::Branch(branch) => {
                let item = Selection::Branch(project.clone(), Some(branch.clone()));
                return self.push(Row::Ghost(item));
            }
            Moving::Files(_) if moving.op == Op::Commit => {
                Vec::from([Row::NewCommit(project.clone())])
            }
            // A card, a worktree or a terminal goes where it'd land itself, and files squashed
            // go into a row.
            Moving::Project(_)
            | Moving::Worktree { .. }
            | Moving::Terminal { .. }
            | Moving::Files(_) => return,
        };
        if !new_branch {
            return self.extend(copies);
        }
        self.push(Row::GhostBranch(moving.project.clone(), moving.at.clone()));
        self.open();
        self.extend(copies);
        self.close();
    }
}

/// The places `commits` of `project` can land, in the order `rows` list them, and which of
/// them is where they are: the one just above the first of them. `copied`, as a cherry-pick
/// leaves them there, they land right below themselves too, and that one above them is where
/// their copies go right above them, not where they already are. A folded branch takes them at
/// its top, where they show under its row, its commits stepped over and it staying folded.
/// `open` says whether a heading shows what it holds: the main checkout's stacks show only
/// while its heading's open, as a new stack goes before and after them, which starts from the
/// target, so not without one, and with `between`, as a branch goes, between every two of
/// them too. A linked worktree's lane takes new branches as a stack does, and so does the stack
/// checked out without the workspace, where a new stack sets the workspace up.
pub(super) fn stops<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    project: &ProjectId,
    commits: &[CommitId],
    copied: bool,
    between: bool,
    workspace: &Workspace,
    open: impl Fn(&Selection) -> bool,
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
    let new_stacks = workspace.target_branch.is_some();
    let mut stops = Vec::new();
    let mut start = None;
    let mut bottom = None;
    // The commits listed now are in a branch they can land in.
    let mut in_branch = false;
    // The main checkout's stacks are listed now, which a new stack goes after, and before the
    // next of them, until that's listed: the first, and with `between` any other.
    let (mut in_main, mut stack_next, mut first) = (false, false, false);
    let mut rows = rows.into_iter().peekable();
    while let Some(row) = rows.next() {
        if stack_next
            && let Row::Item(Selection::Branch(p, name) | Selection::Stack(p, name)) = row
            && p == project
        {
            stack_next = false;
            if let Some(name) = name.as_ref().filter(|_| first || between) {
                stops.push(Stop::NewStack {
                    before: Some(name.clone()),
                });
            }
            first = false;
        }
        match row {
            // The main checkout's heading, or the project's row standing in for it.
            Row::Item(heading)
                if heading.project() == project
                    && heads(row, rows.peek().copied()) == Some(Checkout::Main) =>
            {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
                in_main = open(heading);
                (stack_next, first) = (in_main && new_stacks, true);
            }
            // Not an unnamed branch, which has no ref to land on, nor the default branch,
            // which work stays off.
            Row::Item(Selection::Branch(p, Some(name)))
                if p == project && workspace.target_branch.as_ref() != Some(name) =>
            {
                end_stack(&mut stops, &mut bottom);
                if let Some((lane, at)) = workspace.lane_of(Some(name)) {
                    stops.push(Stop::NewBranch {
                        beside: name.clone(),
                        below: false,
                    });
                    if at + 1 == lane.branches.len() {
                        bottom = Some(name);
                    }
                }
                stops.push(Stop::In(Destination::Branch(name.clone())));
                in_branch = true;
            }
            // An unnamed or the default branch, or a stack's caption: what's above it ends.
            Row::Item(Selection::Branch(p, _) | Selection::Stack(p, _)) if p == project => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
            }
            Row::Item(Selection::Commit(p, id)) if p == project => {
                let own = commits.contains(id);
                if own {
                    start = start.or(stops.len().checked_sub(1));
                }
                if in_branch && (copied || !own) {
                    stops.push(Stop::In(Destination::BelowCommit(*id)));
                }
            }
            // What a branch lists beside its own commits: its remote's, the folds of its
            // commits, stand-ins for commits on their way, and the copies of those moving; and
            // a checkout's uncommitted changes, first under its heading.
            Row::Item(
                Selection::Upstream(..)
                | Selection::Section(_, Section::Upstream(_))
                | Selection::Pending(..)
                | Selection::Uncommitted(..),
            )
            | Row::More(..)
            | Row::Fewer(..)
            | Row::Pending(..)
            | Row::Ghost(..)
            | Row::GhostBranch(..)
            | Row::NewCommit(_) => {}
            // The next stack starts: what's listed above it ends.
            Row::Between(p) if p == project => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
                stack_next = in_main && new_stacks;
            }
            Row::Between(_) => {}
            // A checkout's terminals, listed above its branches.
            Row::Item(Selection::Terminal(..)) => {}
            _ => {
                end_stack(&mut stops, &mut bottom);
                in_branch = false;
                stack_next = false;
                if std::mem::take(&mut in_main) && new_stacks {
                    stops.push(Stop::NewStack { before: None });
                }
            }
        }
    }
    end_stack(&mut stops, &mut bottom);
    if in_main && new_stacks {
        stops.push(Stop::NewStack { before: None });
    }
    (stops, start.unwrap_or_default())
}

/// The commit right under a new commit placed at `at`, which the same changes amend instead.
fn under_commit_at(at: &Stop, workspace: &Workspace) -> Option<CommitId> {
    match at {
        Stop::In(Destination::Branch(branch)) => {
            Some(workspace.find_branch(Some(branch))?.commits.first()?.id)
        }
        Stop::In(Destination::AboveCommit(id)) => Some(*id),
        Stop::In(Destination::BelowCommit(id)) => {
            let (branch, _) = workspace.find_commit(*id)?;
            let at = branch.commits.iter().position(|commit| commit.id == *id)?;
            Some(branch.commits.get(at + 1)?.id)
        }
        Stop::NewBranch { .. }
        | Stop::NewStack { .. }
        | Stop::Into(_)
        | Stop::Card(_)
        | Stop::Worktree(_)
        | Stop::Terminal { .. } => None,
    }
}

/// The rows of `project`, in the order `rows` list them, that what's being squashed can go
/// into, as `lands` says, and which of them is `home`, the row it starts from, listed whether
/// it lands there or not. A branch's commits take it, but a folded branch stands in for its
/// newest commit, out of sight, and the branch being squashed squashes into one at its own row.
/// A checkout's uncommitted changes take what's uncommitted into them, or with none listed, its
/// open heading does. Not the default branch a linked worktree can have checked out, which work
/// stays off.
pub(super) fn squash_stops<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    project: &ProjectId,
    workspace: &Workspace,
    home: &Target,
    open: impl Fn(&Selection) -> bool,
    lands: impl Fn(&Stop) -> bool,
) -> Option<(Vec<Stop>, usize)> {
    let mut stops = Vec::new();
    // The commits listed now are in a branch they can go into.
    let mut in_branch = false;
    let mut rows = rows.into_iter().peekable();
    while let Some(row) = rows.next() {
        let target = match row {
            Row::Item(Selection::Uncommitted(p, checkout)) if p == project => {
                in_branch = false;
                Some(Target::Uncommitted(checkout.clone()))
            }
            // A worktree's heading, or the project's row standing in for the main one's.
            Row::Item(heading @ (Selection::Checkout(p, _) | Selection::Project(p)))
                if p == project =>
            {
                in_branch = false;
                let next = rows.peek().copied();
                let changes = matches!(next, Some(Row::Item(Selection::Uncommitted(..))));
                let target = heads(row, next).map(Target::Uncommitted);
                target.filter(|_| open(heading) && !changes)
            }
            Row::Item(item @ Selection::Branch(p, Some(name)))
                if p == project && workspace.target_branch.as_ref() != Some(name) =>
            {
                in_branch = true;
                let target = Target::Branch(name.clone());
                (!open(item) || target == *home).then_some(target)
            }
            Row::Item(Selection::Branch(p, _)) if p == project => {
                in_branch = false;
                None
            }
            Row::Item(Selection::Commit(p, id)) if p == project => {
                let target = Target::Commit(*id);
                (in_branch || target == *home).then_some(target)
            }
            // What a branch lists beside its own commits, and a checkout's terminals.
            Row::Item(
                Selection::Upstream(..)
                | Selection::Section(_, Section::Upstream(_))
                | Selection::Pending(..)
                | Selection::Terminal(..),
            )
            | Row::More(..)
            | Row::Fewer(..)
            | Row::Pending(..)
            | Row::Ghost(..)
            | Row::GhostBranch(..)
            | Row::NewCommit(_)
            | Row::Between(_) => None,
            _ => {
                in_branch = false;
                None
            }
        };
        let Some(target) = target else {
            continue;
        };
        let is_home = target == *home;
        let stop = Stop::Into(target);
        if is_home || lands(&stop) {
            stops.push(stop);
        }
    }
    // Past the last with `home` listed nowhere, as what's dragged from a page can be.
    let start = (stops.iter())
        .position(|stop| matches!(stop, Stop::Into(at) if at == home))
        .unwrap_or(stops.len());
    Some((stops, start))
}

impl Navigator {
    /// Stand-ins for the rows as last drawn, with the move `was` shown, that it takes away at
    /// `now` as they go to those keyed `keys`, as the squash that was shown `ended` if it did,
    /// or the move Return landed shows `landing` it.
    pub(super) fn stand_ins(
        &self,
        keys: &[Option<RowKey>],
        was: Option<Op>,
        (ended, landing): (bool, bool),
        now: Instant,
        store: &Store,
    ) -> Vec<StandIn> {
        let Some(op) = was else {
            return Vec::new();
        };
        let taken = taken_away(&self.row_keys, keys, ended, landing);
        // Those taken away together, next to each other, stay outlined as one.
        let together: HashSet<usize> = (taken.iter())
            .map(|(ix, _)| *ix)
            .filter(|ix| matches!(self.rows.get(*ix), Some(Row::Item(_))))
            .collect();
        (taken.into_iter())
            .filter_map(|(ix, above)| {
                let of = match self.rows.get(ix)? {
                    Row::Item(Selection::Commit(project, id)) => {
                        // Landed, it's another commit where it went, found by its key.
                        let key = match self.row_keys.get(ix).cloned().flatten() {
                            Some(RowKey::Left(own)) => match *own {
                                RowKey::Commit(_, key) => Some(key),
                                _ => None,
                            },
                            _ => None,
                        };
                        let commit = (store.find_commit(project, *id).map(|(_, commit)| commit))
                            .or_else(|| {
                                let key = key?;
                                store.workspace(project)?.find_commit_by_key(&key)
                            })?;
                        Gone::Commit(project.clone(), Box::new(commit.clone()))
                    }
                    Row::Item(Selection::Branch(project, ref_name @ Some(_))) => {
                        let workspace = store.workspace(project)?;
                        let branch = workspace.find_branch(ref_name.as_ref())?;
                        Gone::Branch {
                            id: format!("stand-in-{project}-{}", branch.name),
                            name: store.name_of(project, branch).to_owned(),
                        }
                    }
                    Row::GhostBranch(_, at) => Gone::NewBranch(at.clone()),
                    _ => return None,
                };
                let outlined_with = (
                    ix.checked_sub(1).is_some_and(|ix| together.contains(&ix)),
                    together.contains(&(ix + 1)),
                );
                Some(StandIn {
                    of,
                    outlined_with,
                    under: self.row_keys.get(above)?.clone()?,
                    by: span_height(&self.rows, above..ix, self.row),
                    inset: self.insets.get(ix).copied().unwrap_or_default(),
                    guides: self.guides.get(ix).cloned().unwrap_or_default(),
                    since: now,
                    shown: 1.,
                    op,
                })
            })
            .collect()
    }

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

    /// Where `what` of `project` can land as `op` as the rows are now, and which of those places
    /// is where it is: moved, as [`stops`] says, or squashed, as [`squash_stops`] says, from
    /// where [`Moving::home`] says.
    pub(super) fn stops_for(
        &self,
        project: &ProjectId,
        what: &Moving,
        op: Op,
        cx: &App,
    ) -> Option<(Vec<Stop>, usize)> {
        let store = self.store.read(cx);
        match what {
            Moving::Project(order) => {
                let (slots, start) = slots(order, project)?;
                return Some((slots.into_iter().map(Stop::Card).collect(), start));
            }
            Moving::Worktree { name, order } => {
                let (slots, start) = slots(order, name)?;
                return Some((slots.into_iter().map(Stop::Worktree).collect(), start));
            }
            Moving::Terminal { id, lists } => {
                let (slots, start) = group_slots(lists, id)?;
                let stops = slots
                    .into_iter()
                    .map(|(checkout, slot)| Stop::Terminal { checkout, slot });
                return Some((stops.collect(), start));
            }
            // Files are only squashed, and committed, and only commits are cherry-picked, the
            // workspace's or copies of others.
            Moving::Files(_) if matches!(op, Op::Move | Op::CherryPick) => return None,
            Moving::Branch(_) if op == Op::CherryPick => return None,
            Moving::Copies(_) if op != Op::CherryPick => return None,
            Moving::Commits(_) | Moving::Branch(_) | Moving::Files(_) | Moving::Copies(_) => {}
        }
        let workspace = store.workspace(project)?;
        let open = |heading: &Selection| self.open_as_built(heading);
        let lands = |at: &Stop| {
            // Commits picked together squash at the one selected, where they start, the outline
            // around them all, not into the others one by one.
            let among = matches!((what, at),
                (Moving::Commits(ids), Stop::Into(Target::Commit(id))) if ids.contains(id));
            let (project, what, at) = (project.clone(), what.clone(), at.clone());
            let moving = Move {
                project,
                what,
                at,
                op,
            };
            !among && moving.lands(store)
        };
        match op {
            Op::Move => {
                let commits = match what {
                    Moving::Commits(ids) => &ids[..],
                    Moving::Branch(_)
                    | Moving::Project(_)
                    | Moving::Worktree { .. }
                    | Moving::Terminal { .. }
                    | Moving::Files(_)
                    | Moving::Copies(_) => &[],
                };
                let between = matches!(what, Moving::Branch(_));
                let all = stops(
                    self.shown_rows(),
                    project,
                    commits,
                    false,
                    between,
                    workspace,
                    open,
                );
                let Moving::Branch(branch) = what else {
                    return Some(all);
                };
                branch_stops(all.0, branch, lands)
            }
            // Where commits moved can land, and right below the workspace's, which start right
            // above where they are, as copies go beside them. Copies of others, which are
            // nowhere here, start on top of the topmost branch, as a new commit does.
            Op::CherryPick => {
                let Moving::Commits(commits) = what else {
                    let (all, _) = stops(
                        self.shown_rows(),
                        project,
                        &[],
                        false,
                        false,
                        workspace,
                        open,
                    );
                    let topmost = (all.iter())
                        .position(|stop| matches!(stop, Stop::In(Destination::Branch(_))))
                        .unwrap_or_default();
                    return Some((all, topmost));
                };
                Some(stops(
                    self.shown_rows(),
                    project,
                    commits,
                    true,
                    false,
                    workspace,
                    open,
                ))
            }
            Op::Squash(_) => {
                let home = what.home(project, store.selection.as_ref())?;
                squash_stops(self.shown_rows(), project, workspace, &home, open, lands)
            }
            // Where commits moved can land. It starts where it's started from
            // ([`Self::start_committing`]), not where anything is.
            Op::Commit => {
                let (all, _) = stops(
                    self.shown_rows(),
                    project,
                    &[],
                    false,
                    false,
                    workspace,
                    open,
                );
                let mut stops: Vec<Stop> = all.into_iter().filter(|stop| lands(stop)).collect();
                // The main checkout's changes can always go in a new branch of their own, after
                // the rest, even with its heading folded and none of its branches listed.
                let new_stack = Stop::NewStack { before: None };
                if !stops.contains(&new_stack) && lands(&new_stack) {
                    stops.push(new_stack);
                }
                Some((stops, 0))
            }
        }
    }

    /// Whether `heading` showed what it holds as the rows were last built; the filter shows
    /// what it finds, folds aside.
    fn open_as_built(&self, heading: &Selection) -> bool {
        let fold = Fold::Under(heading.clone());
        self.folds_seen.get(&fold).copied().unwrap_or(true)
    }

    /// The terminals of the watched projects' worktrees for a move of one among them to start
    /// from: as the move of one under way started, which the rows show elsewhere, else as
    /// they're listed now (see [`terminal_lists`]).
    pub(super) fn listed_terminals(
        &self,
        store: &Store,
    ) -> Arc<[(ProjectCheckout, Vec<TerminalId>)]> {
        if let Some((_, lists)) = moving_terminal(store) {
            return lists.clone();
        }
        let open = |heading: &Selection| self.open_as_built(heading);
        terminal_lists(self.shown_rows(), open).into()
    }

    /// The stops of the move under way, which of them it's at, and which it started from:
    /// where what it moves is, also where it's at if the one it was at is gone, as when it's
    /// folded away.
    fn move_stops(&self, cx: &App) -> Option<(Vec<Stop>, usize, usize)> {
        let moving = self.store.read(cx).moving.as_ref()?;
        let (stops, start) = self.stops_for(&moving.project, &moving.what, moving.op, cx)?;
        let at = (stops.iter().position(|stop| *stop == moving.at)).unwrap_or(start);
        Some((stops, at, start))
    }

    /// Start moving the selected commit, with those picked with it, the selected branch, the
    /// selected watched project's card, linked worktree or terminal, from where it is. The keys that step through the rows
    /// step through where it can land until Return lands it or Escape lets go. While it's
    /// being squashed, move it instead.
    pub(super) fn start_move(
        &mut self,
        _: &MoveSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        if let Some(moving) = store.moving.clone() {
            return self.switch(moving, Op::Move, cx);
        }
        let (project, what) = match store.selection.clone() {
            // Among the cards as they're listed now, whatever order lists them.
            Some(Selection::Project(project))
                if store.project(&project).is_some_and(|p| p.expanded) =>
            {
                let order: Arc<[ProjectId]> = (self.watched(store).into_iter())
                    .filter(|entry| {
                        store.prefs.project_group(&entry.id) == store.prefs.project_group(&project)
                    })
                    .map(|e| e.id.clone())
                    .collect();
                (project, Moving::Project(order))
            }
            // Among the linked worktrees listed with it now, active or inactive.
            Some(Selection::Checkout(project, Checkout::Linked(name))) => {
                let Some(order) = listed_with(store, &project, name.as_ref()) else {
                    return;
                };
                (project, Moving::Worktree { name, order })
            }
            // Among the terminals of the watched projects' worktrees as they're listed now.
            Some(Selection::Terminal(project, id)) => {
                let lists = self.listed_terminals(store);
                (project, Moving::Terminal { id, lists })
            }
            _ => match commits_selected(store) {
                Some(selected) => selected,
                None => return,
            },
        };
        // Somewhere besides where it is.
        let op = Op::Move;
        let start = (self.stops_for(&project, &what, op, cx))
            .filter(|(stops, _)| stops.len() > 1)
            .and_then(|(stops, start)| stops.into_iter().nth(start));
        let Some(at) = start else {
            return snack(&self.store, "Nowhere to move it", cx);
        };
        let moving = Move {
            project,
            what,
            at,
            op,
        };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        self.take_keys(window, cx);
    }

    /// Start squashing the selected commit, with those picked with it, or the selected branch,
    /// from its own row, as the TUI's squash mode does: the keys step through what it can go
    /// into until Return squashes it there or Escape lets go. While it's being moved, squash it
    /// instead. The selected worktree's uncommitted changes start amending, as Amend does.
    pub(super) fn start_squash(
        &mut self,
        _: &SquashSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        if let Some(moving) = store.moving.clone() {
            return self.switch(moving, Op::Squash(Messages::default()), cx);
        }
        let (project, what) = match store.selection.clone() {
            Some(Selection::Uncommitted(..)) => {
                return window.dispatch_action(Box::new(StartAmend), cx);
            }
            _ => match commits_selected(store) {
                Some(selected) => selected,
                None => return,
            },
        };
        self.squash(project, what, None, window, cx);
    }

    /// Start amending the selected commit, or the selected branch's newest commit, with the
    /// uncommitted changes of its worktree, as the TUI's ⇧R does.
    pub(super) fn start_squash_into(
        &mut self,
        _: &SquashIntoSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            self.store.read(cx).selection,
            Some(Selection::Commit(..) | Selection::Branch(_, Some(_)))
        ) {
            window.dispatch_action(Box::new(StartAmend), cx);
        }
    }

    /// Where amending started from `from` in `project` starts: the commit right under a commit
    /// being placed, which picking a branch for it moves, the selected commit, or the selected
    /// branch's newest; `None` from the changes' own row, which starts at the top of the branch
    /// below it ([`Self::squash`]). A folded branch stands in for its newest commit, out of
    /// sight.
    pub(crate) fn amend_start(
        &self,
        project: &ProjectId,
        from: &Selection,
        cx: &App,
    ) -> Option<Stop> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project)?;
        let placed = (store.moving.as_ref())
            .filter(|moving| moving.op == Op::Commit && moving.project == *project);
        let into = match (placed, from) {
            (Some(moving), _) => under_commit_at(&moving.at, workspace)?,
            (None, Selection::Commit(_, id)) => *id,
            (None, Selection::Branch(_, Some(name))) => {
                workspace.find_branch(Some(name))?.commits.first()?.id
            }
            (None, _) => return None,
        };
        let name = workspace.find_commit(into)?.0.ref_name.clone()?;
        let row = Selection::Branch(project.clone(), Some(name.clone()));
        Some(Stop::Into(match self.open_as_built(&row) {
            true => Target::Commit(into),
            false => Target::Branch(name),
        }))
    }

    /// Where a commit started from `from` in `project` is placed first: on top of the selected
    /// branch, or right above the selected commit; `None` from the changes' own row, which
    /// places it on top of the branch below it ([`Self::start_committing`]). The branch shown
    /// open stays open as the selection leaves it for the changes', so the commit shows where
    /// it's placed.
    pub(crate) fn commit_start(
        &mut self,
        project: &ProjectId,
        from: &Selection,
        cx: &mut Context<Self>,
    ) -> Option<Stop> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project)?;
        let (branch, start) = match from {
            Selection::Branch(_, Some(name)) => {
                (name.clone(), Stop::In(Destination::Branch(name.clone())))
            }
            Selection::Commit(_, id) => {
                let (branch, _) = workspace.find_commit(*id)?;
                let name = branch.ref_name.clone()?;
                let at = branch.commits.iter().position(|commit| commit.id == *id)?;
                let above = at
                    .checked_sub(1)
                    .and_then(|above| branch.commits.get(above));
                let start = match above {
                    Some(above) => Stop::In(Destination::BelowCommit(above.id)),
                    None => Stop::In(Destination::Branch(name.clone())),
                };
                (name, start)
            }
            _ => return None,
        };
        let row = Selection::Branch(project.clone(), Some(branch.clone()));
        if self.open_as_built(&row) {
            self.set_open(
                super::ClickOpened::Here((project.clone(), Some(branch))),
                true,
                cx,
            );
        }
        Some(start)
    }

    /// Start placing a commit of `worktree`'s uncommitted changes in `project`, from `start`
    /// if it can go there, else on top of the topmost branch of `worktree`, as the TUI's commit
    /// mode does: the keys step through where it can go until Return commits it there, its
    /// message written first, or Escape lets go. The composer shows what goes in, and the
    /// message. So c and Return commit on top of the branch below the changes. The keys come
    /// here to place it. Whether it started.
    pub(crate) fn start_committing(
        &mut self,
        project: ProjectId,
        worktree: Checkout,
        start: Option<Stop>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let files = (self.detail.read(cx)).files_to_move(&project, worktree.clone(), cx);
        let what = Moving::Files(Changes {
            from: None,
            worktree: worktree.clone(),
            files: files.into(),
        });
        let stops = (self.stops_for(&project, &what, Op::Commit, cx))
            .map(|(stops, _)| stops)
            .unwrap_or_default();
        // Its own checkout's topmost branch, else a new branch of its own: another checkout's
        // branches take it only when stepped to.
        let workspace = self.store.read(cx).workspace(&project);
        let own = |branch: &FullName| {
            (workspace.and_then(|ws| ws.lane_of(Some(branch))))
                .is_some_and(|(lane, _)| lane.checkout == worktree)
        };
        let topmost = (stops.iter())
            .position(|stop| matches!(stop, Stop::In(Destination::Branch(b)) if own(b)))
            .or_else(|| (stops.iter()).position(|stop| matches!(stop, Stop::NewStack { .. })))
            .unwrap_or_default();
        let at = (start.filter(|start| stops.contains(start)))
            .or_else(|| stops.into_iter().nth(topmost));
        let Some(at) = at else {
            snack(&self.store, "Nowhere to commit to", cx);
            return false;
        };
        let moving = Move {
            project,
            what,
            at,
            op: Op::Commit,
        };
        self.reveal_selection = true;
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        self.take_keys(window, cx);
        true
    }

    /// Start placing `worktree`'s uncommitted changes in `project` from `start`, as a commit or
    /// with `amend`, into one. Whether it started.
    pub(crate) fn start_placing(
        &mut self,
        amend: bool,
        project: ProjectId,
        worktree: Checkout,
        start: Option<Stop>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match amend {
            true => self.start_amending(project, worktree, start, window, cx),
            false => self.start_committing(project, worktree, start, window, cx),
        }
    }

    /// Why `worktree`'s changes in `project` can't amend anything started from `from`, if they
    /// can't: there are none, or nothing can take them. None being checked is fine, as an amend
    /// of nothing still changes the message.
    pub(crate) fn amend_refused(
        &self,
        project: &ProjectId,
        worktree: &Checkout,
        from: &Selection,
        cx: &App,
    ) -> Option<&'static str> {
        let uncommitted = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.uncommitted_in(worktree))
            .is_some_and(|files| !files.is_empty());
        if !uncommitted {
            return Some("Nothing uncommitted to amend");
        }
        let Some(what) = self.amended_changes(project, worktree.clone(), cx) else {
            return Some("Nothing to amend with");
        };
        let start = self.amend_start(project, from, cx);
        (self.squash_move(project, &what, start, cx))
            .is_none()
            .then_some("Nowhere to amend it")
    }

    /// Start amending a commit with `worktree`'s uncommitted changes in `project`, from `start`
    /// if they can go there: the keys step through what they can go into until Return amends
    /// it, or Escape lets go. With the composer open, it shows the commit's message and what
    /// goes in. Whether it started.
    pub(crate) fn start_amending(
        &mut self,
        project: ProjectId,
        worktree: Checkout,
        start: Option<Stop>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(what) = self.amended_changes(&project, worktree, cx) else {
            return false;
        };
        // The commit's branch may only be open for the selection, which has left it for the
        // changes: it stays open, for the commit to be outlined.
        if let Some(Stop::Into(Target::Commit(id))) = &start {
            let name = (self.store.read(cx).workspace(&project))
                .and_then(|workspace| workspace.find_commit(*id))
                .and_then(|(branch, _)| branch.ref_name.clone());
            if let Some(name) = name {
                self.set_open(
                    super::ClickOpened::Here((project.clone(), Some(name))),
                    true,
                    cx,
                );
            }
        }
        self.reveal_selection = true;
        self.reveal_source = true;
        self.squash(project, what, start, window, cx)
    }

    /// What an amend of `worktree`'s uncommitted changes holds: none while the composer's open
    /// on them, as the checks can change till it lands and a file going from what's held would
    /// end it. Otherwise them all but conflicted ones, `None` when that leaves none.
    fn amended_changes(&self, project: &ProjectId, worktree: Checkout, cx: &App) -> Option<Moving> {
        let detail = self.detail.read(cx);
        let composing = detail.composing_in(project, &worktree);
        let files = match composing {
            true => Vec::new(),
            false => detail.files_to_move(project, worktree.clone(), cx),
        };
        (composing || !files.is_empty()).then(|| {
            Moving::Files(Changes {
                from: None,
                worktree,
                files: files.into(),
            })
        })
    }

    /// The squash of `what` of `project` from the selection's row, which holds it, at `at` if
    /// it can go there. `None` with nowhere to go: somewhere besides where it is, unless it goes
    /// somewhere there, as a branch squashing into one commit does.
    fn squash_move(
        &self,
        project: &ProjectId,
        what: &Moving,
        at: Option<Stop>,
        cx: &App,
    ) -> Option<Move> {
        let op = Op::Squash(Messages::default());
        let (stops, start) = self.stops_for(project, what, op, cx)?;
        // Uncommitted changes go first into what's on top of the branch below their row, as
        // a commit of them goes there first.
        let start = match what {
            Moving::Files(Changes { from: None, .. }) => (stops.iter().enumerate().skip(start + 1))
                .find(|(_, stop)| matches!(stop, Stop::Into(Target::Commit(_) | Target::Branch(_))))
                .map_or(start, |(at, _)| at),
            Moving::Commits(_)
            | Moving::Branch(_)
            | Moving::Project(_)
            | Moving::Worktree { .. }
            | Moving::Terminal { .. }
            | Moving::Files(_)
            | Moving::Copies(_) => start,
        };
        let start = (at.as_ref())
            .and_then(|at| stops.iter().position(|stop| stop == at))
            .unwrap_or(start);
        let moving = Move {
            project: project.clone(),
            what: what.clone(),
            // With its own row out of sight, as a commit's is in a folded branch or past the
            // commits a branch lists, it starts at the first place it can go.
            at: stops.get(start).or(stops.first())?.clone(),
            op,
        };
        (stops.len() > 1 || moving.lands(self.store.read(cx))).then_some(moving)
    }

    /// Start squashing `what` of `project` from the selection's row, which holds it, at `at`
    /// if it can go there. Whether it started.
    pub(crate) fn squash(
        &mut self,
        project: ProjectId,
        what: Moving,
        at: Option<Stop>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(moving) = self.squash_move(&project, &what, at, cx) else {
            let why = match what {
                Moving::Files(Changes { from: None, .. }) => "Nowhere to amend it",
                Moving::Commits(_)
                | Moving::Branch(_)
                | Moving::Project(_)
                | Moving::Worktree { .. }
                | Moving::Terminal { .. }
                | Moving::Files(_)
                | Moving::Copies(_) => "Nowhere to squash it",
            };
            snack(&self.store, why, cx);
            return false;
        };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        self.take_keys(window, cx);
        true
    }

    /// Start cherry-picking the selected commit, with those picked with it, or the commit a
    /// branch's remote has that it lists, as the TUI's p does: the keys step through where
    /// copies of them can go until Return puts them there or Escape lets go. While commits are
    /// being moved or squashed, cherry-pick them instead.
    pub(crate) fn start_cherry_pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        if let Some(moving) = store.moving.clone() {
            return self.switch(moving, Op::CherryPick, cx);
        }
        let (project, what) = match store.selection.clone() {
            Some(Selection::Commit(project, id)) => {
                let ids = store.with_picks(&project, id);
                (project, Moving::Commits(ids))
            }
            Some(Selection::Upstream(project, _, id)) => {
                let remote = (store.workspace(&project))
                    .and_then(|ws| ws.remote_commits().find(|commit| commit.id == id));
                let Some(remote) = remote else {
                    return;
                };
                // As `git cherry-pick` refuses: which side to take isn't said.
                if remote.merge {
                    return snack(&self.store, "A merge can't be cherry-picked", cx);
                }
                let copies = [(id, remote.title.clone().into())].into();
                (project, Moving::Copies(copies))
            }
            _ => return,
        };
        self.cherry_pick(project, what, window, cx)
    }

    /// Start cherry-picking `what` into `project`: the workspace's commits from where they
    /// are, and copies of others from the top of the topmost branch.
    pub(crate) fn cherry_pick(
        &mut self,
        project: ProjectId,
        what: Moving,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let op = Op::CherryPick;
        let start = (self.stops_for(&project, &what, op, cx))
            .and_then(|(stops, start)| stops.into_iter().nth(start));
        let Some(at) = start else {
            return snack(&self.store, "Nowhere to cherry-pick it", cx);
        };
        let moving = Move {
            project,
            what,
            at,
            op,
        };
        self.reveal_selection = true;
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
        self.take_keys(window, cx);
    }

    /// Squash what `moving` moves, or move or cherry-pick what it squashes, as `op` says: from
    /// what matches where it's at, below a commit or into it, which show where it goes in the
    /// same place, or the same stop, where moving and cherry-picking both land, else from where
    /// it is.
    fn switch(&mut self, moving: Move, op: Op, cx: &mut Context<Self>) {
        let same_op = matches!(
            (moving.op, op),
            (Op::Move, Op::Move)
                | (Op::Squash(_), Op::Squash(_))
                | (Op::CherryPick, Op::CherryPick)
        );
        // A drag's op is where the pointer holds it, so the keys don't switch it.
        if same_op || !moving.of_commits() || moving.dragged(self.store.read(cx)) {
            return;
        }
        let Move {
            project, what, at, ..
        } = moving;
        let Some((stops, start)) = self.stops_for(&project, &what, op, cx) else {
            return;
        };
        let same = match (&at, op) {
            (Stop::In(Destination::BelowCommit(id)), Op::Squash(_)) => {
                Some(Stop::Into(Target::Commit(*id)))
            }
            (Stop::Into(Target::Commit(id)), Op::Move | Op::CherryPick) => {
                Some(Stop::In(Destination::BelowCommit(*id)))
            }
            (_, Op::Move | Op::CherryPick) => Some(at.clone()),
            (_, Op::Squash(_) | Op::Commit) => None,
        };
        let Some(at) =
            (same.filter(|stop| stops.contains(stop))).or_else(|| stops.into_iter().nth(start))
        else {
            return;
        };
        self.reveal_selection = true;
        let moving = Move {
            project,
            what,
            at,
            op,
        };
        self.store
            .update(cx, |store, cx| store.set_moving(Some(moving), cx));
    }

    /// Keep only the message of what a squash goes into, or both again, as the TUI's u does.
    pub(super) fn use_target_message(
        &mut self,
        _: &UseTargetMessage,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.store.update(cx, |store, cx| {
            if let Some(Move {
                op: Op::Squash(keep),
                ..
            }) = &mut store.moving
            {
                *keep = match keep {
                    Messages::Both => Messages::Target,
                    Messages::Target => Messages::Both,
                };
                cx.notify();
            }
        });
    }

    /// Step the move `delta` places on, stopping at either end.
    pub(super) fn step_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some((stops, at, _)) = self.move_stops(cx) else {
            return;
        };
        let last = stops.len().saturating_sub(1) as isize;
        let next = (at as isize).saturating_add(delta).clamp(0, last) as usize;
        self.move_to(stops, next, cx);
    }

    /// Jump the move to the top of the next branch, or with `forward` false the one before, or
    /// to a new stack, as ⇧J and ⇧K jump between sections. A branch, which lands only between
    /// branches, jumps between the tops of stacks, and a card from card to card, as it steps.
    pub(super) fn jump_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some((stops, at, _)) = self.move_stops(cx) else {
            return;
        };
        let store = self.store.read(cx);
        let workspace = (store.moving.as_ref()).and_then(|moving| store.workspace(&moving.project));
        let tops = |beside: &FullName| {
            workspace
                .and_then(|ws| ws.lane_of(Some(beside)))
                .is_some_and(|(_, at)| at == 0)
        };
        // A branch's newest commit, which a squash goes into for it while it's open.
        let newest = |id: &CommitId| {
            (workspace.and_then(|ws| ws.find_commit(*id)))
                .is_some_and(|(branch, _)| branch.commits.first().map(|c| c.id) == Some(*id))
        };
        let heads = |(_, stop): &(usize, &Stop)| match stop {
            Stop::In(Destination::Branch(_))
            | Stop::NewStack { .. }
            | Stop::Card(_)
            | Stop::Worktree(_)
            | Stop::Terminal { .. }
            | Stop::Into(Target::Branch(_) | Target::Uncommitted(_)) => true,
            Stop::Into(Target::Commit(id)) => newest(id),
            Stop::NewBranch {
                beside,
                below: false,
            } => tops(beside),
            Stop::In(_) | Stop::NewBranch { .. } => false,
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

    /// Put the move under way where a click on `item`'s row says, as [`clicked_stop`] does,
    /// as though the keys had stepped it there. A click it can't use leaves it where it is,
    /// which blinks to say it's still on: a click on a row does nothing else while it's under
    /// way, so a stray one never ends it.
    pub(super) fn click_move(
        &mut self,
        item: &Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.take_keys(window, cx);
        let store = self.store.read(cx);
        let Some(moving) = store.moving.as_ref() else {
            return;
        };
        let clicked = self.move_stops(cx).and_then(|(stops, at, start)| {
            let own = matches!(mark(store, item), Some(Mark::Taken(_) | Mark::Copied(_)));
            // The project's row lands as the main checkout's heading it stands in for.
            let item = match item {
                Selection::Project(p) if !store.main_headed(p) => {
                    &Selection::Checkout(p.clone(), Checkout::Main)
                }
                _ => item,
            };
            let checkout = store.checkout_of(item);
            let ix = clicked_stop(&stops, (at, start), moving, item, own, checkout)?;
            Some((stops, ix))
        });
        match clicked {
            Some((stops, ix)) => self.move_to(stops, ix, cx),
            None => self.nudge_move(cx),
        }
    }

    /// Land what's being moved where the move is at, as dropping it there would, or squash it
    /// there.
    pub(super) fn move_here(&mut self, _: &MoveHere, window: &mut Window, cx: &mut Context<Self>) {
        let Some(moving) = self.store.read(cx).moving.as_ref() else {
            return;
        };
        // A commit's made as ⌘↩ makes it: from the composer, its message written first, or
        // as it is without one, where the navigator came out for it. An amend, by Amend.
        if moving.op == Op::Commit {
            return window.dispatch_action(Box::new(crate::app::StartCommit), cx);
        }
        if moving.amending() {
            return window.dispatch_action(Box::new(StartAmend), cx);
        }
        self.land_move(window, cx)
    }

    /// Land what's being moved where the move is at, but a commit, which the composer makes.
    pub(crate) fn land_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(moving) = store.moving.clone() else {
            return;
        };
        let lands = moving.lands(store);
        let squashed = moving.squashed(store);
        let dragged = moving.dragged(store);
        self.store
            .update(cx, |store, cx| store.set_moving(None, cx));
        // Where they already are, there's nothing to do.
        if !lands {
            return;
        }
        // On what it goes into, as the TUI ends there: the commit, a branch squashed into one
        // too, or the uncommitted changes. It's selected before what goes is hidden, which
        // would leave a selected commit be, so what goes fades as the rows close over it.
        if let (Some(transfer), Stop::Into(target)) = (squashed, &moving.at) {
            let project = moving.project.clone();
            let on = match &transfer {
                Transfer::Squash { into, .. }
                | Transfer::Amend { into, .. }
                | Transfer::MoveFiles { to: into, .. } => Selection::Commit(project.clone(), *into),
                Transfer::Uncommit { .. }
                | Transfer::UncommitFiles { .. }
                | Transfer::MoveCommit { .. }
                | Transfer::CherryPick { .. }
                | Transfer::StackBranch { .. }
                | Transfer::BranchBelow { .. }
                | Transfer::StageOnBranch { .. }
                | Transfer::CommitOnBranch { .. } => target.row(&project),
            };
            return self.store.update(cx, |store, cx| {
                store.select(Some(on), cx);
                // Only staging comes back, which a squash never is.
                let _ = transfer.perform(&project, None, store, cx);
            });
        }
        // A card, a worktree or a terminal, already shown where it goes, goes there in the
        // order they keep.
        if let Some(order) = moving.project_order() {
            // A group's keyboard move orders only its members. Leave the other groups'
            // repositories in their existing order.
            let mut reordered = order.iter();
            let all: Vec<_> = self
                .store
                .read(cx)
                .expanded_ids()
                .into_iter()
                .map(|id| {
                    if order.contains(&id) {
                        reordered.next().cloned().unwrap_or(id)
                    } else {
                        id
                    }
                })
                .collect();
            return self
                .store
                .update(cx, |store, cx| store.order_watched(&all, cx));
        }
        if let (Moving::Worktree { name, .. }, Stop::Worktree(slot)) = (&moving.what, &moving.at) {
            let before = match slot {
                Slot::Before(before) => Some(before.as_ref()),
                Slot::End => None,
            };
            return (self.store).update(cx, |store, cx| {
                store.move_worktree(&moving.project, name.as_ref(), before, cx)
            });
        }
        if let Some(order) = moving.terminal_order() {
            return (self.store).update(cx, |store, cx| store.order_terminals(&order, cx));
        }
        // Copies go where it's at, the commits staying where they are, and show there till
        // they're there.
        if let (Op::CherryPick, Some(ids)) = (moving.op, moving.picked()) {
            let (project, at, lands) = (moving.project.clone(), moving.at.clone(), moving);
            return self.store.update(cx, |store, cx| match at {
                Stop::In(to) => {
                    // Only staging comes back, which a cherry-pick never is.
                    let transfer = Transfer::CherryPick { ids, to };
                    let _ = transfer.perform(&project, Some(lands), store, cx);
                }
                Stop::NewBranch { beside, below } => {
                    let beside = Some((beside, below));
                    store.cherry_pick_to_new_branch(&project, ids, beside, None, lands, cx)
                }
                Stop::NewStack { before } => {
                    store.cherry_pick_to_new_branch(&project, ids, None, before, lands, cx)
                }
                Stop::Into(_) | Stop::Card(_) | Stop::Worktree(_) | Stop::Terminal { .. } => {}
            });
        }
        // A branch's copy stays where it lands till it's there. What's dragged has no copy out,
        // and one coming now would leave its row to go only once it's there, so it lands as
        // anything dropped does, leaving where it was as it shows where it goes.
        let lands = (!dragged).then(|| moving.clone());
        if let Some((target, place)) = moving.drop() {
            let what = moving.what.clone();
            return self.app.update_later(window, cx, move |app, window, cx| {
                app.land(&what, &target, place, lands, window, cx)
            });
        }
        // With no row to show them in till it's made, commits dragged to a new branch stay where
        // they are till the reload slides them there, selected for it to open with them in it.
        let overlay = Overlay {
            lands: Some(moving.clone()),
            dropped: dragged,
            ..Overlay::default()
        };
        let Move {
            project, what, at, ..
        } = moving;
        let select = |store: &mut Store, commits: &[CommitId], cx: &mut Context<Store>| {
            if let Some(id) = commits.first().filter(|_| dragged) {
                store.select(Some(Selection::Commit(project.clone(), *id)), cx);
            }
        };
        self.store.update(cx, |store, cx| match (what, at) {
            (Moving::Commits(commits), Stop::NewBranch { beside, below }) => {
                select(store, &commits, cx);
                let beside = Some((beside, below));
                store.move_to_new_branch(&project, commits, beside, None, overlay, cx)
            }
            (Moving::Commits(commits), Stop::NewStack { before }) => {
                select(store, &commits, cx);
                store.move_to_new_branch(&project, commits, None, before, overlay, cx)
            }
            (Moving::Branch(branch), Stop::NewStack { before }) => {
                store.unstack(&project, branch, before, lands, cx)
            }
            // Those land as drops do, cards in their order, and squashes, above.
            (
                Moving::Commits(_),
                Stop::In(_)
                | Stop::Card(_)
                | Stop::Worktree(_)
                | Stop::Terminal { .. }
                | Stop::Into(_),
            )
            | (
                Moving::Branch(_)
                | Moving::Project(_)
                | Moving::Worktree { .. }
                | Moving::Terminal { .. }
                | Moving::Files(_)
                | Moving::Copies(_),
                _,
            ) => {}
        });
    }
}

/// The selected commit, with those picked with it, or the selected branch, as what moves or
/// squashes, in its project.
fn commits_selected(store: &Store) -> Option<(ProjectId, Moving)> {
    match store.selection.clone()? {
        Selection::Commit(project, id) => {
            let commits = store.with_picks(&project, id);
            Some((project, Moving::Commits(commits)))
        }
        Selection::Branch(project, Some(branch)) => Some((project, Moving::Branch(branch))),
        Selection::Branch(_, None)
        | Selection::Project(_)
        | Selection::Checkout(..)
        | Selection::Uncommitted(..)
        | Selection::Stack(..)
        | Selection::Upstream(..)
        | Selection::Section(..)
        | Selection::Terminal(..)
        | Selection::Pending(..)
        | Selection::Mesh(..) => None,
    }
}

/// The terminals `rows` list under each worktree's heading, the main one's, or the project's
/// row standing in for it, and the linked ones', of every project, in the order they're listed:
/// those whose heading `open` says shows what it holds, as a terminal landing under a folded one
/// would be out of sight.
fn terminal_lists<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    open: impl Fn(&Selection) -> bool,
) -> Vec<(ProjectCheckout, Vec<TerminalId>)> {
    let mut lists: Vec<(ProjectCheckout, Vec<TerminalId>)> = Vec::new();
    // Under a heading that's open now.
    let mut under = false;
    let mut rows = rows.into_iter().peekable();
    while let Some(row) = rows.next() {
        let Row::Item(item) = row else {
            continue;
        };
        if let Selection::Terminal(p, id) = item {
            if under
                && let Some((on, list)) = lists.last_mut()
                && on.project == *p
            {
                list.push(*id);
            }
        } else if let Some(checkout) = heads(row, rows.peek().copied()) {
            under = open(item);
            if under {
                let project = item.project().clone();
                lists.push((ProjectCheckout { project, checkout }, Vec::new()));
            }
        }
    }
    lists
}

/// Whether `item`'s row is what's being moved where it goes where it'd land itself, rather than
/// show a copy there: a project's card, by its heading, a linked worktree or a terminal.
pub(super) fn goes_itself(store: &Store, item: &Selection) -> bool {
    store.moving.as_ref().is_some_and(|moving| {
        matches!(
            moving.what,
            Moving::Project(_) | Moving::Worktree { .. } | Moving::Terminal { .. }
        ) && moving.holds(item)
    })
}

/// Which of `stops` a click on `item`'s row puts `moving` at, `at` being the one it's at and
/// `start` where it started, or `None` for a row it has no place at. `own` says the row is what
/// moves, or of it, as a branch's commits are, which takes it back to where it started, but a
/// commit being made, which started nowhere. `checkout` is where the row is.
///
/// Commits land under the commit clicked, or at the top of the branch clicked, folded or not; a
/// branch lands above the one clicked; what's squashed goes into the row clicked.
/// A project's card, a linked worktree or a terminal takes the place of the one clicked, as
/// dragging it there does, or of the card or worktree a row clicked is in; a terminal goes last
/// in the worktree clicked.
fn clicked_stop(
    stops: &[Stop],
    (at, start): (usize, usize),
    moving: &Move,
    item: &Selection,
    own: bool,
    checkout: Option<Checkout>,
) -> Option<usize> {
    let find = |stop: Stop| stops.iter().position(|other| *other == stop);
    // Before the one clicked coming from below it, after it coming from above.
    let instead_of = |stop: Stop| find(stop).map(|ix| if ix < at { ix } else { ix + 1 });
    let project = item.project();
    match &moving.what {
        Moving::Project(_) if *project == moving.project => Some(at),
        Moving::Project(_) => instead_of(Stop::Card(Slot::Before(project.clone()))),
        Moving::Terminal { id, .. } => {
            let checkout = || {
                let (project, checkout) = (project.clone(), checkout.clone()?);
                Some(ProjectCheckout { project, checkout })
            };
            match item {
                Selection::Terminal(_, other) if other == id => Some(at),
                Selection::Terminal(_, other) => instead_of(Stop::Terminal {
                    checkout: checkout()?,
                    slot: Slot::Before(*other),
                }),
                _ => find(Stop::Terminal {
                    checkout: checkout()?,
                    slot: Slot::End,
                }),
            }
        }
        _ if *project != moving.project => None,
        Moving::Worktree { name, .. } => match checkout? {
            Checkout::Linked(other) if other == *name => Some(at),
            Checkout::Linked(other) => instead_of(Stop::Worktree(Slot::Before(other))),
            Checkout::Main => None,
        },
        Moving::Commits(_) | Moving::Branch(_) | Moving::Files(_) | Moving::Copies(_) => {
            if own {
                return (moving.op != Op::Commit).then_some(start);
            }
            match (moving.op, item) {
                (Op::Squash(_), Selection::Checkout(_, checkout)) => {
                    find(Stop::Into(Target::Uncommitted(checkout.clone())))
                }
                (Op::Squash(_), _) => find(Stop::Into(Target::of(item)?)),
                (Op::Move | Op::CherryPick | Op::Commit, Selection::Commit(_, id)) => {
                    find(Stop::In(Destination::BelowCommit(*id)))
                }
                (Op::Move | Op::CherryPick | Op::Commit, Selection::Branch(_, Some(name))) => {
                    find(Stop::In(Destination::Branch(name.clone()))).or_else(|| {
                        find(Stop::NewBranch {
                            beside: name.clone(),
                            below: false,
                        })
                    })
                }
                (Op::Move | Op::CherryPick | Op::Commit, _) => None,
            }
        }
    }
}

/// The stops of `all` a branch lands at, between branches, as a new branch would, and where it
/// is: the place above it, and the place below it, just as where it is, no stop. `lands` says
/// whether landing at a stop moves it, as it doesn't where it already is.
fn branch_stops(
    all: Vec<Stop>,
    branch: &FullName,
    lands: impl Fn(&Stop) -> bool,
) -> Option<(Vec<Stop>, usize)> {
    let home = Stop::NewBranch {
        beside: branch.clone(),
        below: false,
    };
    let stops: Vec<Stop> = (all.into_iter())
        .filter(|stop| match stop {
            _ if *stop == home => true,
            Stop::NewBranch { .. } | Stop::NewStack { .. } => lands(stop),
            Stop::In(_)
            | Stop::Card(_)
            | Stop::Worktree(_)
            | Stop::Terminal { .. }
            | Stop::Into(_) => false,
        })
        .collect();
    let start = stops.iter().position(|stop| *stop == home)?;
    Some((stops, start))
}

/// The title of commit `id` that the cherry-pick shown in `project` copies from outside the
/// workspace, if it's one of those.
fn copied_title(store: &Store, project: &ProjectId, id: CommitId) -> Option<SharedString> {
    let Moving::Copies(copies) = &shown(store, project)?.what else {
        return None;
    };
    let (_, title) = copies.iter().find(|(copy, _)| *copy == id)?;
    Some(title.clone())
}

/// A copy of `item`'s row where what's moving would land, set in by `inset`, outlined as one
/// with the copies above and below as `outlined_with` says, and blinking, as a click the move
/// can't use reminds the user of it, by which `blink` it's in.
pub(super) fn ghost_row(
    item: &Selection,
    inset: f32,
    outlined_with: (bool, bool),
    blink: Option<usize>,
    store: &Store,
    theme: &Theme,
) -> AnyElement {
    // In the colour of what's being done with it, as what it's of is marked.
    let style = RowStyle {
        held: true,
        mark: mark_of(store, item.project()),
        outlined_with,
        blink,
        ..RowStyle::plain(theme, inset)
    };
    let row = match item {
        Selection::Commit(project, id) => {
            let row = format!("ghost-{project}-{}", id.0);
            match store.find_commit(project, *id) {
                Some((_, commit)) => Some(style.commit(row, project, commit, store, None)),
                // A copy of a commit from outside the workspace, as the commit it'd be.
                None => copied_title(store, project, *id).map(|title| style.copy(row, title)),
            }
            .map(IntoElement::into_any_element)
        }
        Selection::Branch(project, ref_name) => (store.workspace(project))
            .and_then(|ws| ws.find_branch(ref_name.as_ref()))
            .map(|branch| {
                let id = format!("ghost-{project}-{}", branch.name);
                let name = store.name_of(project, branch).to_owned();
                branch_copy(id, name, style)
            }),
        _ => None,
    };
    row.unwrap_or_else(|| div().h(dp(theme.sizes.control)).into_any_element())
}

/// The commit uncommitted changes being committed in `project` would make, where it'd go,
/// set in by `inset` as a commit row is: titled as its message is written, outlined in the
/// colour of committing, as copies of what's moving are in theirs, and blinking as they do.
pub(super) fn new_commit_row(
    project: &ProjectId,
    inset: f32,
    blink: Option<usize>,
    store: &Store,
    theme: &Theme,
) -> AnyElement {
    let style = RowStyle {
        held: true,
        mark: Op::Commit.hue(),
        blink,
        ..RowStyle::plain(theme, inset)
    };
    let title = shown(store, project).and_then(|moving| moving.commit_title(store));
    // Untitled till its message is written, quiet as an empty title is.
    let title = match title {
        Some(title) => div().child(title.to_owned()),
        None => div().text_color(style.muted()).child("New commit"),
    };
    (style.row(format!("new-commit-{project}"), COMMIT_INDENT))
        .gap(dp(10.))
        .child(small_icon(Icon::Commit).text_color(style.meta(theme.blue)))
        .child(title.flex_1().min_w_0().truncate())
        .into_any_element()
}

/// Where what's moving lands outlined over its row, as dropping on it outlines it but in the
/// `colour` of what's done with it, its top and bottom `edges` from the row's, as
/// [`SquashOutline::edges`] says, over the rows it passes, blinking as copies of what's moving
/// do.
pub(super) fn squash_outline(
    edges: (f32, f32),
    blink: Option<usize>,
    hue: Hue,
    theme: &Theme,
) -> Div {
    let (top, bottom) = edges;
    let (shape, colour) = (theme.shape, theme.hue(hue));
    div()
        .absolute()
        .top(dp(top))
        .bottom(dp(-bottom))
        .left(dp(3.))
        .right(dp(3.))
        .rounded(dp(shape.control))
        .border_1()
        .border_color(colour)
        .bg(theme.wash(hue))
        .when_some(blink, |el, n| {
            let shape = div().absolute().inset_0().rounded(dp(shape.control));
            el.child(blink_once(("blink", n), shape, colour))
        })
}

/// Branch `name`'s row as a copy draws it, row `id`, in `style`: folded, as its own row is,
/// and nothing on it acts.
fn branch_copy(id: String, name: String, style: RowStyle) -> AnyElement {
    (style.row(id, 6.))
        .child(icon(Icon::Branch).text_color(style.muted()))
        .child(div().flex_1().min_w_0().truncate().child(name))
        .into_any_element()
}

/// What a move took away, drawn where it was below the row drawing it, as it was: inside the
/// card's edges, which the row's slice draws a hairline in, on the card's ground with its
/// guides, and over what the card's slices stretch over, so they don't cover it, while the rows
/// after it, closing over it, go over it.
pub(super) fn stand_in(
    stand_in: &StandIn,
    (left, right): (f32, f32),
    layer: usize,
    store: &Store,
    theme: &Theme,
) -> Div {
    let StandIn {
        of,
        by,
        inset,
        guides,
        shown,
        op,
        outlined_with,
        ..
    } = stand_in;
    let style = RowStyle {
        taken: true,
        mark: op.hue(),
        outlined_with: *outlined_with,
        ..RowStyle::plain(theme, *inset)
    };
    let row = match of {
        Gone::Commit(project, commit) => {
            let id = format!("stand-in-{project}-{}", commit.id.0);
            style
                .commit(id, project, commit, store, None)
                .into_any_element()
        }
        Gone::Branch { id, name } => branch_copy(id.clone(), name.clone(), style),
        Gone::NewBranch(at) => ghost_branch_row(at, *inset, op.colour(theme), None, theme),
    };
    let guides = guides.iter().map(|at| guide(*at, theme));
    div()
        .absolute()
        .top(dp(*by))
        .left(dp(left))
        .right(dp(right))
        .px(fixed(1.))
        .child(
            on_top(
                (div().relative().bg(theme.bg_1).opacity(*shown))
                    .child(row)
                    .children(guides),
            )
            .priority(layer),
        )
}

/// The new branch commits being moved or made would land in at stop `at`, over their copies,
/// set in by `inset` as an item row is, marked in `mark`, the colour of what's doing it, and
/// blinking as they do. It names the branch it goes above or below in their stack, or says
/// it's independent, in a stack of its own.
pub(super) fn ghost_branch_row(
    at: &Stop,
    inset: f32,
    mark: Hsla,
    blink: Option<usize>,
    theme: &Theme,
) -> AnyElement {
    let beside = match at {
        Stop::NewBranch { beside, below } => {
            let side = if *below { "below" } else { "above" };
            Some((side, short_ref(beside).into_owned()))
        }
        Stop::In(_)
        | Stop::NewStack { .. }
        | Stop::Card(_)
        | Stop::Worktree(_)
        | Stop::Terminal { .. }
        | Stop::Into(_) => None,
    };
    let said = match &beside {
        Some((side, _)) => format!("New branch {side}"),
        None => "New independent branch".into(),
    };
    h_flex()
        .min_h(dp(theme.sizes.control))
        .mx(dp(3.))
        .pl(dp(8. + inset))
        .pr(dp(12.))
        .gap(dp(8.))
        .rounded(dp(theme.shape.control))
        .border_1()
        .border_dashed()
        .border_color(mark)
        .text_color(mark)
        .when_some(blink, |el, n| {
            let shape = div().absolute().inset_0().rounded(dp(theme.shape.control));
            el.relative().child(blink_once(("blink", n), shape, mark))
        })
        .child(small_icon(Icon::Branch).text_color(mark))
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
    use std::sync::Arc;

    use super::{
        Row, RowKey, branch_stops, clicked_stop, squash_stops, stops, taken_away, terminal_lists,
        under_commit_at,
    };
    use crate::backend::model::build::{branch, full, id, workspace, worktree};
    use crate::backend::{Checkout, CommitId, Destination, Messages, Workspace, short_ref};
    use crate::state::{ProjectCheckout, ProjectId, Selection};
    use crate::terminal::TerminalId;
    use crate::transfer::{Move, Moving, Op, Slot, Stop, Target, group_slots, slots};

    fn on(project: &ProjectId, checkout: Checkout) -> ProjectCheckout {
        let project = project.clone();
        ProjectCheckout { project, checkout }
    }

    /// Whether `heading` shows what it holds, as [`listed`] lists them: all but typo.
    fn open(heading: &Selection) -> bool {
        !matches!(heading, Selection::Branch(_, Some(name)) if short_ref(name) == "typo")
    }

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
                    Stop::NewStack { before: None } => "new stack".into(),
                    Stop::NewStack {
                        before: Some(before),
                    } => format!("new stack before {}", short(before)),
                    Stop::Card(Slot::Before(project)) => format!("before {project}"),
                    Stop::Worktree(Slot::Before(name)) => format!("before worktree {name}"),
                    Stop::Worktree(Slot::End) => "last worktree".into(),
                    Stop::Terminal { checkout, slot } => {
                        let on = match &checkout.checkout {
                            Checkout::Main => "main".to_owned(),
                            Checkout::Linked(name) => name.to_string(),
                        };
                        let on = format!("{}/{on}", checkout.project);
                        match slot {
                            Slot::Before(id) => format!("before terminal {} on {on}", id.0),
                            Slot::End => format!("last on {on}"),
                        }
                    }
                    Stop::Card(Slot::End) => "last".into(),
                    Stop::Into(Target::Commit(c)) => format!("into {}", commit(c)),
                    Stop::Into(Target::Branch(name)) => format!("into {}", short(name)),
                    Stop::Into(Target::Uncommitted(Checkout::Main)) => {
                        "into uncommitted on main".into()
                    }
                    Stop::Into(Target::Uncommitted(Checkout::Linked(name))) => {
                        format!("into uncommitted on {name}")
                    }
                };
                let here = if ix == start { " <" } else { "" };
                format!("{said}{here}\n")
            })
            .collect()
    }

    /// Two stacks, `ui` on `api` under their caption and `typo` alone, on target `main`, and a
    /// linked worktree `wt` on `agent`, and the rows the navigator lists for them, with a
    /// terminal open and `typo` folded.
    fn listed(project: &ProjectId) -> (Workspace, Vec<Row>) {
        let ws = Workspace {
            managed: true,
            target_branch: Some(full("refs/heads/main")),
            ..workspace(
                vec![
                    vec![branch("ui", &[3]), branch("api", &[2, 1])],
                    vec![branch("typo", &[4])],
                ],
                vec![worktree("wt", None, vec![branch("agent", &[5])])],
            )
        };
        let p = || project.clone();
        let named = |name: &str| Some(full(&format!("refs/heads/{name}")));
        let rows = vec![
            Row::Item(Selection::Project(p())),
            Row::Item(Selection::Checkout(p(), Checkout::Main)),
            Row::Item(Selection::Uncommitted(p(), Checkout::Main)),
            // Listed above the stacks, which it doesn't part from the heading either.
            Row::Item(Selection::Terminal(p(), TerminalId(1))),
            // The caption over ui and api, which a new stack goes above.
            Row::Item(Selection::Stack(p(), named("api"))),
            Row::Item(Selection::Branch(p(), named("ui"))),
            Row::Item(Selection::Commit(p(), id(3))),
            Row::Item(Selection::Branch(p(), named("api"))),
            Row::Item(Selection::Commit(p(), id(2))),
            Row::Item(Selection::Commit(p(), id(1))),
            // The mark between stacks, which parts nothing it walks.
            Row::Between(p()),
            Row::Item(Selection::Branch(p(), named("typo"))),
            Row::WorktreeDivider,
            Row::Item(Selection::Checkout(p(), Checkout::Linked("wt".into()))),
            Row::Item(Selection::Uncommitted(p(), Checkout::Linked("wt".into()))),
            Row::Item(Selection::Branch(p(), named("agent"))),
            Row::Item(Selection::Commit(p(), id(5))),
            Row::WorktreeDivider,
        ];
        (ws, rows)
    }

    #[test]
    fn a_commit_stops_between_every_branch_and_commit_but_in_what_is_folded() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (stops, start) = stops(&rows, &project, &[id(2)], false, false, &ws, open);
        // A new stack before the main checkout's stacks, a new branch above every branch of a
        // stack and below its last, each branch's top and the place below each of its commits
        // but the one moving, which starts below 3 at the top of api; then a new stack once the
        // stacks end. Folded, typo has its top but nothing below its commits, out of sight. The
        // linked worktree's lane takes new branches as a stack does.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new stack before api
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
new branch above agent
top of agent
below 5
new branch below agent

"#]]
        );
    }

    #[test]
    fn a_cherry_pick_stops_right_above_and_right_below_the_commit() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (stops, start) = stops(&rows, &project, &[id(2)], true, false, &ws, open);
        // As a move's, but the copy also goes below 2, and starts right above it, at the top
        // of api, as the original stays.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new stack before api
new branch above ui
top of ui
below 3
new branch above api
top of api <
below 2
below 1
new branch below api
new branch above typo
top of typo
new branch below typo
new stack
new branch above agent
top of agent
below 5
new branch below agent

"#]]
        );
    }

    #[test]
    fn a_folded_main_checkout_takes_nothing_and_without_a_target_no_new_stack_is_offered() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let shut = |_: &Selection| false;
        let (folded, _) = stops(
            rows.iter().take(2),
            &project,
            &[id(2)],
            false,
            false,
            &ws,
            shut,
        );
        assert!(
            folded.is_empty(),
            "nothing shows under the folded heading, and no new stack goes after it"
        );
        let untargeted = Workspace {
            managed: false,
            target_branch: None,
            ..ws
        };
        let (stops, start) = stops(&rows, &project, &[id(1)], false, false, &untargeted, open);
        // A stack of its own starts from the target, so without one there's no new stack,
        // only new branches beside the others, in every lane.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new branch above ui
top of ui
below 3
new branch above api
top of api
below 2 <
new branch below api
new branch above typo
top of typo
new branch below typo
new branch above agent
top of agent
below 5
new branch below agent

"#]]
        );
    }

    #[test]
    fn what_a_move_takes_away_stays_under_the_nearest_row_that_stays() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let branch = |name: &str| {
            let name = full(&format!("refs/heads/{name}"));
            Some(RowKey::Item(Selection::Branch(p(), Some(name))))
        };
        let commit = |n: usize| Some(RowKey::Commit(p(), n.to_string()));
        let left = |n: usize| {
            let own = RowKey::Commit(p(), n.to_string());
            Some(RowKey::Left(Box::new(own)))
        };
        let new_branch = || Some(RowKey::GhostBranch(p()));
        // Commit 3 moving from the top of ui to a new branch above api.
        let above_api = [
            branch("ui"),
            left(3),
            commit(2),
            new_branch(),
            commit(3),
            branch("api"),
        ];
        let top_of_api = [branch("ui"), left(3), commit(2), branch("api"), commit(3)];
        assert_eq!(
            taken_away(&above_api, &top_of_api, false, false),
            [(3, 2)],
            "stepping on, the new branch goes, under commit 2"
        );
        let landed = [branch("ui"), commit(2), branch("api"), commit(3)];
        assert_eq!(
            taken_away(&top_of_api, &landed, false, true),
            [(1, 0)],
            "landing, where it was left goes, under ui"
        );
        let back = [branch("ui"), commit(3), commit(2), branch("api")];
        assert_eq!(
            taken_away(&above_api, &back, false, false),
            [(3, 2)],
            "letting go, the new branch goes, and the commit takes its own row back"
        );
        let both = [
            branch("ui"),
            left(3),
            left(2),
            commit(1),
            commit(3),
            commit(2),
        ];
        let below_one = [branch("ui"), commit(1), commit(3), commit(2)];
        assert_eq!(
            taken_away(&both, &below_one, false, true),
            [(1, 0), (2, 0)],
            "commits landing together go under the row above the first"
        );
    }

    #[test]
    fn a_cherry_pick_landing_takes_nothing_away_as_the_commit_stays() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let branch = |name: &str| {
            let name = full(&format!("refs/heads/{name}"));
            Some(RowKey::Item(Selection::Branch(p(), Some(name))))
        };
        let commit = |n: usize| Some(RowKey::Commit(p(), n.to_string()));
        let left = || Some(RowKey::Left(Box::new(RowKey::Commit(p(), "3".into()))));
        // Commit 3's copy at the top of api, then landed there as commit 9, a new change.
        let copied = [branch("ui"), left(), commit(2), branch("api"), commit(3)];
        let landed = [branch("ui"), commit(3), commit(2), branch("api"), commit(9)];
        assert_eq!(
            taken_away(&copied, &landed, false, false),
            [],
            "3 takes its own row back where it is, as a move let go of does"
        );
    }

    #[test]
    fn what_a_squash_takes_away_stays_under_the_row_above_it_as_it_ends() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let ui = || {
            let name = full("refs/heads/ui");
            Some(RowKey::Item(Selection::Branch(p(), Some(name))))
        };
        let commit = |n: usize| Some(RowKey::Commit(p(), n.to_string()));
        // Commit 3 squashed into 1, or 3 and 2 squashed where they're held into 1 below them.
        let before = [ui(), commit(3), commit(2), commit(1)];
        assert_eq!(
            taken_away(&before, &[ui(), commit(2), commit(1)], true, false),
            [(1, 0)],
            "it goes under ui, as the rows close over it"
        );
        let into_one = [ui(), commit(1)];
        assert_eq!(
            taken_away(&before, &into_one, true, false),
            [(1, 0), (2, 0)],
            "commits going together go under the row above the first"
        );
        assert_eq!(
            taken_away(&before, &into_one, false, false),
            [],
            "while no squash ends, commits that go are another's to show going"
        );
    }

    #[test]
    fn a_squash_stops_at_every_commit_folded_branch_and_heading_it_can_go_into() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let home = Target::Commit(id(2));
        let lands = |stop: &Stop| *stop != Stop::Into(Target::Commit(id(5)));
        let (stops, start) = squash_stops(&rows, &project, &ws, &home, open, lands).unwrap();
        // Each checkout's changes, uncommitting; every commit, but 5 where it doesn't land, and 2, where
        // it starts; and folded typo for its newest commit, out of sight. Open branches have
        // their newest commits listed.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
into uncommitted on main
into 3
into 2 <
into 1
into typo
into uncommitted on wt

"#]]
        );
    }

    #[test]
    fn a_clean_worktrees_open_heading_takes_what_a_squash_uncommits() {
        let project = ProjectId::from("p");
        let (ws, mut rows) = listed(&project);
        rows.retain(|row| !matches!(row, Row::Item(Selection::Uncommitted(..))));
        let home = Target::Commit(id(2));
        let (stops, _) = squash_stops(&rows, &project, &ws, &home, open, |_| true).unwrap();
        assert!(
            stops.contains(&Stop::Into(Target::Uncommitted(Checkout::Main))),
            "with no changes row, the heading is where they uncommit into"
        );
        assert!(
            stops.contains(&Stop::Into(Target::Uncommitted(Checkout::Linked(
                "wt".into()
            )))),
            "a linked worktree's as the main one's"
        );
    }

    #[test]
    fn a_project_row_stands_in_for_a_main_checkout_without_a_heading() {
        let project = ProjectId::from("p");
        let (ws, mut rows) = listed(&project);
        let divider = rows
            .iter()
            .position(|row| matches!(row, Row::WorktreeDivider));
        rows.truncate(divider.unwrap());
        rows.retain(|row| {
            !matches!(
                row,
                Row::Item(Selection::Checkout(..) | Selection::Uncommitted(..))
            )
        });
        let (stops, _) = stops(&rows, &project, &[id(2)], false, false, &ws, open);
        assert!(
            matches!(
                (stops.first(), stops.last()),
                (
                    Some(Stop::NewStack { before: Some(_) }),
                    Some(Stop::NewStack { before: None })
                )
            ),
            "the main checkout's stacks are listed under it, so new ones go around them"
        );
        let home = Target::Commit(id(2));
        let (stops, _) = squash_stops(&rows, &project, &ws, &home, open, |_| true).unwrap();
        assert!(
            stops.contains(&Stop::Into(Target::Uncommitted(Checkout::Main))),
            "with no changes row, what a squash uncommits goes there"
        );
        let filtered = [
            Row::Item(Selection::Project(project.clone())),
            Row::Item(Selection::Checkout(
                project.clone(),
                Checkout::Linked("wt".into()),
            )),
            Row::Item(Selection::Branch(
                project.clone(),
                Some(full("refs/heads/agent")),
            )),
        ];
        let (stops, _) = super::stops(&filtered, &project, &[id(5)], false, false, &ws, open);
        assert!(
            !stops
                .iter()
                .any(|stop| matches!(stop, Stop::NewStack { .. })),
            "the filter left the main checkout's heading out with all its stacks, not alone"
        );
    }

    #[test]
    fn a_folded_heading_takes_nothing_a_squash_uncommits() {
        let project = ProjectId::from("p");
        let (ws, mut rows) = listed(&project);
        rows.retain(|row| !matches!(row, Row::Item(Selection::Uncommitted(..))));
        let home = Target::Commit(id(2));
        let folded = |heading: &Selection| !matches!(heading, Selection::Checkout(..));
        let (stops, _) = squash_stops(&rows, &project, &ws, &home, folded, |_| true).unwrap();
        assert!(
            !stops.contains(&Stop::Into(Target::Uncommitted(Checkout::Main))),
            "what it lands in would be out of sight"
        );
    }

    #[test]
    fn a_heading_over_listed_changes_is_no_stop_of_its_own() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let home = Target::Commit(id(2));
        let (stops, _) = squash_stops(&rows, &project, &ws, &home, open, |_| true).unwrap();
        let main = Stop::Into(Target::Uncommitted(Checkout::Main));
        assert_eq!(
            stops.iter().filter(|stop| **stop == main).count(),
            1,
            "the changes' row and the heading are one place to go"
        );
    }

    #[test]
    fn a_click_on_a_clean_worktrees_heading_uncommits_there() {
        let project = ProjectId::from("p");
        let (ws, mut rows) = listed(&project);
        rows.retain(|row| !matches!(row, Row::Item(Selection::Uncommitted(..))));
        let home = Target::Commit(id(2));
        let (stops, start) = squash_stops(&rows, &project, &ws, &home, open, |_| true).unwrap();
        let moving = Move {
            project: project.clone(),
            what: Moving::Commits(Arc::from([id(2)])),
            at: stops[start].clone(),
            op: Op::Squash(Messages::Both),
        };
        let heading = Selection::Checkout(project, Checkout::Main);
        let clicked = clicked_stop(&stops, (start, start), &moving, &heading, false, None);
        assert_eq!(
            clicked.map(|ix| &stops[ix]),
            Some(&Stop::Into(Target::Uncommitted(Checkout::Main))),
            "as the keys step there"
        );
    }

    #[test]
    fn a_branch_squashes_into_one_commit_at_its_own_row_but_never_into_the_target() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let ws = Workspace {
            target_branch: Some(full("refs/heads/agent")),
            ..ws
        };
        let home = Target::Branch(full("refs/heads/api"));
        let lands = |stop: &Stop| matches!(stop, Stop::Into(Target::Commit(_)));
        let (stops, start) = squash_stops(&rows, &project, &ws, &home, open, lands).unwrap();
        // Open api is listed as it's what's squashed; the headings don't land, and agent is
        // the target, which work stays off, its commits too.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
into 3
into api <
into 2
into 1

"#]]
        );
    }

    #[test]
    fn a_branch_stops_only_between_branches_and_where_it_is() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (all, _) = stops(&rows, &project, &[], false, true, &ws, open);
        let api = full("refs/heads/api");
        // Below itself it doesn't move, and a branch keeps to its own side, out of wt.
        let lands = |stop: &Stop| match stop {
            Stop::NewBranch { beside, below } => match short_ref(beside).as_ref() {
                "api" => !below,
                "agent" => false,
                _ => true,
            },
            Stop::In(_)
            | Stop::NewStack { .. }
            | Stop::Card(_)
            | Stop::Worktree(_)
            | Stop::Terminal { .. }
            | Stop::Into(_) => true,
        };
        let (stops, start) = branch_stops(all, &api, lands).unwrap();
        // The places between branches it lands at, where a new branch would go, a stack of its
        // own before each stack and after them, and where it is, the place right above it.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new stack before api
new branch above ui
new branch above api <
new stack before typo
new branch above typo
new branch below typo
new stack

"#]]
        );
    }

    #[test]
    fn a_branch_stacked_outside_the_workspace_moves_within_its_stack_and_apart() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let named = |name: &str| Some(full(&format!("refs/heads/{name}")));
        // `top` checked out on `low`, without the workspace, on target `main`.
        let ws = Workspace {
            target_branch: Some(full("refs/heads/main")),
            ..workspace(vec![vec![branch("top", &[2]), branch("low", &[1])]], vec![])
        };
        let rows = [
            Row::Item(Selection::Project(p())),
            Row::Item(Selection::Checkout(p(), Checkout::Main)),
            Row::Item(Selection::Uncommitted(p(), Checkout::Main)),
            Row::Item(Selection::Branch(p(), named("top"))),
            Row::Item(Selection::Commit(p(), id(2))),
            Row::Item(Selection::Branch(p(), named("low"))),
            Row::Item(Selection::Commit(p(), id(1))),
        ];
        let (all, _) = stops(&rows, &project, &[], false, true, &ws, open);
        // It's already right above low.
        let above_low = Stop::NewBranch {
            beside: full("refs/heads/low"),
            below: false,
        };
        let lands = |stop: &Stop| *stop != above_low;
        let (stops, start) = branch_stops(all, &full("refs/heads/top"), lands)
            .expect("it can go below low, or in a stack of its own");
        // Below low, or in a stack of its own, as the engine moves it there without the
        // workspace too.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
new stack before top
new branch above top <
new branch below low
new stack

"#]]
        );
    }

    #[test]
    fn a_terminal_stops_among_the_terminals_of_every_open_worktree() {
        let project = ProjectId::from("p");
        let p = || project.clone();
        let terminal = |n| Row::Item(Selection::Terminal(p(), TerminalId(n)));
        let rows = [
            Row::Item(Selection::Project(p())),
            Row::Item(Selection::Checkout(p(), Checkout::Main)),
            Row::Item(Selection::Uncommitted(p(), Checkout::Main)),
            terminal(1),
            terminal(2),
            Row::Item(Selection::Branch(p(), Some(full("refs/heads/api")))),
            Row::Item(Selection::Checkout(p(), Checkout::Linked("wt".into()))),
            Row::Item(Selection::Checkout(p(), Checkout::Linked("folded".into()))),
            terminal(3),
            Row::Item(Selection::Checkout(p(), Checkout::Linked("empty".into()))),
        ];
        // `folded` lists terminal 3 as it closes.
        let open = |heading: &Selection| !matches!(heading, Selection::Checkout(_, Checkout::Linked(name)) if name == "folded");
        let lists = terminal_lists(&rows, open);
        let (stops, start) = group_slots(&lists, &TerminalId(1)).unwrap();
        let stops: Vec<Stop> = (stops.into_iter())
            .map(|(checkout, slot)| Stop::Terminal { checkout, slot })
            .collect();
        // Before 2 or after it on main, where 1 is before 2, and first on each open linked
        // worktree, with terminals or none, but the folded one.
        snapbox::assert_data_eq!(
            describe(&stops, start),
            snapbox::str![[r#"
before terminal 2 on p/main <
last on p/main
last on p/wt
last on p/empty

"#]]
        );
        assert_eq!(
            lists.first(),
            Some(&(
                on(&project, Checkout::Main),
                vec![TerminalId(1), TerminalId(2)]
            )),
            "the main worktree's terminals, as listed"
        );
        let headless: Vec<Row> = (rows.into_iter())
            .filter(|row| !matches!(row, Row::Item(Selection::Checkout(_, Checkout::Main))))
            .collect();
        assert_eq!(
            terminal_lists(&headless, open).first(),
            Some(&(
                on(&project, Checkout::Main),
                vec![TerminalId(1), TerminalId(2)]
            )),
            "the project's row stands in for a main worktree without a heading"
        );
    }

    /// Commit 2 of `listed`'s being moved as `op`, with the stops a move of it has.
    fn moving_commit(project: &ProjectId, op: Op) -> (Move, Vec<Stop>, usize) {
        let (ws, rows) = listed(project);
        let (stops, start) = stops(&rows, project, &[id(2)], false, false, &ws, open);
        let moving = Move {
            project: project.clone(),
            what: Moving::Commits(Arc::from([id(2)])),
            at: stops[start].clone(),
            op,
        };
        (moving, stops, start)
    }

    /// The stop `clicked` says a click puts a move at, as `describe` names it.
    fn named(stops: &[Stop], clicked: Option<usize>) -> Option<String> {
        let ix = clicked?;
        Some(describe(&stops[ix..=ix], usize::MAX).trim().to_owned())
    }

    #[test]
    fn a_click_puts_commits_under_the_commit_clicked() {
        let project = ProjectId::from("p");
        let (moving, stops, start) = moving_commit(&project, Op::Move);
        let clicked = clicked_stop(
            &stops,
            (start, start),
            &moving,
            &Selection::Commit(project, id(1)),
            false,
            None,
        );
        assert_eq!(
            named(&stops, clicked).as_deref(),
            Some("below 1"),
            "a commit takes what's moving right under it"
        );
    }

    #[test]
    fn a_click_puts_commits_at_the_top_of_the_branch_clicked() {
        let project = ProjectId::from("p");
        let (moving, stops, start) = moving_commit(&project, Op::Move);
        let ui = Selection::Branch(project, Some(full("refs/heads/ui")));
        let clicked = clicked_stop(&stops, (start, start), &moving, &ui, false, None);
        assert_eq!(
            named(&stops, clicked).as_deref(),
            Some("top of ui"),
            "an open branch takes it at its top"
        );
    }

    #[test]
    fn a_click_on_a_folded_branch_puts_commits_at_its_top() {
        let project = ProjectId::from("p");
        let (moving, stops, start) = moving_commit(&project, Op::Move);
        let typo = Selection::Branch(project, Some(full("refs/heads/typo")));
        let clicked = clicked_stop(&stops, (start, start), &moving, &typo, false, None);
        assert_eq!(
            named(&stops, clicked).as_deref(),
            Some("top of typo"),
            "folded, it takes them at its top as an open branch does"
        );
    }

    #[test]
    fn a_click_on_a_folded_branch_puts_a_branch_above_it() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let (all, _) = stops(&rows, &project, &[], false, true, &ws, open);
        let api = full("refs/heads/api");
        let (stops, start) = branch_stops(all, &api, |_| true).unwrap();
        let moving = Move {
            project: project.clone(),
            what: Moving::Branch(api),
            at: stops[start].clone(),
            op: Op::Move,
        };
        let typo = Selection::Branch(project, Some(full("refs/heads/typo")));
        let clicked = clicked_stop(&stops, (start, start), &moving, &typo, false, None);
        assert_eq!(
            named(&stops, clicked).as_deref(),
            Some("new branch above typo"),
            "a branch lands only between branches, so never at another's top"
        );
    }

    #[test]
    fn a_click_on_what_moves_takes_it_back_where_it_started() {
        let project = ProjectId::from("p");
        let (moving, stops, start) = moving_commit(&project, Op::Move);
        let clicked = clicked_stop(
            &stops,
            (0, start),
            &moving,
            &Selection::Commit(project, id(2)),
            true,
            None,
        );
        assert_eq!(clicked, Some(start), "it goes back from the first stop");
    }

    #[test]
    fn a_click_on_the_changes_being_committed_leaves_the_new_commit_where_it_is() {
        let project = ProjectId::from("p");
        let (moving, stops, start) = moving_commit(&project, Op::Commit);
        let clicked = clicked_stop(
            &stops,
            (0, start),
            &moving,
            &Selection::Uncommitted(project, Checkout::Main),
            true,
            None,
        );
        assert_eq!(
            clicked, None,
            "a commit being made started nowhere to go back to"
        );
    }

    #[test]
    fn a_click_in_another_project_leaves_commits_where_they_are() {
        let project = ProjectId::from("p");
        let (moving, stops, start) = moving_commit(&project, Op::Move);
        let elsewhere = Selection::Commit(ProjectId::from("q"), id(1));
        let clicked = clicked_stop(&stops, (start, start), &moving, &elsewhere, false, None);
        assert_eq!(clicked, None, "commits move only within their project");
    }

    #[test]
    fn a_click_on_a_row_commits_cant_land_at_leaves_them_where_they_are() {
        let project = ProjectId::from("p");
        let (moving, stops, start) = moving_commit(&project, Op::Move);
        let terminal = Selection::Terminal(project, TerminalId(1));
        let clicked = clicked_stop(&stops, (start, start), &moving, &terminal, false, None);
        assert_eq!(clicked, None, "a terminal takes no commits");
    }

    /// A branch of commits, newest first: 3, 2, 1.
    fn branch_of_three() -> Workspace {
        workspace(vec![vec![branch("ui", &[3, 2, 1])]], Vec::new())
    }

    #[test]
    fn a_commit_placed_on_top_of_a_branch_has_its_newest_under_it() {
        let at = Stop::In(Destination::Branch(full("refs/heads/ui")));
        assert_eq!(
            under_commit_at(&at, &branch_of_three()),
            Some(id(3)),
            "amending instead goes into the commit it'd go on top of"
        );
    }

    #[test]
    fn a_commit_placed_below_one_has_the_next_under_it() {
        let at = Stop::In(Destination::BelowCommit(id(3)));
        assert_eq!(
            under_commit_at(&at, &branch_of_three()),
            Some(id(2)),
            "the commit it's below is above it, so not the one it'd amend"
        );
    }

    #[test]
    fn a_commit_placed_above_one_has_that_one_under_it() {
        let at = Stop::In(Destination::AboveCommit(id(2)));
        assert_eq!(
            under_commit_at(&at, &branch_of_three()),
            Some(id(2)),
            "it's right on top of it"
        );
    }

    #[test]
    fn a_commit_placed_at_the_bottom_of_a_branch_has_nothing_under_it() {
        let at = Stop::In(Destination::BelowCommit(id(1)));
        assert_eq!(
            under_commit_at(&at, &branch_of_three()),
            None,
            "the branch has no commit further down"
        );
    }

    #[test]
    fn a_commit_placed_in_a_new_stack_has_nothing_under_it() {
        let at = Stop::NewStack { before: None };
        assert_eq!(
            under_commit_at(&at, &branch_of_three()),
            None,
            "a new branch has no commits"
        );
    }

    #[test]
    fn a_click_squashes_into_the_row_clicked() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let home = Target::Commit(id(2));
        let (stops, start) =
            squash_stops(&rows, &project, &ws, &home, open, |_| true).expect("2 is listed");
        let moving = Move {
            project: project.clone(),
            what: Moving::Commits(Arc::from([id(2)])),
            at: stops[start].clone(),
            op: Op::Squash(Messages::Both),
        };
        let three = Selection::Commit(project, id(3));
        let clicked = clicked_stop(&stops, (start, start), &moving, &three, false, None);
        assert_eq!(
            clicked.map(|ix| &stops[ix]),
            Some(&Stop::Into(Target::Commit(id(3)))),
            "the commit clicked takes the squash"
        );
    }

    #[test]
    fn a_click_on_a_row_nothing_squashes_into_leaves_the_squash_where_it_is() {
        let project = ProjectId::from("p");
        let (ws, rows) = listed(&project);
        let home = Target::Commit(id(2));
        let (stops, start) =
            squash_stops(&rows, &project, &ws, &home, open, |_| true).expect("2 is listed");
        let moving = Move {
            project: project.clone(),
            what: Moving::Commits(Arc::from([id(2)])),
            at: stops[start].clone(),
            op: Op::Squash(Messages::Both),
        };
        let terminal = Selection::Terminal(project, TerminalId(1));
        let clicked = clicked_stop(&stops, (start, start), &moving, &terminal, false, None);
        assert_eq!(clicked, None, "a terminal is nothing to squash into");
    }

    /// Card `b` among `a`, `b`, `c` being moved, at `at`, with the stops a move of it has.
    fn moving_card(at: usize) -> (Move, Vec<Stop>, usize) {
        let [a, b, c] = ["a", "b", "c"].map(ProjectId::from);
        let order: Arc<[ProjectId]> = Arc::from([a, b.clone(), c]);
        let (slots, start) = slots(&order, &b).expect("b is listed");
        let stops: Vec<Stop> = slots.into_iter().map(Stop::Card).collect();
        let moving = Move {
            project: b,
            what: Moving::Project(order),
            at: stops[at].clone(),
            op: Op::Move,
        };
        (moving, stops, start)
    }

    #[test]
    fn a_click_on_a_card_below_puts_the_card_moving_after_it() {
        let (moving, stops, start) = moving_card(1);
        let c = Selection::Project(ProjectId::from("c"));
        let clicked = clicked_stop(&stops, (start, start), &moving, &c, false, None);
        assert_eq!(
            clicked.map(|ix| &stops[ix]),
            Some(&Stop::Card(Slot::End)),
            "it takes c's place, after it"
        );
    }

    #[test]
    fn a_click_on_a_card_above_puts_the_card_moving_before_it() {
        let (moving, stops, start) = moving_card(1);
        let a = Selection::Checkout(ProjectId::from("a"), Checkout::Main);
        let clicked = clicked_stop(&stops, (start, start), &moving, &a, false, None);
        assert_eq!(
            clicked.map(|ix| &stops[ix]),
            Some(&Stop::Card(Slot::Before(ProjectId::from("a")))),
            "any row of a's card puts it in a's place, before it"
        );
    }

    #[test]
    fn a_click_on_the_card_moving_leaves_it_where_it_is() {
        let (moving, stops, start) = moving_card(2);
        let b = Selection::Project(ProjectId::from("b"));
        let clicked = clicked_stop(&stops, (2, start), &moving, &b, false, None);
        assert_eq!(clicked, Some(2), "it's already where it's shown");
    }

    #[test]
    fn a_click_in_a_worktree_puts_the_terminal_moving_last_in_it() {
        let project = ProjectId::from("p");
        let lists = [
            (
                on(&project, Checkout::Main),
                vec![TerminalId(1), TerminalId(2)],
            ),
            (on(&project, Checkout::Linked("wt".into())), vec![]),
        ];
        let (slots, start) = group_slots(&lists, &TerminalId(1)).expect("1 is listed");
        let stops: Vec<Stop> = (slots.into_iter())
            .map(|(checkout, slot)| Stop::Terminal { checkout, slot })
            .collect();
        let moving = Move {
            project: project.clone(),
            what: Moving::Terminal {
                id: TerminalId(1),
                lists: Arc::from(lists),
            },
            at: stops[start].clone(),
            op: Op::Move,
        };
        let agent = Selection::Branch(project.clone(), Some(full("refs/heads/agent")));
        let wt = Checkout::Linked("wt".into());
        let clicked = clicked_stop(
            &stops,
            (start, start),
            &moving,
            &agent,
            false,
            Some(wt.clone()),
        );
        assert_eq!(
            clicked.map(|ix| &stops[ix]),
            Some(&Stop::Terminal {
                checkout: on(&project, wt),
                slot: Slot::End
            }),
            "a branch of wt's takes it under wt's heading"
        );
    }

    #[test]
    fn a_terminal_from_a_tile_shows_among_those_listed_where_it_is_held() {
        let project = ProjectId::from("p");
        let (main, wt) = (
            on(&project, Checkout::Main),
            on(&project, Checkout::Linked("wt".into())),
        );
        let moving = Move {
            project,
            what: Moving::Terminal {
                id: TerminalId(3),
                lists: Arc::from([
                    (main.clone(), vec![TerminalId(1)]),
                    (wt.clone(), vec![TerminalId(2)]),
                ]),
            },
            at: Stop::Terminal {
                checkout: wt.clone(),
                slot: Slot::Before(TerminalId(2)),
            },
            op: Op::Move,
        };
        assert_eq!(
            moving.terminal_order(),
            Some(vec![
                (main, vec![TerminalId(1)]),
                (wt, vec![TerminalId(3), TerminalId(2)]),
            ]),
            "it's listed nowhere yet, so it only joins the worktree it's held over"
        );
    }

    #[test]
    fn a_click_on_the_next_terminal_puts_the_terminal_moving_after_it() {
        let project = ProjectId::from("p");
        let lists = [(
            on(&project, Checkout::Main),
            vec![TerminalId(1), TerminalId(2)],
        )];
        let (slots, start) = group_slots(&lists, &TerminalId(1)).expect("1 is listed");
        let stops: Vec<Stop> = (slots.into_iter())
            .map(|(checkout, slot)| Stop::Terminal { checkout, slot })
            .collect();
        let moving = Move {
            project: project.clone(),
            what: Moving::Terminal {
                id: TerminalId(1),
                lists: Arc::from(lists),
            },
            at: stops[start].clone(),
            op: Op::Move,
        };
        let two = Selection::Terminal(project.clone(), TerminalId(2));
        let clicked = clicked_stop(
            &stops,
            (start, start),
            &moving,
            &two,
            false,
            Some(Checkout::Main),
        );
        assert_eq!(
            clicked.map(|ix| &stops[ix]),
            Some(&Stop::Terminal {
                checkout: on(&project, Checkout::Main),
                slot: Slot::End
            }),
            "they trade places"
        );
    }
}
