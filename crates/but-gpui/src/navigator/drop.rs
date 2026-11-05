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
use super::rows::{Row, span_height};
use crate::backend::{CommitId, Destination, FullName, Messages};
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::transfer::{Changes, Dragged, Move, Moving, Op, Place, Stop, Target};
use crate::ui::{Icon, dp, h_flex, small_icon};

impl Navigator {
    /// Let a row offer its right-click menu.
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
        let (at, op) = match drag_zone(&self.rows, &project, &what, depth, takes) {
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
    /// among them, so no row moves for it.
    pub(super) fn drag_divider(&self, theme: &Theme, cx: &App) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let moving = (store.moving.as_ref())
            .filter(|moving| moving.dragged(store) && moving.lands(store))?;
        let (ix, place) = divider_at(&self.rows, &moving.project, &moving.at)?;
        let row = self.list.bounds_for_item(ix)?;
        let edge = match place == Place::Above {
            true => row.top(),
            false => row.bottom(),
        };
        let inset = self.insets.get(ix).copied().unwrap_or_default();
        let (left, right) = self.card_margins();
        let line = divider(moving.op.colour(theme))
            .left(dp(left + inset + 10.))
            .right(dp(right + 10.))
            .top(edge - self.list.viewport_bounds().top() - theme.scaled(1.));
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

    /// Where a commit being dragged lands in a new branch of its own, or a stacked branch in a
    /// stack of its own: a dashed box, lit while a drag is over it. Padded, not margined: a
    /// margin collapses through the card's ground.
    pub(super) fn new_branch_zone(
        &self,
        ix: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let highlight = theme.pop;
        let dragging = self.store.read(cx).dragging.as_ref();
        let label = match dragging.map(|dragged| &dragged.what) {
            Some(Moving::Branch(_)) => "Unstack",
            _ => "New independent branch",
        };
        let zone = h_flex()
            .id(("new-branch-drop", ix))
            .h(dp(30.))
            .mx(dp(3.))
            .justify_center()
            .gap(dp(6.))
            .rounded(dp(4.))
            .border_1()
            .border_dashed()
            .border_color(theme.border_2)
            .text_size(dp(12.))
            .text_color(theme.text_2)
            .child(small_icon(Icon::Branch).text_color(theme.text_2))
            .child(label)
            .drag_over::<Dragged>(move |style, _, _, _| {
                style
                    .bg(highlight.opacity(0.14))
                    .border_color(highlight)
                    .text_color(highlight)
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

/// Where the pointer `depth` down `rows` lands `what` of `project`, as what, where `takes` says
/// what lands where. Nowhere, what's dragged goes back where it is, over its own rows too.
fn drag_zone(
    rows: &[Row],
    project: &ProjectId,
    what: &Moving,
    depth: f32,
    takes: impl Fn(&Stop, Op) -> bool,
) -> Option<(Stop, Op)> {
    if over_own_row(rows, project, what, depth) {
        return None;
    }
    let at = stop_at(rows, project, what, depth)?;
    let op = op_at(what, &at);
    takes(&at, op).then_some((at, op))
}

/// The stop `what` of `project` aims at `depth` down `rows`. Commits go between rows from the top
/// and bottom quarter of a commit, into it between, and to the top of a branch from its heading.
/// They make no new branch but the zone's, whose stops would share a gap with others. A branch
/// goes where the pointer is, above another from the top half of it and its commits and below it
/// from the bottom half, either side of the line between stacks too. Files go into a commit or a
/// worktree's changes, whichever row they're over, and a worktree's changes onto a branch start a
/// commit there; they make no new branch.
fn stop_at(rows: &[Row], project: &ProjectId, what: &Moving, depth: f32) -> Option<Stop> {
    let (ix, y) = row_at(rows, depth)?;
    match what {
        Moving::Commits(_) => commit_stop(rows, ix, y, project),
        Moving::Branch(_) => branch_stop(rows, ix, y, project),
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

fn file_stop(rows: &[Row], ix: usize, project: &ProjectId, from: Option<CommitId>) -> Option<Stop> {
    match rows.get(ix)? {
        Row::Item(Selection::Commit(p, id)) if p == project => {
            Some(Stop::Into(Target::Commit(*id)))
        }
        Row::Item(Selection::Uncommitted(p, checkout) | Selection::Checkout(p, checkout))
            if p == project =>
        {
            Some(Stop::Into(Target::Uncommitted(checkout.clone())))
        }
        Row::Item(Selection::Branch(p, Some(name))) if p == project && from.is_none() => {
            Some(Stop::In(Destination::Branch(name.clone())))
        }
        _ => None,
    }
}

/// The row `depth` down `rows`, and how far down that row, from 0 at its top to 1 at its bottom.
fn row_at(rows: &[Row], depth: f32) -> Option<(usize, f32)> {
    let mut top = 0.;
    for (ix, row) in rows.iter().enumerate() {
        let height = row.height();
        if depth < top + height {
            return Some((ix, (depth - top) / height));
        }
        top += height;
    }
    None
}

/// Whether `depth` down `rows` is over what's moving, where it started.
fn over_own_row(rows: &[Row], project: &ProjectId, what: &Moving, depth: f32) -> bool {
    let row = row_at(rows, depth).and_then(|(ix, _)| rows.get(ix));
    matches!(row, Some(Row::Item(item)) if what.holds(project, item))
}

fn commit_stop(rows: &[Row], ix: usize, y: f32, project: &ProjectId) -> Option<Stop> {
    match rows.get(ix)? {
        Row::NewBranch(p) if p == project => Some(Stop::NewStack { first: false }),
        Row::Item(Selection::Uncommitted(p, checkout) | Selection::Checkout(p, checkout))
            if p == project =>
        {
            Some(Stop::Into(Target::Uncommitted(checkout.clone())))
        }
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

/// The stop between row `ix`, a commit, and what's listed above it in its branch.
fn gap_above(rows: &[Row], ix: usize, project: &ProjectId) -> Option<Stop> {
    let above = rows.get(..ix)?.iter().rev().find_map(|row| match row {
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

fn branch_stop(rows: &[Row], ix: usize, y: f32, project: &ProjectId) -> Option<Stop> {
    let row = rows.get(ix)?;
    match row {
        Row::NewBranch(p) if p == project => return Some(Stop::NewStack { first: false }),
        Row::Between(p) if p == project && y >= 0.5 => {
            let (_, next) = next_branch(rows, ix + 1, project)?;
            return Some(new_branch(next, false));
        }
        Row::Between(p) if p == project => {
            let (_, above) = branch_above(rows, ix, project)?;
            return Some(new_branch(above, true));
        }
        Row::Item(Selection::Branch(p, Some(_)) | Selection::Commit(p, _)) if p == project => {}
        _ => return None,
    }
    let (heading, name) = branch_above(rows, ix + 1, project)?;
    let end = branch_end(rows, heading);
    let down = span_height(rows, heading..ix) + y * row.height();
    if down < span_height(rows, heading..end) / 2. {
        return Some(new_branch(name, false));
    }
    // The stop below it is above the branch under it in its stack, or its stack's end.
    Some(match rows.get(end) {
        Some(Row::Item(Selection::Branch(p, Some(next)))) if p == project => {
            new_branch(next, false)
        }
        _ => new_branch(name, true),
    })
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

/// The first named branch of `project` listed from row `start`, by its row.
fn next_branch<'a>(
    rows: &'a [Row],
    start: usize,
    project: &ProjectId,
) -> Option<(usize, &'a FullName)> {
    (rows.iter().enumerate().skip(start)).find_map(|(at, row)| match row {
        Row::Item(Selection::Branch(p, Some(name))) if p == project => Some((at, name)),
        _ => None,
    })
}

/// Whether `row` comes after a branch's commits rather than among them.
fn ends_branch(row: Option<&Row>) -> bool {
    matches!(
        row,
        Some(
            Row::Item(Selection::Branch(..))
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

/// The row of `project` whose edge shows the stop `at`, and which edge. A stop that goes into a
/// row has no gap to show, nor one that's a zone of its own, which lights up by itself.
fn divider_at(rows: &[Row], project: &ProjectId, at: &Stop) -> Option<(usize, Place)> {
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
    match at {
        Stop::In(Destination::AboveCommit(id)) => Some((commit(id)?, Place::Above)),
        Stop::In(Destination::BelowCommit(id)) => Some((commit(id)?, Place::Below)),
        // Above its first commit, past what's listed before its commits.
        Stop::In(Destination::Branch(name)) => {
            let heading = heading(name)?;
            let first = (heading + 1..branch_end(rows, heading)).find(|ix| {
                matches!(rows.get(*ix), Some(Row::Item(Selection::Commit(p, _))) if p == project)
            });
            Some(first.map_or((heading, Place::Below), |ix| (ix, Place::Above)))
        }
        Stop::NewBranch {
            beside,
            below: false,
        } => Some((heading(beside)?, Place::Above)),
        Stop::NewBranch {
            beside,
            below: true,
        } => Some((branch_end(rows, heading(beside)?) - 1, Place::Below)),
        Stop::Into(_)
        | Stop::NewStack { .. }
        | Stop::Card(_)
        | Stop::Worktree(_)
        | Stop::Terminal { .. } => None,
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
    use std::sync::Arc;

    use super::{Row, divider_at, drag_zone, over_own_row, stop_at};
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

    /// How far down row `row` of a list of plain rows, `y` of the way through it.
    fn depth(row: f32, y: f32) -> f32 {
        (row + y) * ROW_HEIGHT
    }

    #[test]
    fn a_commits_middle_takes_a_squash_and_its_edges_a_move() {
        let rows = Vec::from([branch("ui"), commit(3), commit(2), commit(1)]);
        let dragged = commits(&[id(1)]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(2., 0.5)),
            Some(Stop::Into(Target::Commit(id(2)))),
            "its middle goes into it"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(2., 0.1)),
            Some(below(3)),
            "its top quarter goes between it and the commit above"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(2., 0.9)),
            Some(below(2)),
            "its bottom quarter goes below it"
        );
    }

    #[test]
    fn the_line_between_stacks_is_no_stop_for_a_commit() {
        let rows = Vec::from([
            branch("ui"),
            commit(3),
            Row::Between(project()),
            branch("api"),
        ]);
        let line = 2. * ROW_HEIGHT + 6.;
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), line),
            None,
            "what's dragged goes back where it is there"
        );
    }

    #[test]
    fn the_pointer_over_no_stop_sends_what_is_dragged_back() {
        let rows = Vec::from([
            branch("ui"),
            commit(3),
            Row::Between(project()),
            branch("api"),
        ]);
        let line = 2. * ROW_HEIGHT + 6.;
        assert_eq!(
            drag_zone(&rows, &project(), &commits(&[id(9)]), line, |_, _| { true }),
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
                depth(1., 0.5),
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
                depth(1., 0.5),
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
                depth(2., 0.2),
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
                depth(0., 0.5),
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
            over_own_row(&rows, &project(), &commits(&[id(3)]), depth(1., 0.5)),
            "where it started, it goes back"
        );
        assert!(
            !over_own_row(&rows, &project(), &commits(&[id(3)]), depth(2., 0.5)),
            "the next commit is another's"
        );
    }

    #[test]
    fn the_gap_under_a_heading_is_the_top_of_its_branch() {
        let rows = Vec::from([branch("ui"), commit(3)]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), depth(1., 0.1)),
            Some(top_of("ui")),
            "above a branch's first commit is its top"
        );
    }

    #[test]
    fn a_heading_takes_a_commit_at_the_top_of_its_branch() {
        let rows = Vec::from([branch("ui"), commit(3)]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), depth(0., 0.5)),
            Some(top_of("ui")),
            "never a new branch, whose stop would share the gap"
        );
    }

    #[test]
    fn the_zone_takes_a_commit_in_a_new_stack() {
        let rows = Vec::from([branch("ui"), Row::NewBranch(project())]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), depth(1., 0.5)),
            Some(Stop::NewStack { first: false }),
            "the one new branch a drag makes"
        );
    }

    #[test]
    fn a_worktrees_heading_takes_a_commit_into_its_changes() {
        let rows = Vec::from([Row::Item(Selection::Checkout(project(), Checkout::Main))]);
        assert_eq!(
            stop_at(&rows, &project(), &commits(&[id(9)]), depth(0., 0.5)),
            Some(Stop::Into(Target::Uncommitted(Checkout::Main))),
            "it uncommits there, with changes listed under it or not"
        );
    }

    #[test]
    fn a_divider_below_a_commit_is_along_its_bottom() {
        let rows = Vec::from([branch("ui"), commit(3), commit(2)]);
        assert_eq!(
            divider_at(&rows, &project(), &below(3)),
            Some((1, Place::Below)),
            "the row it goes under keeps its place"
        );
    }

    #[test]
    fn a_divider_at_the_top_of_a_branch_is_above_its_first_commit() {
        let upstream = Row::Item(Selection::Upstream(
            project(),
            Some(full("refs/remotes/origin/ui")),
            id(8),
        ));
        let rows = Vec::from([branch("ui"), upstream, commit(3)]);
        assert_eq!(
            divider_at(&rows, &project(), &top_of("ui")),
            Some((2, Place::Above)),
            "what's listed before its commits stays above where it lands"
        );
    }

    #[test]
    fn a_divider_at_the_top_of_a_folded_branch_is_under_its_heading() {
        let rows = Vec::from([branch("ui"), Row::Between(project()), branch("api")]);
        assert_eq!(
            divider_at(&rows, &project(), &top_of("ui")),
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
            divider_at(&rows, &project(), &at),
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
            divider_at(&rows, &project(), &at),
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
            divider_at(&rows, &project(), &Stop::NewStack { first: false }),
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
            divider_at(&rows, &project(), &top_of("ui")),
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
            stop_at(&rows, &project(), &files(None), depth(1., 0.1)),
            Some(Stop::Into(Target::Commit(id(3)))),
            "files only go into a commit, never between two"
        );
    }

    #[test]
    fn a_branch_takes_uncommitted_files_at_its_top() {
        let rows = Vec::from([branch("ui")]);
        assert_eq!(
            stop_at(&rows, &project(), &files(None), depth(0., 0.5)),
            Some(top_of("ui")),
            "a commit of them starts there"
        );
    }

    #[test]
    fn a_branch_takes_no_files_of_a_commit() {
        let rows = Vec::from([branch("ui")]);
        assert_eq!(
            stop_at(&rows, &project(), &files(Some(id(1))), depth(0., 0.5)),
            None,
            "they go into a commit, not onto a branch"
        );
    }

    #[test]
    fn the_zone_takes_no_files() {
        let rows = Vec::from([Row::NewBranch(project())]);
        assert_eq!(
            stop_at(&rows, &project(), &files(None), depth(0., 0.5)),
            None,
            "they make no new branch"
        );
    }

    #[test]
    fn a_worktrees_heading_takes_a_commits_files_into_its_changes() {
        let rows = Vec::from([Row::Item(Selection::Checkout(project(), Checkout::Main))]);
        assert_eq!(
            stop_at(&rows, &project(), &files(Some(id(1))), depth(0., 0.5)),
            Some(Stop::Into(Target::Uncommitted(Checkout::Main))),
            "as a commit does"
        );
    }

    #[test]
    fn another_projects_rows_are_no_stop() {
        let other = Row::Item(Selection::Commit(ProjectId::from("q"), id(2)));
        assert_eq!(
            stop_at(&[other], &project(), &commits(&[id(9)]), depth(0., 0.5)),
            None,
            "a move stays in its project"
        );
    }

    #[test]
    fn a_branch_goes_above_one_from_the_top_of_its_heading() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui"), commit(3)]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(0., 0.2)),
            Some(Stop::NewBranch {
                beside: full("refs/heads/ui"),
                below: false
            }),
            "stacked on top of it"
        );
    }

    #[test]
    fn a_branch_goes_above_an_open_one_from_all_of_its_heading() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui"), commit(3), commit(2), commit(1)]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(0., 0.9)),
            Some(Stop::NewBranch {
                beside: full("refs/heads/ui"),
                below: false
            }),
            "below it is rows away from the pointer, under its commits"
        );
    }

