//! The card overlays float on, as but-ui-react's `Popup`.

use gpui::{AnyElement, Div, SharedString, div, prelude::*};

use crate::theme::tokens;

use crate::theme::Theme;
use crate::*;

pub fn popover(theme: &Theme) -> Div {
    v_flex()
        .rounded(dp(theme.shape.popup))
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .shadow_lg()
}

/// A popup's group of rows, under its `label` when it has one, as React's `PopupSection`: the
/// label small and quiet, the rows inset by the menu's padding.
pub fn popup_section(
    label: Option<SharedString>,
    items: impl IntoIterator<Item = AnyElement>,
    theme: &Theme,
) -> Div {
    let labelled = label.is_some();
    v_flex()
        .flex_none()
        // On one line, cut with an ellipsis, as a long name would wrap it.
        .children(label.map(|label| {
            div()
                .pt(dp(8.))
                .px(dp(10.))
                .pb(dp(tokens::PADDING_MENU))
                .min_w_0()
                .truncate()
                .text_12()
                .text_color(theme.subtle_foreground)
                .child(label)
        }))
        .child(
            v_flex()
                .p(dp(tokens::PADDING_MENU))
                .when(labelled, |el| el.pt_0())
                .children(items),
        )
}

/// One row of a popup, as React's `PopupItem`: its glyph, its label, which takes the width and
/// gives it back last, and something of its own at its end, as a status. `highlighted` gives it
/// the pointer's tint, as the row it was opened from has.
pub fn popup_item(
    icon: Option<Icon>,
    label: impl Into<SharedString>,
    end: Option<AnyElement>,
    highlighted: bool,
    theme: &Theme,
) -> Div {
    h_flex()
        .flex_none()
        .w_full()
        .h(dp(theme.sizes.control))
        .px(dp(8.))
        .gap(dp(8.))
        .rounded(dp(theme.shape.control))
        .text_13()
        .text_color(theme.foreground)
        .when(highlighted, |el| el.bg(theme.hover))
        .children(icon.map(|icon| {
            icon_at(icon, 14.)
                .flex_none()
                .text_color(theme.muted_foreground)
        }))
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
        .children(end.map(|end| h_flex().flex_none().child(end)))
}
