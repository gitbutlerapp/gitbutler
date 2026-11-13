//! What's dragged over the navigator's rows, which is a move, the pointer stepping it through its
//! stops as the keys do, the zone for a new branch, and scrolling while it's near an edge. The
//! rows stay where they are as it goes, so what the pointer aims at holds still: a divider between
//! them shows where it lands. A linked worktree dragged by its heading moves as the move of it
//! does (see `order.rs`).

use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Context, Div, DragMoveEvent, Hsla, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, PlatformInput, Point, Window, div, prelude::*,
};

use super::Navigator;
use super::rows::{CARD_EDGE, Row, card_ends, heads, span_height};
use crate::backend::{Checkout, CommitId, Destination, FullName, Messages};
use crate::state::{ProjectId, Section, Selection};
use crate::theme::{Hue, Theme};
use crate::transfer::{Changes, Dragged, Move, Moving, Op, Place, Stop, Target};
use crate::ui::{Icon, dp, h_flex, small_icon};

/// Where the pointer aims what's dragged: `depth` down rows `row` tall.
#[derive(Clone, Copy)]
struct Aim {
    depth: f32,
    row: f32,
}

impl Navigator {
    pub(super) fn row_menu(
        &self,
        el: AnyElement,
        menu_target: Selection,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .relative()
            .child(el)
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    // No menu while something is being moved, which is all a click can do then.
                    if this.store.read(cx).moving.is_some() {
                        return this.nudge_move(cx);
                    }
                    if this.row_menu_items(&menu_target, cx).is_empty() {
                        return;
                    }
                    let picked = matches!(&menu_target,
                        Selection::Commit(project, id) if this.store.read(cx).is_picked(project, *id));
                    if picked {
                        // The picks' actions are the navigator's, dispatched from focus.
                        window.focus(&this.focus, cx);
                    } else {
                        this.select(menu_target.clone(), window, cx);
                    }
                    // After selecting, which reads a commit, so its menu offers to unread it.
                    let target = menu_target.clone();
                    this.open_menu(
                        event.position,
                        move |this, cx| this.row_menu_items(&target, cx),
                        cx,
                    );
                }),
            )
            .into_any_element()
    }

    /// Have what's dragged be at the stop under the pointer, as the keys would step it there.
    /// Off the rows, it's no move: what it's over takes it as anything dragged, and the rows are
    /// as they were. A move the keys started goes on, as does that of a linked worktree dragged
    /// by its heading.
    pub(super) fn follow_dragged(
        &mut self,
        event: &DragMoveEvent<Dragged>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        if (store.moving.as_ref()).is_some_and(|moving| !moving.dragged(store)) {
            return;
        }
        // A linked worktree dragged by its heading follows the pointer on its own.
        let Some(Dragged { project, what }) = (store.dragging.clone())
            .filter(|dragged| !matches!(dragged.what, Moving::Worktree { .. }))
        else {
            return;
        };
        let at = event.event.position;
        let depth = (event.bounds.contains(&at))
            .then(|| self.depth_at(at, Theme::get(cx)))
            .flatten();
        self.hold_dragged(project, what, depth, cx);
    }

    /// Have what's dragged be `depth` down the rows, or with `None` end its move.
    fn hold_dragged(
        &mut self,
        project: ProjectId,
        what: Moving,
        depth: Option<f32>,
        cx: &mut Context<Self>,
    ) {
        let next = depth.and_then(|depth| self.dragged_to(project, what, depth, cx));
        let store = self.store.read(cx);
        let unchanged = match (&store.moving, &next) {
            (Some(was), Some(next)) => was.at == next.at && was.op == next.op,
            (was, next) => was.is_none() && next.is_none(),
        };
        if !unchanged {
            let held = next.is_some();
            (self.store).update(cx, |store, cx| {
                store.set_moving(next, cx);
                store.move_dragged = held;
            });
        }
    }

    /// `what` of `project` where [`drag_zone`] says the pointer `depth` down the rows has it.
    fn dragged_to(&self, project: ProjectId, what: Moving, depth: f32, cx: &App) -> Option<Move> {
        let takes = |at: &Stop, op: Op| {
            (self.stops_for(&project, &what, op, cx)).is_some_and(|(stops, _)| stops.contains(at))
        };
        let aim = Aim {
            depth,
            row: self.row,
        };
        let (at, op) = match drag_zone(&self.rows, &project, &what, aim, takes) {
            Some(landing) => landing,
            None => self.dragged_from(&project, &what, cx)?,
        };
        Some(Move {
            project,
            what,
            at,
            op,
        })
    }

    /// Where what's dragged is, which lands nothing.
    fn dragged_from(&self, project: &ProjectId, what: &Moving, cx: &App) -> Option<(Stop, Op)> {
        match what {
            // Dragged from a page, their row may not be listed, which is nothing to them.
            Moving::Files(_) => {
                let selected = self.store.read(cx).selection.as_ref();
                let home = what.home(project, selected)?;
                Some((Stop::Into(home), Op::Squash(Messages::default())))
            }
            Moving::Commits(_)
            | Moving::Branch(_)
            | Moving::Project(_)
            | Moving::Worktree { .. }
            | Moving::Terminal { .. }
            | Moving::Copies(_) => {
                let (stops, start) = self.stops_for(project, what, Op::Move, cx)?;
                Some((stops.into_iter().nth(start)?, Op::Move))
            }
        }
    }

    /// Over the rows, the line between them where what's dragged would land. Drawn over them, not
    /// among them, so no row moves for it. What lands on a row, as files starting a commit on a
    /// branch do, has the row outlined instead.
    pub(super) fn drag_divider(&self, theme: &Theme, cx: &App) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let moving = (store.moving.as_ref())
            .filter(|moving| moving.dragged(store) && moving.lands(store))?;
        // A branch joining a stack has it outlined, and the line too where it goes into it.
        if self.squash_outline.is_some() && !joins_stack(&self.rows, moving) {
            return None;
        }
        let line = divider_at(&self.rows, &moving.project, &moving.at)?;
        let (ix, above) = (line.row, line.place == Place::Above);
        let inset = self.insets.get(line.inset_of).copied().unwrap_or_default();
        // In the card under a tab, from the top of its rows, all of which are drawn.
        let (edge, (left, right)) = match self.card_top.get() {
            Some(_) => {
                let rows = 0..ix + usize::from(!above);
                (
                    theme.scaled(span_height(&self.rows, rows, self.row)),
                    (0., 0.),
                )
            }
            None => {
                let row = self.list.bounds_for_item(ix)?;
                // A card's first and last rows hold its edge, which the line stays inside.
                let (first, last) = card_ends(&self.rows, ix);
                let edge = match above {
                    true if first => row.top() + theme.scaled(CARD_EDGE),
                    true => row.top(),
                    false if last => row.bottom() - theme.scaled(CARD_EDGE),
                    false => row.bottom(),
                };
                (
                    edge - self.list.viewport_bounds().top(),
                    self.card_margins(),
                )
            }
        };
        let line = divider(moving.op.colour(theme))
            .left(dp(left + inset + 10.))
            .right(dp(right + 10.))
            .top(edge - theme.scaled(1.));
        Some(
            div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .child(line)
                .into_any_element(),
        )
    }

    /// Let go of what's dragged where its move is, as Return lands it, or of a linked worktree
    /// where it's shown. Changes on a branch start a commit there, as the composer takes them.
    pub(super) fn drop_dragged(
        &mut self,
        dropped: &Dragged,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drop_worktree(dropped, window, cx);
        let store = self.store.read(cx);
        let Some(moving) = (store.moving.clone()).filter(|moving| moving.dragged(store)) else {
            return;
        };
        let Move {
            project, at, op, ..
        } = &moving;
        if let (Op::Commit, Stop::In(Destination::Branch(branch)), Some(dragged)) =
            (*op, at, store.dragging.clone())
        {
            let target = Selection::Branch(project.clone(), Some(branch.clone()));
            (self.store).update(cx, |store, cx| store.set_moving(None, cx));
            return self.app.update_later(window, cx, move |app, window, cx| {
                app.transfer(&dragged.what, &target, Place::Into, window, cx)
            });
        }
        self.land_move(window, cx);
    }

    /// Where a commit being dragged lands in a new branch of its own: a dashed box, lit while a
    /// drag is over it. Padded, not margined: a margin collapses through the card's ground.
    pub(super) fn new_branch_zone(&self, ix: usize, theme: &Theme) -> AnyElement {
        let highlight = theme.pop;
        let wash = theme.wash(Hue::Pop);
        let zone = h_flex()
            .id(("new-branch-drop", ix))
            .h(dp(30.))
            .mx(dp(3.))
            .justify_center()
            .gap(dp(6.))
            .rounded(dp(theme.shape.control))
            .border_1()
            .border_dashed()
            .border_color(theme.border_2)
            .text_size(dp(12.))
            .text_color(theme.text_2)
            .child(small_icon(Icon::Branch).text_color(theme.text_2))
            .child("New independent branch")
            .drag_over::<Dragged>(move |style, _, _, _| {
                style.bg(wash).border_color(highlight).text_color(highlight)
            });
        div().py(dp(2.)).child(zone).into_any_element()
    }

    /// While something dragged is near the list's top or bottom edge, or past it, scroll the
    /// list that way, faster the closer it gets, so it can reach rows out of view.
    pub(super) fn scroll_near_edge(
        &mut self,
        at: Point<Pixels>,
        list: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        const EDGE: f32 = 48.;
        const FASTEST: f32 = 16.;
        let edge = Theme::get(cx).scaled(EDGE);
        let closeness = |gap: Pixels| 1. - (gap / edge).max(0.);
        let (from_top, from_bottom) = (at.y - list.top(), list.bottom() - at.y);
        self.edge_scroll = match at.x >= list.left() && at.x <= list.right() {
            false => 0.,
            true if from_top < edge => -FASTEST * closeness(from_top),
            true if from_bottom < edge => FASTEST * closeness(from_bottom),
            true => 0.,
        };
        if self.edge_scroll == 0. || self.edge_scrolling.is_some() {
            return;
        }
        self.edge_scrolling = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let step = this.update_in(cx, |this, window, cx| {
                    let speed = this.edge_scroll;
                    if speed == 0. || !cx.has_active_drag() {
                        return false;
                    }
                    let offset = -this.list.scroll_px_offset_for_scrollbar().y;
                    let room = match speed < 0. {
                        true => offset > Pixels::ZERO,
                        false => offset < this.list.max_offset_for_scrollbar().y,
                    };
                    if room {
                        this.list.scroll_by(Theme::get(cx).scaled(speed));
                        cx.notify();
                        // The rows moved under a pointer that stays put: once they're drawn
                        // where they are now, have the drag land again where it is, as moving
                        // the pointer would. A next frame's callbacks run before it's drawn,
                        // so the frame after the one that draws them.
                        window.on_next_frame(|window, _| {
                            window.on_next_frame(|window, cx| {
                                let event = MouseMoveEvent {
                                    position: window.mouse_position(),
                                    pressed_button: Some(MouseButton::Left),
                                    modifiers: window.modifiers(),
                                };
                                window.dispatch_event(PlatformInput::MouseMove(event), cx);
                            })
                        });
                    }
                    true
                });
                if !step.unwrap_or(false) {
                    break;
                }
            }
            (this.update(cx, |this, _| this.edge_scrolling = None)).ok();
        }));
    }
}

