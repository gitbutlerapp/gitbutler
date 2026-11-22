//! Sizes and the flex boxes everything is laid out with.

use gpui::{Div, Pixels, Rems, div, prelude::*};

use crate::Text;
use crate::theme::Theme;
use gpui::rems;

pub const BASE_REM: f32 = 16.;

/// `n` pixels as designed, scaled with the interface: but-ui-react's pixel sizes go in as they are.
pub fn dp(n: f32) -> Rems {
    rems(n / BASE_REM)
}

/// `n` pixels at every interface scale, only for what must stay put, such as a hairline.
#[expect(clippy::disallowed_methods)]
pub const fn fixed(n: f32) -> Pixels {
    gpui::px(n)
}

pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

pub fn v_flex() -> Div {
    div().flex().flex_col()
}

pub fn muted(text: impl IntoElement, theme: &Theme) -> Div {
    div()
        .text_12()
        .text_color(theme.muted_foreground)
        .child(text)
}

pub fn faint(text: impl IntoElement, theme: &Theme) -> Div {
    div()
        .text_11()
        .text_color(theme.subtle_foreground)
        .child(text)
}

pub fn section_label(text: impl IntoElement, theme: &Theme) -> Div {
    faint(text, theme).semibold()
}
