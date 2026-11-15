//! Toggles joined in a row, as but-ui-react's `ToggleGroup`.

use gpui::{Div, ElementId, Role, SharedString, Stateful, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// As but-ui-react's `ToggleGroupStyles` of outline toggles.
pub fn toggle_group(theme: &Theme, toggles: impl IntoIterator<Item = Stateful<Div>>) -> Div {
    let line = theme.control_border;
    let toggles: Vec<_> = toggles.into_iter().collect();
    let last = toggles.len().saturating_sub(1);
    h_flex()
        .flex_none()
        .h(dp(theme.sizes.control))
        .rounded(dp(theme.shape.control))
        .border_1()
        .border_color(line)
        .children(toggles.into_iter().enumerate().map(|(ix, toggle)| {
            // Rounded inside the border, so a pressed one's ground keeps to it.
            toggle
                .when(ix == 0, |el| el.rounded_l(dp(theme.shape.xs)))
                .when(ix == last, |el| el.rounded_r(dp(theme.shape.xs)))
                .when(ix > 0, |el| el.border_l_1().border_color(line))
        }))
}

/// One of a [`toggle_group`]'s toggles.
pub fn icon_toggle(
    id: impl Into<ElementId>,
    glyph: Icon,
    tip: &'static str,
    key: Option<SharedString>,
    pressed: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = theme.hover;
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(tip)
        .w(dp(28.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .when(pressed, |el| el.bg(theme.bg_2))
        .when(!pressed, |el| el.hover(move |el| el.bg(hover)))
        .child(icon(glyph).text_color(theme.text_1).opacity(0.7))
        .tooltip(Tooltip::keyed(tip, key))
}
