//! A count in a pill, as but-ui-react's `Badge`.

use gpui::{Div, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// As but-ui-react's light grey `Badge`.
pub fn count_badge(count: usize, theme: &Theme) -> Div {
    div()
        .flex_none()
        .h(dp(16.))
        .min_w(dp(16.))
        .px(dp(5.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme.muted)
        .text_11()
        .semibold()
        .text_color(theme.text_1)
        .child(count.to_string())
}