    #[test]
    fn a_branch_goes_below_a_folded_one_from_the_bottom_of_its_heading() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui")]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(0., 0.9)),
            Some(Stop::NewBranch {
                beside: full("refs/heads/ui"),
                below: true
            }),
            "with nothing under it, that's right where the pointer is"
        );
    }

    #[test]
    fn the_line_between_stacks_splits_a_branch_either_side_of_it() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui"), Row::Between(project()), branch("api")]);
        let line = 12.;
        assert_eq!(
            stop_at(&rows, &project(), &dragged, ROW_HEIGHT + 0.25 * line),
            Some(Stop::NewBranch {
                beside: full("refs/heads/ui"),
                below: true
            }),
            "above the line it joins the stack that ends there"
        );
        assert_eq!(
            stop_at(&rows, &project(), &dragged, ROW_HEIGHT + 0.75 * line),
            Some(Stop::NewBranch {
                beside: full("refs/heads/api"),
                below: false
            }),
            "below it, the stack that starts there"
        );
    }

    #[test]
    fn a_branch_goes_above_the_next_one_in_its_stack_from_lower_down() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([branch("ui"), commit(3), branch("api"), commit(1)]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(1., 0.5)),
            Some(Stop::NewBranch {
                beside: full("refs/heads/api"),
                below: false
            }),
            "the stop between them, as the keys list it"
        );
    }

    #[test]
    fn a_branch_goes_below_the_last_one_of_its_stack() {
        let dragged = Moving::Branch(full("refs/heads/x"));
        let rows = Vec::from([
            branch("ui"),
            commit(3),
            Row::Between(project()),
            branch("api"),
        ]);
        assert_eq!(
            stop_at(&rows, &project(), &dragged, depth(1., 0.5)),
            Some(Stop::NewBranch {
                beside: full("refs/heads/ui"),
                below: true
            }),
            "the line says the stack ends there"
        );
    }
}
