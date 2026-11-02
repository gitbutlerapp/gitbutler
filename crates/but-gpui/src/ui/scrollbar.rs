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
    /// How far the content was scrolled as last drawn, and when it last moved: what shows a
    /// bar shown only while scrolling.
    offset: Option<Pixels>,
    scrolled_at: Option<std::time::Instant>,
}

/// When a bar shows its thumb.
#[derive(Clone, Copy, PartialEq)]
enum Shows {
    /// While the pointer's over the list.
    OnHover,
    /// Always, beside marks that say where the content is.
    Always,
    /// Only while the content scrolls, and a moment after, as macOS shows its scrollbars, or
    /// while the pointer's on it then or holds it: `middle` in from the list's right, in a
    /// gap there, which the rest of takes the pointer for itself.
    WhileScrolling { middle: Pixels },
}

/// How long a thumb shown only while scrolling stays once the content stops, and how long it
/// takes to fade then.
const STAYS: std::time::Duration = std::time::Duration::from_millis(1000);
const FADES: std::time::Duration = std::time::Duration::from_millis(200);

/// How much of a thumb shown only while scrolling shows at `now`, from 1 down to 0.
fn left_showing(scrolled_at: Option<std::time::Instant>, now: std::time::Instant) -> f32 {
    let Some(at) = scrolled_at else {
        return 0.;
    };
    let since = now.saturating_duration_since(at);
    match since.checked_sub(STAYS) {
        None => 1.,
        Some(fading) => 1. - (fading.as_secs_f32() / FADES.as_secs_f32()).min(1.),
    }
}

/// `list` filling the height left for it, with a thumb shown only while it scrolls, `middle`
/// in from its right, in the gap there: the navigator's, between its cards and the detail's.
pub fn scrolled_in_gap(
    list: impl IntoElement,
    id: &'static str,
    scroll: Scroll,
    middle: Pixels,
    theme: &Theme,
) -> Div {
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .child(list)
        .children(bar(id, scroll, Shows::WhileScrolling { middle }, theme))
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
    bar(id, scroll, Shows::OnHover, theme)
}

/// [`scrollbar`] with its thumb shown with the pointer elsewhere too, as beside marks that
/// say where the content is.
pub fn shown_scrollbar(
    id: &'static str,
    scroll: Scroll,
    theme: &Theme,
) -> Option<impl IntoElement + use<>> {
    bar(id, scroll, Shows::Always, theme)
}

/// The thumb's length on a track `height` long, and how far down it sits at `offset` of
/// `max`. None while the track's too short for the shortest thumb to travel, as an opening
/// drawer's is.
fn thumb_on(height: Pixels, offset: Pixels, max: Pixels) -> Option<(Pixels, Pixels)> {
    const SHORTEST: Pixels = fixed(24.);
    if max <= Pixels::ZERO || height <= SHORTEST {
        return None;
    }
    let thumb = (height * (height / (height + max))).clamp(SHORTEST, height);
    Some((thumb, (height - thumb) * (offset / max).clamp(0., 1.)))
}

