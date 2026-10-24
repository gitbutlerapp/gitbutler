//! The window's edge where the desktop leaves it to the app, as GNOME does on Wayland: a shadow
//! around the window, a hairline border, and a band in the shadow that resizes the window from
//! its sides and corners. A side that's against the screen's edge, as a maximized or tiled
//! window's are, has none of them. As Zed draws its own.

use gpui::{
    AnyElement, App, BoxShadow, CursorStyle, Decorations, Div, MouseButton, Pixels, ResizeEdge,
    Stateful, Tiling, Window, div, hsla, prelude::*,
};

use super::fixed;
use crate::theme::Theme;

/// How far the shadow reaches past the window, which is as wide as the band that resizes it.
const SHADOW: Pixels = fixed(10.);

/// `content`, all the window shows, inside the frame where the app draws it; as it is where
/// the desktop does.
pub fn window_frame(content: Stateful<Div>, window: &mut Window, cx: &App) -> AnyElement {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return content.into_any_element();
    };
    window.set_client_inset(SHADOW);
    let content = content
        .when(!tiling.top, |el| el.border_t_1())
        .when(!tiling.bottom, |el| el.border_b_1())
        .when(!tiling.left, |el| el.border_l_1())
        .when(!tiling.right, |el| el.border_r_1())
        .border_color(Theme::get(cx).border_2)
        .when(!tiling.is_tiled(), |el| {
            el.shadow(vec![
                BoxShadow::new(fixed(0.), fixed(0.), hsla(0., 0., 0., 0.4))
                    .blur_radius(SHADOW / 2.),
            ])
        });
    inside_frame(div().size_full(), window)
        .child(content)
        .children(
            [
                ResizeEdge::Top,
                ResizeEdge::TopRight,
                ResizeEdge::Right,
                ResizeEdge::BottomRight,
                ResizeEdge::Bottom,
                ResizeEdge::BottomLeft,
                ResizeEdge::Left,
                ResizeEdge::TopLeft,
            ]
            .into_iter()
            .filter(|edge| window.is_resizable() && !against_screen(*edge, tiling))
            .map(resize_handle),
        )
        .into_any_element()
}

/// `el`, which covers the window, kept off the frame's shadow where the app draws one, as what
/// GPUI draws over the whole window, as an alert, must be.
pub fn inside_frame(el: Div, window: &Window) -> Div {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return el;
    };
    el.when(!tiling.top, |el| el.pt(SHADOW))
        .when(!tiling.bottom, |el| el.pb(SHADOW))
        .when(!tiling.left, |el| el.pl(SHADOW))
        .when(!tiling.right, |el| el.pr(SHADOW))
}

/// Whether `edge` is on a side that's against the screen's edge.
fn against_screen(edge: ResizeEdge, tiling: Tiling) -> bool {
    match edge {
        ResizeEdge::Top => tiling.top,
        ResizeEdge::Right => tiling.right,
        ResizeEdge::Bottom => tiling.bottom,
        ResizeEdge::Left => tiling.left,
        ResizeEdge::TopRight => tiling.top || tiling.right,
        ResizeEdge::BottomRight => tiling.bottom || tiling.right,
        ResizeEdge::BottomLeft => tiling.bottom || tiling.left,
        ResizeEdge::TopLeft => tiling.top || tiling.left,
    }
}

/// The part of the shadow's band that resizes the window from `edge`. A corner's reaches a
/// little into the window, so it's easier to take.
fn resize_handle(edge: ResizeEdge) -> Div {
    let corner = SHADOW * 1.5;
    let handle = div().absolute();
    let handle = match edge {
        ResizeEdge::Top => handle.top_0().left(corner).right(corner).h(SHADOW),
        ResizeEdge::Bottom => handle.bottom_0().left(corner).right(corner).h(SHADOW),
        ResizeEdge::Left => handle.left_0().top(corner).bottom(corner).w(SHADOW),
        ResizeEdge::Right => handle.right_0().top(corner).bottom(corner).w(SHADOW),
        ResizeEdge::TopLeft => handle.top_0().left_0().size(corner),
        ResizeEdge::TopRight => handle.top_0().right_0().size(corner),
        ResizeEdge::BottomLeft => handle.bottom_0().left_0().size(corner),
        ResizeEdge::BottomRight => handle.bottom_0().right_0().size(corner),
    };
    handle
        .cursor(match edge {
            ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
            ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
            ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
            ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
        })
        .on_mouse_down(MouseButton::Left, move |_, window, _| {
            window.start_window_resize(edge)
        })
}