/// Where the pointer's `aim` over `rows` lands `what` of `project`, as what, where `takes` says
/// what lands where. Nowhere, what's dragged goes back where it is, over its own rows too.
fn drag_zone(
    rows: &[Row],
    project: &ProjectId,
    what: &Moving,
    aim: Aim,
    takes: impl Fn(&Stop, Op) -> bool,
) -> Option<(Stop, Op)> {
    if over_own_row(rows, project, what, aim.depth, aim.row) {
        return None;
    }
    let at = stop_at(rows, project, what, aim)?;
    let op = op_at(what, &at);
    takes(&at, op).then_some((at, op))
}

/// The stop `what` of `project` is at where `aim` points over `rows`. Commits go between rows
/// from the top and bottom quarter of a commit, into it between, and to the top of a branch from
/// its heading. They make no new branch but the zone's, whose stops would share a gap with
/// others. A branch goes where the pointer is, as [`branch_stop`] says. Files go into a commit or
/// a worktree's changes, whichever row they're over, and a worktree's changes onto a branch start
/// a commit there; they make no new branch. An open stack's caption is aimed at as the top of the
/// branch listed right under it.
fn stop_at(rows: &[Row], project: &ProjectId, what: &Moving, aim: Aim) -> Option<Stop> {
    let (ix, y) = aimed_at(rows, aim.depth, aim.row)?;
    match what {
        Moving::Commits(_) => commit_stop(rows, ix, y, project),
        Moving::Branch(_) => branch_stop(rows, ix, y, project, aim),
        Moving::Files(Changes { from, .. }) => file_stop(rows, ix, project, *from),
        Moving::Project(_)
        | Moving::Worktree { .. }
        | Moving::Terminal { .. }
        | Moving::Copies(_) => None,
    }
}