fn bar(
    id: &'static str,
    scroll: Scroll,
    shows: Shows,
    theme: &Theme,
) -> Option<impl IntoElement + use<>> {
    let (_, max) = scroll.offsets();
    if max <= Pixels::ZERO {
        return None;
    }
    let (rest, active) = (theme.text_3.opacity(0.5), theme.text_3.opacity(0.8));
    let gap = match shows {
        Shows::WhileScrolling { middle } => Some(middle),
        Shows::OnHover | Shows::Always => None,
    };
    let measured = scroll.clone();
    let element = canvas(
        move |bounds, window, cx| {
            // Changing this state redraws the view the list belongs to.
            let state =
                window.use_keyed_state(ElementId::Name(id.into()), cx, |_, _| BarState::default());
            let area = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            let (offset, max) = measured.offsets();
            let thumb = thumb_on(bounds.size.height, offset, max);
            // Only while scrolling: it shows from when the content last moved, and while held.
            let now = crate::clock::now();
            let showing = gap.map(|_| {
                state.update(cx, |state, _| {
                    if state.offset.is_some_and(|was| was != offset) {
                        state.scrolled_at = Some(now);
                    }
                    state.offset = Some(offset);
                    match state.held {
                        Some(_) => 1.,
                        None => left_showing(state.scrolled_at, now),
                    }
                })
            });
            let track_bounds = match (gap, thumb) {
                (_, None) => None,
                // The gap's width over the thumb alone, the rest of the gap left to what else
                // it's for, and only while it shows.
                (Some(middle), Some((thumb, down))) => Some(Bounds::new(
                    point(bounds.right() - middle - fixed(4.), bounds.top() + down),
                    size(fixed(8.), thumb),
                )),
                (None, Some(_)) => Some(Bounds::new(
                    point(bounds.right() - TRACK, bounds.top()),
                    size(TRACK, bounds.size.height),
                )),
            };
            let track = (track_bounds.filter(|_| showing.is_none_or(|showing| showing > 0.)))
                .map(|track| window.insert_hitbox(track, HitboxBehavior::BlockMouseExceptScroll));
            (state, area, track, showing, now)
        },
        move |bounds, (state, area, track, showing, now), window, cx| {
            let (offset, max) = scroll.offsets();
            let Some((thumb, down)) = thumb_on(bounds.size.height, offset, max) else {
                return;
            };
            let travel = bounds.size.height - thumb;
            let top = bounds.top() + down;
            // The offset that puts the thumb's top at `y`, less where in it the pointer holds it.
            let offset_at = move |y: Pixels, held: Pixels| {
                max * ((y - bounds.top() - held) / travel).clamp(0., 1.)
            };

            let held = state.read(cx).held;
            let on_track = track.as_ref().is_some_and(|track| track.is_hovered(window));
            // On it while it shows, it stays.
            if on_track && showing.is_some() {
                state.update(cx, |state, _| state.scrolled_at = Some(now));
            }
            let shown = match shows {
                Shows::Always => true,
                Shows::OnHover => held.is_some() || on_track || area.is_hovered(window),
                Shows::WhileScrolling { .. } => showing.is_some_and(|showing| showing > 0.),
            };
            state.update(cx, |state, _| state.drawn = (shown, on_track));
            if shown {
                let wide = held.is_some() || on_track;
                let (width, x) = match gap {
                    Some(middle) => {
                        let width = if wide { fixed(6.) } else { fixed(4.) };
                        (width, bounds.right() - middle - width / 2.)
                    }
                    None => {
                        let width = if wide { fixed(7.) } else { fixed(5.) };
                        (width, bounds.right() - fixed(2.) - width)
                    }
                };
                let thumb_bounds = Bounds::new(point(x, top), size(width, thumb));
                let color = if wide { active } else { rest };
                let color = color.opacity(color.a * showing.unwrap_or(1.));
                window.paint_quad(fill(thumb_bounds, color).corner_radii(width / 2.));
            }
            // Shown only while scrolling, it wakes its view to fade, and no more once gone.
            if let Some(showing) = showing.filter(|&showing| showing > 0. && held.is_none()) {
                let since = state
                    .read(cx)
                    .scrolled_at
                    .map(|at| now.saturating_duration_since(at));
                let due = match since.and_then(|since| STAYS.checked_sub(since)) {
                    Some(stays) if showing >= 1. => stays,
                    _ => std::time::Duration::from_millis(16),
                };
                let view = window.current_view();
                (cx.spawn(async move |cx| {
                    cx.background_executor().timer(due).await;
                    cx.update(|cx| cx.notify(view));
                }))
                .detach();
            }
            let Some(track) = track else {
                return;
            };
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
                            let shown = match shows {
                                Shows::OnHover => on_track || area.is_hovered(window),
                                // Showing, as it has a track, it widens under the pointer.
                                Shows::Always | Shows::WhileScrolling { .. } => true,
                            };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_shorter_than_the_shortest_thumb_has_none() {
        assert_eq!(
            thumb_on(fixed(10.), fixed(0.), fixed(100.)),
            None,
            "an opening drawer's track, a few pixels tall, has no room for a thumb"
        );
        assert_eq!(
            thumb_on(fixed(100.), fixed(50.), fixed(100.)),
            Some((fixed(50.), fixed(25.))),
            "half the content shows, so the thumb's half the track, halfway down its travel"
        );
    }
}
