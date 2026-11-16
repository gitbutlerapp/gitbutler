//! A snackbar, as but-ui-react's `Snackbar`.

use gpui::{AnyElement, Div, ElementId, Role, SharedString, Stateful, Svg, div, prelude::*};

use crate::theme::{Theme, tokens};
use crate::*;

/// Figma's close button is left out: every snackbar here leaves on its own.
pub fn snackbar(
    id: impl Into<ElementId>,
    message: SharedString,
    warning: bool,
    action: Option<AnyElement>,
    theme: &Theme,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::Alert)
        .aria_label(message.clone())
        // but-ui's paddings, each a pixel less for the border.
        .h(dp(tokens::SIZE_SNACKBAR))
        .pl(dp(match warning {
            true => tokens::PADDING_SNACKBAR_START,
            false => tokens::PADDING_SNACKBAR_END,
        } - 1.))
        .pr(dp(match action.is_some() {
            true => tokens::PADDING_SNACKBAR_BLOCK,
            false => tokens::PADDING_SNACKBAR_END,
        } - 1.))
        .gap(dp(tokens::GAP_SNACKBAR))
        .rounded(dp(theme.shape.popup))
        .border_1()
        .border_color(theme.snackbar_border)
        .bg(theme.snackbar_bg)
        .text_color(theme.snackbar_text)
        .text_12()
        .shadow_lg()
        .when(warning, |el| {
            el.child(snackbar_icon(Icon::Warning).text_color(theme.snackbar_warn))
        })
        .child(message)
        .when_some(action, |el, action| {
            el.child(
                div()
                    .flex_none()
                    .pl(dp(4.))
                    .child(div().w(fixed(1.)).h(dp(20.)).bg(theme.snackbar_divider)),
            )
            .child(action)
        })
}

pub fn snackbar_undo(id: impl Into<ElementId>, theme: &Theme) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(dp(20.))
        .px(dp(6.))
        .gap(dp(4.))
        .rounded(dp(theme.shape.control))
        .semibold()
        .hover(|el| el.bg(theme.snackbar_hover))
        .child(snackbar_icon(Icon::Undo).text_color(theme.snackbar_text))
        .child("Undo")
}

fn snackbar_icon(icon: Icon) -> Svg {
    icon_at(icon, 14.)
}