/// What landing `what` at `at` does: into a row squashes it, and files anywhere else start a
/// commit.
fn op_at(what: &Moving, at: &Stop) -> Op {
    match (what, at) {
        (_, Stop::Into(_)) => Op::Squash(Messages::default()),
        (Moving::Files(_), _) => Op::Commit,
        (_, _) => Op::Move,
    }
}

/// The checkout whose uncommitted changes row `ix` of `rows` takes for `project`: its changes',
/// or as the heading it is or stands in for, as [`heads`] says.
fn changes_at(rows: &[Row], ix: usize, project: &ProjectId) -> Option<Checkout> {
    let row = rows.get(ix)?;
    match row {
        Row::Item(Selection::Uncommitted(p, checkout)) if p == project => Some(checkout.clone()),
        Row::Item(item) if item.project() == project => heads(row, rows.get(ix + 1)),
        _ => None,
    }
}

fn file_stop(rows: &[Row], ix: usize, project: &ProjectId, from: Option<CommitId>) -> Option<Stop> {
    if let Some(checkout) = changes_at(rows, ix, project) {
        return Some(Stop::Into(Target::Uncommitted(checkout)));
    }
    match rows.get(ix)? {
        Row::Item(Selection::Commit(p, id)) if p == project => {
            Some(Stop::Into(Target::Commit(*id)))
        }
        Row::Item(Selection::Branch(p, Some(name))) if p == project && from.is_none() => {
            Some(Stop::In(Destination::Branch(name.clone())))
        }
        _ => None,
    }
}

/// The row `depth` down `rows`, and how far down that row, from 0 at its top to 1 at its bottom.
fn row_at(rows: &[Row], depth: f32, row: f32) -> Option<(usize, f32)> {
    let mut top = 0.;
    for (ix, item) in rows.iter().enumerate() {
        let height = item.height(row);
        if depth < top + height {
            return Some((ix, (depth - top) / height));
        }
        top += height;
    }
    None
}

/// The row the pointer `depth` down `rows` aims at, and how far down it, as [`row_at`] says,
/// but an open stack's caption stands for the top of the branch listed right under it: its top
/// one, or under the filter the first that matches, where the divider then shows it lands.
fn aimed_at(rows: &[Row], depth: f32, row: f32) -> Option<(usize, f32)> {
    let (ix, y) = row_at(rows, depth, row)?;
    match (rows.get(ix), rows.get(ix + 1)) {
        (Some(Row::Item(Selection::Stack(..))), Some(Row::Item(Selection::Branch(..)))) => {
            Some((ix + 1, 0.))
        }
        _ => Some((ix, y)),
    }
}

/// Whether `depth` down `rows` is over what's moving, where it started.
fn over_own_row(rows: &[Row], project: &ProjectId, what: &Moving, depth: f32, row: f32) -> bool {
    let aimed = aimed_at(rows, depth, row).and_then(|(ix, _)| rows.get(ix));
    matches!(aimed, Some(Row::Item(item)) if what.holds(project, item))
}

fn commit_stop(rows: &[Row], ix: usize, y: f32, project: &ProjectId) -> Option<Stop> {
    if let Some(checkout) = changes_at(rows, ix, project) {
        return Some(Stop::Into(Target::Uncommitted(checkout)));
    }
    match rows.get(ix)? {
        Row::NewBranch(p) if p == project => Some(Stop::NewStack { before: None }),
        Row::Item(Selection::Branch(p, Some(name))) if p == project => {
            Some(Stop::In(Destination::Branch(name.clone())))
        }
        Row::Item(Selection::Commit(p, _)) if p == project && y < 0.25 => {
            gap_above(rows, ix, project)
        }
        Row::Item(Selection::Commit(p, id)) if p == project && y > 0.75 => {
            Some(Stop::In(Destination::BelowCommit(*id)))
        }
        Row::Item(Selection::Commit(p, id)) if p == project => {
            Some(Stop::Into(Target::Commit(*id)))
        }
        _ => None,
    }
}

