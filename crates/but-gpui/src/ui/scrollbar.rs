//! A scrollbar laid over the right edge of a list, drawn and driven the way Zed's is: a thin
//! thumb that shows while the pointer is over the list, widens under the pointer, and can be
//! dragged. A press on the track brings the thumb there.
//!
//! It handles the mouse itself rather than through hover styles: its hitbox over the track
//! keeps the rows beneath from reacting, and a drag keeps going wherever the pointer goes.

use gpui::{
    Bounds, CursorStyle, DispatchPhase, Div, ElementId, HitboxBehavior, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollHandle, canvas, div, fill, point,
    prelude::*, size,
};

use super::{FocusedAt, ShowFocused, fixed};
use crate::theme::Theme;

/// The width of the strip that takes the pointer.
const TRACK: Pixels = fixed(14.);

/// The scroll state a scrollbar reads and moves.
#[derive(Clone)]
pub enum Scroll {
    List(ListState),
    /// A scrolling div's or a `uniform_list`'s, through the handle it tracks.
    Div(ScrollHandle),
}

impl Scroll {
    /// How far the content is scrolled, and how far it can go.
    fn offsets(&self) -> (Pixels, Pixels) {
        match self {
            Scroll::List(list) => (
                -list.scroll_px_offset_for_scrollbar().y,
                list.max_offset_for_scrollbar().y,
            ),
            Scroll::Div(handle) => (-handle.offset().y, handle.max_offset().y),
        }
    }

    /// Scroll the least that shows `bounds`, part of the content as last drawn, or its top
    /// when it's taller than the view. A list keeps its own place.
    fn reveal(&self, bounds: Bounds<Pixels>) {
        let Scroll::Div(handle) = self else { return };
        let view = handle.bounds();
        let by = if bounds.top() < view.top() {
            bounds.top() - view.top()
        } else if bounds.bottom() > view.bottom() {
            (bounds.bottom() - view.bottom()).min(bounds.top() - view.top())
        } else {
            return;
        };
        let (offset, max) = self.offsets();
        self.scroll_to((offset + by).max(Pixels::ZERO).min(max));
    }

    fn scroll_to(&self, offset: Pixels) {
        let offset = point(Pixels::ZERO, -offset);
        match self {
            Scroll::List(list) => list.set_offset_from_scrollbar(offset),
            Scroll::Div(handle) => handle.set_offset(offset),
        }
    }

    /// A list's height counts only the rows drawn so far; keep it still while the thumb is
    /// held, so drawing more doesn't move the thumb away from the pointer.
    fn hold(&self, held: bool) {
        if let Scroll::List(list) = self {
            match held {
                true => list.scrollbar_drag_started(),
                false => list.scrollbar_drag_ended(),
            }
        }
    }
}

#[derive(Default)]
struct BarState {
    /// How far down the thumb the pointer took hold of it, while it's dragged.
    held: Option<Pixels>,
    /// Whether the bar was last drawn showing, and with the pointer on its track.
    drawn: (bool, bool),
}

/// `list` filling the height left for it, with its scrollbar over its right edge.
pub fn scrolled(list: impl IntoElement, id: &'static str, scroll: Scroll, theme: &Theme) -> Div {
    let shown = scroll.clone();
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .on_action(move |_: &ShowFocused, window, cx| {
            if cx.has_global::<FocusedAt>() {
                shown.reveal(cx.remove_global::<FocusedAt>().0);
                window.refresh();
            }
        })
        .child(list)
        .children(scrollbar(id, scroll, theme))
}

/// The scrollbar for `scroll`, to place over a positioned element exactly the list's size.
/// Nothing when the list fits.
pub fn scrollbar(
    id: &'static str,
    scroll: Scroll,
    theme: &Theme,
) -> Option<impl IntoElement + use<>> {
    let (_, max) = scroll.offsets();
    if max <= Pixels::ZERO {
        return None;
    }
    let (rest, active) = (theme.text_3.opacity(0.5), theme.text_3.opacity(0.8));
    let element = canvas(
        move |bounds, window, cx| {
            // Changing this state redraws the view the list belongs to.
            let state =
                window.use_keyed_state(ElementId::Name(id.into()), cx, |_, _| BarState::default());
            let area = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            let track_bounds = Bounds::new(
                point(bounds.right() - TRACK, bounds.top()),
                size(TRACK, bounds.size.height),
            );
            let track = window.insert_hitbox(track_bounds, HitboxBehavior::BlockMouseExceptScroll);
            (state, area, track)
        },
        move |bounds, (state, area, track), window, cx| {
            let (offset, max) = scroll.offsets();
            let height = bounds.size.height;
            // Too short for the shortest thumb, it has no room to travel.
            if max <= Pixels::ZERO || height <= fixed(24.) {
                return;
            }
            let thumb = (height * (height / (height + max))).clamp(fixed(24.), height);
            let travel = height - thumb;
            let top = bounds.top() + travel * (offset / max).clamp(0., 1.);
            // The offset that puts the thumb's top at `y`, less where in it the pointer holds it.
            let offset_at = move |y: Pixels, held: Pixels| {
                max * ((y - bounds.top() - held) / travel).clamp(0., 1.)
            };

            let held = state.read(cx).held;
            let on_track = track.is_hovered(window);
            let shown = held.is_some() || on_track || area.is_hovered(window);
            state.update(cx, |state, _| state.drawn = (shown, on_track));
            if shown {
                let wide = held.is_some() || on_track;
                let width = if wide { fixed(7.) } else { fixed(5.) };
                let thumb_bounds = Bounds::new(
                    point(bounds.right() - fixed(2.) - width, top),
                    size(width, thumb),
                );
                let color = if wide { active } else { rest };
                window.paint_quad(fill(thumb_bounds, color).corner_radii(width / 2.));
            }
            window.set_cursor_style(CursorStyle::Arrow, &track);

            window.on_mouse_event({
                let (state, scroll, track) = (state.clone(), scroll.clone(), track.clone());
                move |event: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble
                        || event.button != MouseButton::Left
                        || !track.is_hovered(window)
                    {
                        return;
                    }
                    let y = event.position.y;
                    let held = if y >= top && y <= top + thumb {
                        y - top
                    } else {
                        scroll.scroll_to(offset_at(y, thumb / 2.));
                        thumb / 2.
                    };
                    scroll.hold(true);
                    state.update(cx, |state, cx| {
                        state.held = Some(held);
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            });
            window.on_mouse_event({
                let (state, scroll) = (state.clone(), scroll.clone());
                move |event: &MouseMoveEvent, phase, window, cx| {
                    match state.read(cx).held {
                        // First in line while dragging, so nothing under the pointer reacts.
                        Some(held) if phase == DispatchPhase::Capture && event.dragging() => {
                            scroll.scroll_to(offset_at(event.position.y, held));
                            state.update(cx, |_, cx| cx.notify());
                            cx.stop_propagation();
                        }
                        None if phase == DispatchPhase::Bubble => {
                            let on_track = track.is_hovered(window);
                            let shown = on_track || area.is_hovered(window);
                            if state.read(cx).drawn != (shown, on_track) {
                                state.update(cx, |_, cx| cx.notify());
                            }
                        }
                        _ => {}
                    }
                }
            });
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture && state.read(cx).held.is_some() {
                    scroll.hold(false);
                    state.update(cx, |state, cx| {
                        state.held = None;
                        cx.notify();
                    });
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    Some(element)
}
