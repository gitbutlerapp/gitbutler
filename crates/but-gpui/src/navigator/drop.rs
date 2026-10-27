//! Dropping what's dragged on the navigator's rows: where on a row it lands, the line or
//! outline that shows it, the zone for a new branch, and scrolling while it's near an edge.

use std::time::Duration;

use gpui::{
    AnyElement, Bounds, Context, Div, DragMoveEvent, Hsla, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, PlatformInput, Point, Window, deferred, div, prelude::*,
};

use super::Navigator;
use crate::state::{Cut, ProjectId, Selection};
use crate::theme::Theme;
use crate::transfer::{Place, Transfer};
use crate::ui::{Icon, dp, fixed, h_flex, small_icon};

impl Navigator {
    /// Make a row take what's dragged or cut where it has a meaning for it, and offer its
    /// right-click menu. While dragging, the row under the pointer shows where letting go
    /// lands: a line above or below a commit, or an outline around what it goes into. While
    /// something is cut, the selected row shows where ⌘V puts it. Rows it can't go on step back.
    pub(super) fn drop_target(
        &self,
        el: AnyElement,
        target: Selection,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let dragging = cx.has_active_drag();
        // What a drop or a paste would carry: the drag while there is one, else the cut.
        let pending = match dragging {
            true => store.dragging.as_ref(),
            false => store.cut.as_ref(),
        };
        let lands = |place: Place| {
            pending
                .is_some_and(|payload| Transfer::resolve(payload, &target, place, store).is_some())
        };
        let landing = match dragging {
            true => (store.drop_at.as_ref())
                .filter(|(at, _)| *at == target)
                .map(|(_, place)| *place),
            false => (pending.filter(|_| store.selection.as_ref() == Some(&target)))
                .map(|cut| Place::from_keys(window.modifiers(), cut, &target)),
        }
        .filter(|place| lands(*place))
        // A branch goes below another under its last commit, rows further down, so a line
        // under its heading would point at the wrong place: the heading is outlined, as the
        // pill says where.
        .map(|place| match target {
            Selection::Branch(..) => Place::Into,
            _ => place,
        });
        let steps_back = pending.is_some()
            && ![Place::Above, Place::Into, Place::Below]
                .into_iter()
                .any(lands);
        let highlight = theme.pop;
        let (store, move_target, can_drop_target, menu_target) = (
            self.store.clone(),
            target.clone(),
            target.clone(),
            target.clone(),
        );
        div()
            .relative()
            .rounded(dp(4.))
            .child(el)
            .when(steps_back, |el| el.opacity(0.35))
            .map(|el| match landing {
                Some(Place::Into) => el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(dp(4.))
                        .border_1()
                        .border_color(highlight)
                        .bg(highlight.opacity(0.14)),
                ),
                // Painted last, as the next row's slice of the card would cover what of it
                // crosses into that row.
                Some(place) => el.child(deferred(insertion_line(place == Place::Above, highlight))),
                None => el,
            })
            // Every row hears every move of a drag: the one under the pointer says where on
            // it a drop lands, and the one it leaves lets go of it. What's dragged is the
            // store's copy, which follows it as agents rewrite it; GPUI's stays as it began.
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<Cut>, window, cx| {
                    let (at, bounds) = (event.event.position, event.bounds);
                    let store = this.store.read(cx);
                    let next = if bounds.contains(&at) {
                        let y = (at.y - bounds.top()) / bounds.size.height;
                        (store.dragging.as_ref()).and_then(|payload| {
                            let place = place_at(payload, &move_target, y, window.modifiers());
                            Transfer::resolve(payload, &move_target, place, store)
                                .map(|_| (move_target.clone(), place))
                        })
                    } else if store
                        .drop_at
                        .as_ref()
                        .is_some_and(|(t, _)| *t == move_target)
                    {
                        None
                    } else {
                        return;
                    };
                    if store.drop_at != next {
                        this.store.update(cx, |store, _| store.drop_at = next);
                    }
                }),
            )
            .can_drop(move |_, _, cx| {
                let store = store.read(cx);
                (store.dragging.as_ref()).is_some_and(|payload| {
                    (store.drop_at.as_ref()).is_some_and(|(at, place)| {
                        *at == can_drop_target
                            && Transfer::resolve(payload, at, *place, store).is_some()
                    })
                })
            })
            .on_drop(cx.listener(move |this, _: &Cut, window, cx| {
                let store = this.store.read(cx);
                let (Some(payload), Some((_, place))) =
                    (store.dragging.clone(), store.drop_at.clone())
                else {
                    return;
                };
                let target = target.clone();
                this.app.update_later(window, cx, move |app, window, cx| {
                    app.transfer(&payload, &target, place, window, cx)
                });
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if this.menu_items(&menu_target, cx).is_empty() {
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
                    this.open_menu(event.position, move |this, cx| this.menu_items(&target, cx), cx);
                }),
            )
            .into_any_element()
    }

    /// A pointer moving over a drop zone drops there, so a row still lit lets go, even one a
    /// reload took away, which no row would clear.
    fn over_zone(&mut self, event: &DragMoveEvent<Cut>, _: &mut Window, cx: &mut Context<Self>) {
        if event.bounds.contains(&event.event.position) && self.store.read(cx).drop_at.is_some() {
            self.store.update(cx, |store, _| store.drop_at = None);
            cx.notify();
        }
    }

    /// Where a commit of `project` being dragged lands in a new branch of its own, or a
    /// stacked branch in a stack of its own: a dashed box, lit while a drag is over it.
    /// Padded, not margined: a margin collapses through the card's ground.
    pub(super) fn new_branch_zone(
        &self,
        ix: usize,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let project = project.clone();
        let highlight = theme.pop;
        let label = match self.store.read(cx).dragging {
            Some(Cut::Branch(..)) => "Unstack",
            _ => "New branch",
        };
        let zone = h_flex()
            .id(("new-branch-drop", ix))
            .h(dp(30.))
            .mx(dp(6.))
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
            .drag_over::<Cut>(move |style, _, _, _| {
                style
                    .bg(highlight.opacity(0.14))
                    .border_color(highlight)
                    .text_color(highlight)
            })
            .on_drag_move(cx.listener(Self::over_zone))
            .on_drop(cx.listener(move |this, _: &Cut, _, cx| {
                // A row still lit says where the pill told the drop goes, as when the
                // zone slid under a pointer that didn't move.
                let store = this.store.read(cx);
                if store.drop_at.is_some() {
                    return;
                }
                match store.dragging.clone() {
                    Some(Cut::Commit(from, ids)) if from == project => this
                        .store
                        .update(cx, |store, cx| store.move_to_new_branch(&from, ids, cx)),
                    Some(Cut::Branch(from, branch)) if from == project => this
                        .store
                        .update(cx, |store, cx| store.unstack(&from, branch, cx)),
                    _ => {}
                }
            }));
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

/// Where on `target` a drop of `payload` at `y`, from 0 at its top to 1 at its bottom,
/// lands. A commit dragged onto another goes above it in the top quarter, below it in the
/// bottom one, and into it between; a copy of one, which can't go into another, splits it in
/// halves. Anything else goes into its target, or with ⇧ a branch below another.
fn place_at(payload: &Cut, target: &Selection, y: f32, held: Modifiers) -> Place {
    match (payload, target) {
        (Cut::Commit(..), Selection::Commit(..)) if y < 0.25 => Place::Above,
        (Cut::Commit(..), Selection::Commit(..)) if y > 0.75 => Place::Below,
        (Cut::CherryPick(..), Selection::Commit(..)) if y < 0.5 => Place::Above,
        (Cut::CherryPick(..), Selection::Commit(..)) => Place::Below,
        // A branch's heading: its lower half puts the dragged branch below it, its upper half
        // stacks it on top.
        (Cut::Branch(..), Selection::Branch(..)) if y > 0.5 => Place::Below,
        _ if held.shift && !matches!(target, Selection::Commit(..)) => Place::Below,
        _ => Place::Into,
    }
}

/// The line a drop lands on, along the row's top edge or its bottom one, so the rows either
/// side of a gap draw it in the same place.
pub(super) fn insertion_line(above: bool, color: Hsla) -> Div {
    let line = div()
        .absolute()
        .left(dp(10.))
        .right(dp(6.))
        .h(dp(2.))
        .rounded_full()
        .bg(color)
        .child(
            div()
                .absolute()
                .left(dp(-4.))
                .top(dp(-2.))
                .size(dp(6.))
                .rounded_full()
                .bg(color),
        );
    match above {
        true => line.top(fixed(-1.)),
        false => line.bottom(fixed(-1.)),
    }
}