/// The stop between row `ix`, a commit, and what's listed above it in its branch, past any
/// commit on its way there.
fn gap_above(rows: &[Row], ix: usize, project: &ProjectId) -> Option<Stop> {
    let above = rows.get(..ix)?.iter().rev().find_map(|row| match row {
        Row::Item(Selection::Pending(..)) => None,
        Row::Item(item) => Some(item),
        _ => None,
    })?;
    match above {
        Selection::Commit(p, id) if p == project => Some(Stop::In(Destination::BelowCommit(*id))),
        Selection::Branch(p, Some(name)) if p == project => {
            Some(Stop::In(Destination::Branch(name.clone())))
        }
        _ => None,
    }
}

/// Whether `moving` puts a branch in a stack with a caption already, where a line shows where
/// in it. A new one's ghost shows it whole.
fn joins_stack(rows: &[Row], moving: &Move) -> bool {
    let (Moving::Branch(_), Stop::NewBranch { beside, below }) = (&moving.what, &moving.at) else {
        return false;
    };
    let heading = (rows.iter()).position(|row| {
        matches!(row, Row::Item(Selection::Branch(p, Some(name)))
            if *p == moving.project && name == beside)
    });
    heading.is_some_and(|heading| *below || stack_start(rows, heading) != heading)
}

/// Where a branch dragged over row `ix`, `y` of the way down it, lands. A branch and its commits
/// are one target, as a commit is: over its middle it's stacked onto it, right above it, and
/// over the quarter of a row at its top or bottom edge it goes in the gap there. A gap between two
/// branches of a stack is in the stack. One at the top or bottom of one of the main checkout's
/// stacks is between the stacks, where it's a stack of its own, so only dropping it on a branch
/// stacks it. A linked worktree's lane is one stack, so all of it takes it in.
fn branch_stop(rows: &[Row], ix: usize, y: f32, project: &ProjectId, aim: Aim) -> Option<Stop> {
    let on = rows.get(ix)?;
    match on {
        Row::NewBranch(p) if p == project => return Some(Stop::NewStack { before: None }),
        Row::Item(Selection::Branch(p, Some(_)) | Selection::Commit(p, _)) if p == project => {}
        _ => return None,
    }
    let (heading, name) = branch_above(rows, ix + 1, project)?;
    let end = branch_end(rows, heading);
    let apart = in_main(rows, heading);
    let down = span_height(rows, heading..ix, aim.row) + y * on.height(aim.row);
    let edge = aim.row / 4.;
    if down < edge {
        return Some(match apart && starts_stack(rows, heading) {
            true => Stop::NewStack {
                before: Some(name.clone()),
            },
            false => new_branch(name, false),
        });
    }
    if down < span_height(rows, heading..end, aim.row) - edge {
        return Some(new_branch(name, false));
    }
    // The gap below it is above the branch under it in its stack, or at its stack's end.
    Some(match rows.get(end) {
        Some(Row::Item(Selection::Branch(p, Some(next)))) if p == project => {
            new_branch(next, false)
        }
        _ if apart => Stop::NewStack {
            before: next_stack(rows, end, project),
        },
        _ => new_branch(name, true),
    })
}

/// Whether row `heading`, a branch's, is the first of its stack's branches, as the row before it
/// is no other branch's: the stack's caption, the end of the stack before, or what's listed
/// above the stacks.
pub(super) fn starts_stack(rows: &[Row], heading: usize) -> bool {
    let before = heading.checked_sub(1).and_then(|ix| rows.get(ix));
    !before.is_some_and(of_a_branch)
}

/// Whether `row` is a branch's, or listed with one: its commits, its remote's, their folds, and
/// what stands in for commits on their way.
fn of_a_branch(row: &Row) -> bool {
    matches!(
        row,
        Row::Item(
            Selection::Branch(..)
                | Selection::Commit(..)
                | Selection::Upstream(..)
                | Selection::Section(_, Section::Upstream(_))
                | Selection::Pending(..)
        ) | Row::More(..)
            | Row::Fewer(..)
            | Row::Pending(..)
            | Row::Ghost(..)
            | Row::GhostBranch(..)
            | Row::NewCommit(_)
    )
}

/// The row after the last of the stack row `ix` is in.
pub(super) fn stack_end(rows: &[Row], ix: usize) -> usize {
    (ix + 1..rows.len())
        .find(|at| !rows.get(*at).is_some_and(of_a_branch))
        .unwrap_or(rows.len())
}

/// The first row of the stack row `ix` is in: its caption, or its top branch's row.
pub(super) fn stack_start(rows: &[Row], ix: usize) -> usize {
    let mut start = ix;
    while start > 0 && !starts_stack(rows, start) {
        start -= 1;
    }
    match start.checked_sub(1).map(|ix| (ix, rows.get(ix))) {
        Some((caption, Some(Row::Item(Selection::Stack(..))))) => caption,
        _ => start,
    }
}

/// A branch of the next of `project`'s stacks, listed from row `end` on, past the end of the one
/// before it, if there is one.
fn next_stack(rows: &[Row], end: usize, project: &ProjectId) -> Option<FullName> {
    let rows = rows.get(end..)?.iter();
    let mut rows = rows.skip_while(|row| matches!(row, Row::Between(_) | Row::Ghost(_)));
    match rows.next()? {
        Row::Item(Selection::Branch(p, Some(name)) | Selection::Stack(p, Some(name)))
            if p == project =>
        {
            Some(name.clone())
        }
        _ => None,
    }
}

/// Whether row `ix` is under the main checkout rather than a linked worktree's heading, as the
/// heading it's under says: the main checkout's, or a project's row standing in for it.
fn in_main(rows: &[Row], ix: usize) -> bool {
    let above = rows.get(..ix).unwrap_or_default().iter().enumerate().rev();
    let heading = above
        .take_while(|(_, row)| !matches!(row, Row::Gap))
        .find_map(|(at, row)| heads(row, rows.get(at + 1)));
    heading.is_none_or(|checkout| checkout == Checkout::Main)
}

