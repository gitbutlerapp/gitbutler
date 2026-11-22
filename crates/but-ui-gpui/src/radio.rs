//! A radio button, as but-ui-react's `Radio`: a round Checkbox, with the same ground, edge, pop
//! fill, hover and disabled mix, and a dot for its mark.

use gpui::{Div, ElementId, Hsla, Role, SharedString, Stateful, Toggled, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// One choice of several that exclude each other.
pub fn radio(id: impl Into<ElementId>, checked: bool, theme: &Theme) -> Stateful<Div> {
    let (fill, edge, hover) = match checked {
        true => (
            theme.primary,
            theme.primary,
            theme.derived.primary_checked_hover,
        ),
        false => (theme.background, theme.border, theme.border_hover),
    };
    frame(id, checked, fill, edge, 1., theme).map(|el| match checked {
        true => el.hover(move |el| el.bg(hover).border_color(hover)),
        false => el.hover(move |el| el.border_color(hover)),
    })
}

/// A choice that can't be picked or unpicked, `why` in its tooltip.
pub fn radio_disabled(
    id: impl Into<ElementId>,
    checked: bool,
    why: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    let ink = if checked { theme.primary } else { theme.border };
    let faded = Theme::disabled_on(ink, theme.background);
    frame(id, checked, faded, faded, 0.5, theme).tooltip(Tooltip::text(why.into()))
}

fn frame(
    id: impl Into<ElementId>,
    checked: bool,
    fill: Hsla,
    edge: Hsla,
    dot: f32,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::RadioButton)
        .aria_toggled(if checked {
            Toggled::True
        } else {
            Toggled::False
        })
        .flex_none()
        .size(dp(14.))
        .rounded_full()
        .border_1()
        .border_color(edge)
        .bg(fill)
        .flex()
        .items_center()
        .justify_center()
        .when(checked, |el| {
            el.child(
                div()
                    .size(dp(6.))
                    .rounded_full()
                    .bg(theme.primary_foreground)
                    .opacity(dot),
            )
        })
}
