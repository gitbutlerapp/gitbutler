//! The card overlays float on, as but-ui-react's `Popup`.

use gpui::{Div, prelude::*};

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