/// The last named branch of `project` listed before row `end`, by its row.
fn branch_above<'a>(
    rows: &'a [Row],
    end: usize,
    project: &ProjectId,
) -> Option<(usize, &'a FullName)> {
    (rows.get(..end)?.iter().enumerate().rev()).find_map(|(at, row)| match row {
        Row::Item(Selection::Branch(p, Some(name))) if p == project => Some((at, name)),
        _ => None,
    })
}

/// Whether `row` comes after a branch's commits rather than among them.
fn ends_branch(row: Option<&Row>) -> bool {
    matches!(
        row,
        Some(
            Row::Item(Selection::Branch(..) | Selection::Stack(..) | Selection::Project(_))
                | Row::Group(_)
                | Row::Between(_)
                | Row::Gap
                | Row::WorktreeDivider
                | Row::NewBranch(_)
        )
    )
}

/// The row after those of the branch headed by row `heading`.
fn branch_end(rows: &[Row], heading: usize) -> usize {
    (heading + 1..rows.len())
        .find(|at| ends_branch(rows.get(*at)))
        .unwrap_or(rows.len())
}

fn new_branch(beside: &FullName, below: bool) -> Stop {
    Stop::NewBranch {
        beside: beside.clone(),
        below,
    }
}

/// Where the line showing a stop is drawn: along an edge of `row`, set in as far as row
/// `inset_of` is.
#[derive(Debug, PartialEq)]
struct Line {
    row: usize,
    place: Place,
    inset_of: usize,
}

/// Where the line showing the stop `at` of `project` is. A stop that goes into a row has no gap
/// to show, nor one that's a zone of its own, which lights up by itself. A new branch beside one
/// is set in as that branch is, and a stack of its own as the first row of the stack it's beside.
fn divider_at(rows: &[Row], project: &ProjectId, at: &Stop) -> Option<Line> {
    let commit = |id: &CommitId| {
        (rows.iter()).position(|row| {
            matches!(row, Row::Item(Selection::Commit(p, commit)) if p == project && commit == id)
        })
    };
    let heading = |name: &FullName| {
        (rows.iter()).position(|row| {
            matches!(row, Row::Item(Selection::Branch(p, Some(branch)))
                if p == project && branch == name)
        })
    };
    let along = |row: usize, place: Place| Line {
        row,
        place,
        inset_of: row,
    };
    let in_stack = |heading: usize, row: usize, place: Place| Line {
        row,
        place,
        inset_of: heading,
    };
    match at {
        Stop::In(Destination::AboveCommit(id)) => Some(along(commit(id)?, Place::Above)),
        Stop::In(Destination::BelowCommit(id)) => Some(along(commit(id)?, Place::Below)),
        // Above its first commit, past what's listed before its commits.
        Stop::In(Destination::Branch(name)) => {
            let heading = heading(name)?;
            let first = (heading + 1..branch_end(rows, heading)).find(|ix| {
                matches!(rows.get(*ix), Some(Row::Item(Selection::Commit(p, _))) if p == project)
            });
            Some(first.map_or(along(heading, Place::Below), |ix| along(ix, Place::Above)))
        }
        Stop::NewBranch {
            beside,
            below: false,
        } => {
            let heading = heading(beside)?;
            Some(in_stack(heading, heading, Place::Above))
        }
        Stop::NewBranch {
            beside,
            below: true,
        } => {
            let heading = heading(beside)?;
            Some(in_stack(
                heading,
                branch_end(rows, heading) - 1,
                Place::Below,
            ))
        }
        // Along the top of the stack it goes before, or under the last of them.
        Stop::NewStack {
            before: Some(before),
        } => {
            let row = (rows.iter()).position(|row| {
                matches!(row, Row::Item(Selection::Branch(p, Some(name)) | Selection::Stack(p, Some(name)))
                    if p == project && name == before)
            })?;
            Some(along(stack_start(rows, row), Place::Above))
        }
        // Where commits go, the zone for them lights up instead.
        Stop::NewStack { before: None }
            if (rows.iter()).any(|row| matches!(row, Row::NewBranch(p) if p == project)) =>
        {
            None
        }
        Stop::NewStack { before: None } => {
            let last = (rows.iter().enumerate())
                .rev()
                .find_map(|(ix, row)| match row {
                    Row::Item(Selection::Branch(p, Some(_)))
                        if p == project && in_main(rows, ix) =>
                    {
                        Some(ix)
                    }
                    _ => None,
                })?;
            Some(Line {
                inset_of: stack_start(rows, last),
                ..along(branch_end(rows, last) - 1, Place::Below)
            })
        }
        Stop::Into(_) | Stop::Card(_) | Stop::Worktree(_) | Stop::Terminal { .. } => None,
    }
}

