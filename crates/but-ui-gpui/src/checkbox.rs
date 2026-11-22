//! A checkbox, as but-ui-react's `Checkbox`.

use gpui::{Div, ElementId, Hsla, Role, SharedString, Stateful, Toggled, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// A checkbox on the page's ground.
pub fn checkbox(id: impl Into<ElementId>, check: Toggled, theme: &Theme) -> Stateful<Div> {
    checkbox_on(id, check, theme.background, theme)
}

/// A checkbox whose empty box shows `ground`, as React's `--checkbox-bg`, for a row with a
/// background of its own: a solid selected row's is `theme.inverse`.
///
/// Mixed is filled like a checked box: it stands for a folder with some of its files checked.
pub fn checkbox_on(
    id: impl Into<ElementId>,
    check: Toggled,
    ground: Hsla,
    theme: &Theme,
) -> Stateful<Div> {
    let on = check != Toggled::False;
    let (fill, edge, hover) = match on {
        true => (
            theme.primary,
            theme.primary,
            theme.derived.primary_checked_hover,
        ),
        false => (ground, theme.border, theme.border_hover),
    };
    frame(id, check, fill, edge, theme)
        .map(|el| match on {
            true => el.hover(move |el| el.bg(hover).border_color(hover)),
            false => el.hover(move |el| el.border_color(hover)),
        })
        .map(|el| mark(el, check, 1., theme))
}

/// A checkbox that can't be changed, `why` in its tooltip: its colours mixed half and half
/// with the ground, its mark faded.
pub fn checkbox_disabled(
    id: impl Into<ElementId>,
    check: Toggled,
    why: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    let ink = match check {
        Toggled::False => theme.border,
        Toggled::True | Toggled::Mixed => theme.primary,
    };
    let faded = Theme::disabled_on(ink, theme.background);
    frame(id, check, faded, faded, theme)
        .tooltip(Tooltip::text(why.into()))
        .map(|el| mark(el, check, 0.5, theme))
}

fn frame(
    id: impl Into<ElementId>,
    check: Toggled,
    fill: Hsla,
    edge: Hsla,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::CheckBox)
        .aria_toggled(check)
        .flex_none()
        .size(dp(14.))
        .rounded(dp(theme.shape.xs))
        .border_1()
        .border_color(edge)
        .bg(fill)
        .flex()
        .items_center()
        .justify_center()
}

/// The tick, or the dash of a mixed box, at `strength`.
fn mark(el: Stateful<Div>, check: Toggled, strength: f32, theme: &Theme) -> Stateful<Div> {
    match check {
        Toggled::True => el.child(
            icon_at(Icon::Tick, 10.)
                .text_color(theme.primary_foreground)
                .opacity(strength),
        ),
        Toggled::Mixed => el.child(
            div()
                .w(dp(7.))
                .h(dp(1.5))
                .bg(theme.primary_foreground)
                .opacity(strength),
        ),
        Toggled::False => el,
    }
}