/// The line between two rows, for its caller to place.
fn divider(color: Hsla) -> Div {
    div().absolute().h(dp(2.)).rounded_full().bg(color).child(
        div()
            .absolute()
            .left(dp(-4.))
            .top(dp(-2.))
            .size(dp(6.))
            .rounded_full()
            .bg(color),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn repository_groups_keep_branch_drop_targets_inside_their_project() {
        use super::Row;
        use crate::state::Selection;
        let rows = [
            Row::Item(Selection::Project("alpha".into())),
            Row::Item(Selection::Branch("alpha".into(), None)),
            Row::Item(Selection::Project("beta".into())),
            Row::Item(Selection::Branch("beta".into(), None)),
        ];
        assert_eq!(
            super::branch_end(&rows, 1),
            2,
            "another repository ends the branch even without a gap between cards"
        );
    }

    use std::sync::Arc;

    use super::{
        Aim, Line, Row, divider_at, drag_zone, over_own_row, stack_end, stack_start, stop_at,
    };
    use crate::backend::model::build::{full, id};
    use crate::backend::{Checkout, CommitId, Destination};
    use crate::state::{ProjectId, Selection};
    use crate::transfer::{Changes, Moving, Place, Stop, Target};
    use crate::ui::ROW_HEIGHT;

    fn project() -> ProjectId {
        ProjectId::from("p")
    }

    fn branch(name: &str) -> Row {
        let name = full(&format!("refs/heads/{name}"));
        Row::Item(Selection::Branch(project(), Some(name)))
    }

    fn commit(n: u8) -> Row {
        Row::Item(Selection::Commit(project(), id(n)))
    }

    fn commits(ids: &[CommitId]) -> Moving {
        Moving::Commits(Arc::from(ids))
    }

    fn top_of(name: &str) -> Stop {
        Stop::In(Destination::Branch(full(&format!("refs/heads/{name}"))))
    }

    fn below(n: u8) -> Stop {
        Stop::In(Destination::BelowCommit(id(n)))
    }

    /// The pointer `depth` down plain rows.
    fn aim(depth: f32) -> Aim {
        Aim {
            depth,
            row: ROW_HEIGHT,
        }
    }

    /// How far down row `row` of a list of plain rows, `y` of the way through it.
    fn depth(row: f32, y: f32) -> f32 {
        (row + y) * ROW_HEIGHT
    }

    #[test]
    fn a_commits_middle_takes_a_squash_and_its_edges_a_move() {
        let rows = Vec::from([branch("ui"), commit(3), commit(2), commit(1)]);
        let dragged = commits(&[id(1)]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(2., 0.5))),
            Some(Stop::Into(Target::Commit(id(2)))),
            "its middle goes into it"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(2., 0.1))),
            Some(below(3)),
            "its top quarter goes between it and the commit above"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(2., 0.9))),
            Some(below(2)),
            "its bottom quarter goes below it"
        );
    }

    #[test]
    fn the_pointer_over_no_stop_sends_what_is_dragged_back() {
        let rows = Vec::from([branch("ui"), commit(3), Row::WorktreeDivider, branch("api")]);
        let line = 2. * ROW_HEIGHT + 6.;
        assert_eq!(
            drag_zone(&rows, &project(), &commits(&[id(9)]), aim(line), |_, _| {
                true
            }),
            None,
            "with nothing to land on, no divider stays behind away from the pointer"
        );
    }

    #[test]
    fn the_pointer_over_its_own_row_sends_what_is_dragged_back() {
        let rows = Vec::from([branch("ui"), commit(3), commit(2)]);
        assert_eq!(
            drag_zone(
                &rows,
                &project(),
                &commits(&[id(3)]),
                aim(depth(1., 0.5)),
                |_, _| false
            ),
            None,
            "where it started, nothing lands"
        );
    }

    #[test]
    fn files_over_the_commit_they_came_from_go_back() {
        let rows = Vec::from([branch("ui"), commit(3)]);
        assert_eq!(
            drag_zone(
                &rows,
                &project(),
                &files(Some(id(3))),
                aim(depth(1., 0.5)),
                |_, _| true
            ),
            None,
            "where they start is listed as a stop, but going there lands nothing"
        );
    }

    #[test]
    fn the_gap_right_under_its_own_row_sends_what_is_dragged_back() {
        let rows = Vec::from([branch("ui"), commit(3), commit(2)]);
        assert_eq!(
            drag_zone(
                &rows,
                &project(),
                &commits(&[id(3)]),
                aim(depth(2., 0.2)),
                |_, _| false
            ),
            None,
            "that's where it is, not somewhere to land"
        );
    }

    #[test]
    fn a_stop_that_takes_nothing_is_no_zone_of_its_own() {
        let rows = Vec::from([branch("ui")]);
        assert_eq!(
            drag_zone(
                &rows,
                &project(),
                &commits(&[id(9)]),
                aim(depth(0., 0.5)),
                |_, _| false
            ),
            None,
            "as the default branch's heading takes no commit"
        );
    }

    #[test]
    fn a_commits_own_row_is_over_its_own_row() {
        let rows = Vec::from([branch("ui"), commit(3), commit(2)]);
        assert!(
            over_own_row(
                &rows,
                &project(),
                &commits(&[id(3)]),
                depth(1., 0.5),
                ROW_HEIGHT
            ),
            "where it started, it goes back"
        );
        assert!(
            !over_own_row(
                &rows,
                &project(),
                &commits(&[id(3)]),
                depth(2., 0.5),
                ROW_HEIGHT
            ),
            "the next commit is another's"
        );
    }

    #[test]
    fn the_gap_under_a_heading_is_the_top_of_its_branch() {
        let rows = Vec::from([branch("ui"), commit(3)]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), aim(depth(1., 0.1))),
            Some(top_of("ui")),
            "above a branch's first commit is its top"
        );
    }

    #[test]
    fn a_heading_takes_a_commit_at_the_top_of_its_branch() {
        let rows = Vec::from([branch("ui"), commit(3)]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), aim(depth(0., 0.5))),
            Some(top_of("ui")),
            "never a new branch, whose stop would share the gap"
        );
    }

    #[test]
    fn the_zone_takes_a_commit_in_a_new_stack() {
        let rows = Vec::from([branch("ui"), Row::NewBranch(project())]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), aim(depth(1., 0.5))),
            Some(Stop::NewStack { before: None }),
            "the one new branch a drag makes"
        );
    }

    #[test]
    fn a_worktrees_heading_takes_a_commit_into_its_changes() {
        let rows = Vec::from([Row::Item(Selection::Checkout(project(), Checkout::Main))]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), aim(depth(0., 0.5))),
            Some(Stop::Into(Target::Uncommitted(Checkout::Main))),
            "it uncommits there, with changes listed under it or not"
        );
    }

    #[test]
    fn a_project_row_takes_a_commit_into_its_changes_only_without_a_main_heading() {
        let project_row = || Row::Item(Selection::Project(project()));
        let alone = Vec::from([project_row(), branch("ui")]);
        assert_eq!(
            stop_at(&alone, &project(), &commits(&[id(9)]), aim(depth(0., 0.2))),
            Some(Stop::Into(Target::Uncommitted(Checkout::Main))),
            "it stands in for the main checkout's heading, which its card leaves out"
        );
        let main = Row::Item(Selection::Checkout(project(), Checkout::Main));
        let headed = Vec::from([project_row(), main]);
        assert_eq!(
            stop_at(&headed, &project(), &commits(&[id(9)]), aim(depth(0., 0.2))),
            None,
            "beside other checkouts the main one's own heading takes it"
        );
    }

    #[test]
    fn a_divider_below_a_commit_is_along_its_bottom() {
        let rows = Vec::from([branch("ui"), commit(3), commit(2)]);
        assert_eq!(
            divider_at(&rows, &project(), &below(3)).map(|line| (line.row, line.place)),
            Some((1, Place::Below)),
            "the row it goes under keeps its place"
        );
    }

    #[test]
    fn a_divider_at_the_top_of_a_branch_is_above_its_first_commit() {
        let upstream = Row::Item(Selection::Upstream(
            project(),
            full("refs/remotes/origin/ui"),
            id(8),
        ));
        let rows = Vec::from([branch("ui"), upstream, commit(3)]);
        assert_eq!(
            divider_at(&rows, &project(), &top_of("ui")).map(|line| (line.row, line.place)),
            Some((2, Place::Above)),
            "what's listed before its commits stays above where it lands"
        );
    }

    #[test]
    fn a_divider_at_the_top_of_a_folded_branch_is_under_its_heading() {
        let rows = Vec::from([branch("ui"), Row::Between(project()), branch("api")]);
        assert_eq!(
            divider_at(&rows, &project(), &top_of("ui")).map(|line| (line.row, line.place)),
            Some((0, Place::Below)),
            "with no commit listed, the heading is all there is to go by"
        );
    }

    #[test]
    fn a_divider_above_a_branch_is_along_the_top_of_its_heading() {
        let rows = Vec::from([branch("ui"), commit(3), branch("api"), commit(1)]);
        let at = Stop::NewBranch {
            beside: full("refs/heads/api"),
            below: false,
        };
        assert_eq!(
            divider_at(&rows, &project(), &at).map(|line| (line.row, line.place)),
            Some((2, Place::Above)),
            "between the two branches"
        );
    }

    #[test]
    fn a_divider_below_a_branch_is_under_its_last_row() {
        let more = Row::More(project(), Some(full("refs/heads/ui")), 4);
        let rows = Vec::from([
            branch("ui"),
            commit(3),
            more,
            Row::Between(project()),
            branch("api"),
        ]);
        let at = Stop::NewBranch {
            beside: full("refs/heads/ui"),
            below: true,
        };
        assert_eq!(
            divider_at(&rows, &project(), &at).map(|line| (line.row, line.place)),
            Some((2, Place::Below)),
            "under the row that says more follows too, which is no commit"
        );
    }

    #[test]
    fn a_stop_with_no_gap_shows_no_divider() {
        let rows = Vec::from([branch("ui"), commit(3), Row::NewBranch(project())]);
        assert_eq!(
            divider_at(&rows, &project(), &Stop::Into(Target::Commit(id(3)))),
            None,
            "what goes into a row is outlined"
        );
        assert_eq!(
            divider_at(&rows, &project(), &Stop::NewStack { before: None }),
            None,
            "the zone lights up by itself"
        );
    }

    #[test]
    fn a_divider_is_in_the_project_of_the_move() {
        let other = Row::Item(Selection::Branch(
            ProjectId::from("q"),
            Some(full("refs/heads/ui")),
        ));
        let rows = Vec::from([other, branch("ui"), commit(3)]);
        assert_eq!(
            divider_at(&rows, &project(), &top_of("ui")).map(|line| (line.row, line.place)),
            Some((2, Place::Above)),
            "the same branch name in another project is another row"
        );
    }

    fn files(from: Option<CommitId>) -> Moving {
        Moving::Files(Changes {
            from,
            worktree: Checkout::Main,
            files: Arc::from([]),
        })
    }

    #[test]
    fn all_of_a_commit_takes_files_into_it() {
        let rows = Vec::from([branch("ui"), commit(3)]);
        assert_eq!(
            stop_at(&rows, &project(), &files(None), aim(depth(1., 0.1))),
            Some(Stop::Into(Target::Commit(id(3)))),
            "files only go into a commit, never between two"
        );
    }

    #[test]
    fn a_branch_takes_uncommitted_files_at_its_top() {
        let rows = Vec::from([branch("ui")]);
        assert_eq!(
            stop_at(&rows, &project(), &files(None), aim(depth(0., 0.5))),
            Some(top_of("ui")),
            "a commit of them starts there"
        );
    }

    #[test]
    fn a_branch_takes_no_files_of_a_commit() {
        let rows = Vec::from([branch("ui")]);
        assert_eq!(
            stop_at(&rows, &project(), &files(Some(id(1))), aim(depth(0., 0.5))),
            None,
            "they go into a commit, not onto a branch"
        );
    }

    #[test]
    fn the_zone_takes_no_files() {
        let rows = Vec::from([Row::NewBranch(project())]);
        assert_eq!(
            stop_at(&rows, &project(), &files(None), aim(depth(0., 0.5))),
            None,
            "they make no new branch"
        );
    }

    #[test]
    fn a_worktrees_heading_takes_a_commits_files_into_its_changes() {
        let rows = Vec::from([Row::Item(Selection::Checkout(project(), Checkout::Main))]);
        assert_eq!(
            stop_at(&rows, &project(), &files(Some(id(1))), aim(depth(0., 0.5))),
            Some(Stop::Into(Target::Uncommitted(Checkout::Main))),
            "as a commit does"
        );
    }

    #[test]
    fn another_projects_rows_are_no_stop() {
        let other = Row::Item(Selection::Commit(ProjectId::from("q"), id(2)));
        assert_eq!(
            stop_at(
                &[other],
                &project(),
                &commits(&[id(9)]),
                aim(depth(0., 0.5))
            ),
            None,
            "a move stays in its project"
        );
    }

    fn above(name: &str) -> Stop {
        Stop::NewBranch {
            beside: full(&format!("refs/heads/{name}")),
            below: false,
        }
    }

    fn under(name: &str) -> Stop {
        Stop::NewBranch {
            beside: full(&format!("refs/heads/{name}")),
            below: true,
        }
    }

    fn apart_before(name: Option<&str>) -> Stop {
        Stop::NewStack {
            before: name.map(|name| full(&format!("refs/heads/{name}"))),
        }
    }

    #[test]
    fn a_branch_stacks_onto_one_from_its_middle_and_goes_beside_it_from_its_edges() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui")]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(0., 0.5))),
            Some(above("ui")),
            "dropped on it, it's stacked on top of it"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(0., 0.1))),
            Some(apart_before(Some("ui"))),
            "at its top edge, it's a stack of its own above"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(0., 0.9))),
            Some(apart_before(None)),
            "at its bottom edge, the last, a stack of its own after them all"
        );
    }

    #[test]
    fn an_open_branch_and_its_commits_are_one_target() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui"), commit(3), commit(2), commit(1)]);
        for at in [depth(0., 0.9), depth(2., 0.5), depth(3., 0.5)] {
            assert_eq!(
                stop_at(&rows, &project(), &dragged, aim(at)),
                Some(above("ui")),
                "over its commits it's stacked onto it, as over its heading"
            );
        }
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(3., 0.9))),
            Some(apart_before(None)),
            "under its last commit is the gap below it"
        );
    }

    #[test]
    fn between_two_stacks_a_branch_is_a_stack_of_its_own_from_either_side() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui"), Row::Between(project()), branch("api")]);
        for at in [depth(0., 0.9), depth(1., 0.1)] {
            assert_eq!(
                stop_at(&rows, &project(), &dragged, aim(at)),
                Some(apart_before(Some("api"))),
                "the gap between them is one place, whichever side it's aimed from"
            );
        }
    }

    #[test]
    fn between_two_branches_of_a_stack_a_branch_is_stacked_there() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let caption = Row::Item(Selection::Stack(project(), Some(full("refs/heads/api"))));
        let rows = Vec::from([caption, branch("ui"), commit(3), branch("api"), commit(1)]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(2., 0.9))),
            Some(above("api")),
            "the gap between them, as the keys list it"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(3., 0.1))),
            Some(above("api")),
            "from either side of it"
        );
    }

    #[test]
    fn a_stacks_caption_aims_at_the_top_of_its_top_branch() {
        let caption = Row::Item(Selection::Stack(project(), Some(full("refs/heads/api"))));
        let rows = Vec::from([caption, branch("ui"), commit(3), branch("api"), commit(1)]);
        let on_caption = depth(0., 0.5);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), aim(on_caption)),
            Some(top_of("ui")),
            "commits dropped on it go to the top of the stack"
        );
        assert_eq!(
            stop_at(
                &rows,
                &project(),
                &Moving::Branch(full("refs/heads/x")),
                aim(on_caption)
            ),
            Some(apart_before(Some("ui"))),
            "a branch dropped on it goes above the stack, in one of its own"
        );
        let top = Moving::Branch(full("refs/heads/ui"));
        assert_eq!(
            drag_zone(&rows, &project(), &top, aim(on_caption), |_, _| true),
            None,
            "the top branch itself, dragged over its caption, goes back where it is"
        );
    }

    #[test]
    fn a_linked_worktrees_lane_takes_a_branch_anywhere_in_it() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let heading = Row::Item(Selection::Checkout(
            project(),
            Checkout::Linked("wt".into()),
        ));
        let rows = Vec::from([heading, branch("agent")]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(1., 0.1))),
            Some(above("agent")),
            "a lane is one stack, which has no others to go between"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, aim(depth(1., 0.9))),
            Some(under("agent")),
            "below it too"
        );
    }

    #[test]
    fn a_stack_of_its_own_shows_along_the_stack_it_goes_before() {
        let caption = Row::Item(Selection::Stack(project(), Some(full("refs/heads/api"))));
        let rows = Vec::from([
            branch("x"),
            Row::Between(project()),
            caption,
            branch("ui"),
            branch("api"),
        ]);
        let line = |at: &Stop| divider_at(&rows, &project(), at).unwrap();
        assert_eq!(
            line(&apart_before(Some("api"))),
            Line {
                row: 2,
                place: Place::Above,
                inset_of: 2
            },
            "above its caption, as far in"
        );
        assert_eq!(
            line(&apart_before(None)),
            Line {
                row: 4,
                place: Place::Below,
                inset_of: 2
            },
            "after them all, under the last, as far in as its caption"
        );
        assert_eq!(
            line(&above("api")),
            Line {
                row: 4,
                place: Place::Above,
                inset_of: 4
            },
            "in a stack, as far in as its branches"
        );
    }

    #[test]
    fn a_stack_is_its_rows_from_its_caption_to_its_last_commit() {
        let caption = Row::Item(Selection::Stack(project(), Some(full("refs/heads/api"))));
        let rows = Vec::from([
            branch("x"),
            commit(9),
            Row::Between(project()),
            caption,
            branch("ui"),
            commit(3),
            branch("api"),
            commit(1),
        ]);
        assert_eq!(
            (stack_start(&rows, 6), stack_end(&rows, 6)),
            (3, 8),
            "from the caption over it to its end"
        );
        assert_eq!(
            (stack_start(&rows, 1), stack_end(&rows, 0)),
            (0, 2),
            "a branch alone is a stack of its rows"
        );
    }
}
