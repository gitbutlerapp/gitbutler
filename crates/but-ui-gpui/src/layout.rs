//! Sizes and the flex boxes everything is laid out with.

use gpui::{Div, Pixels, Rems, div, prelude::*};

use crate::theme::Theme;
use gpui::rems;

/// The rem at 100%, so a [`dp`] is a pixel then.
pub const BASE_REM: f32 = 16.;

/// `n` pixels as designed, at 100%, which the interface scale grows or shrinks: sizes from
/// but-ui-react's pixels go in as they are. Only what must stay put, as a hairline or the room
/// for macOS's traffic lights, is in plain pixels.
pub fn dp(n: f32) -> Rems {
    rems(n / BASE_REM)
}

/// `n` pixels at every interface scale, for what must stay put: a hairline, the room for
/// macOS's traffic lights, the scrollbar.
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
        .text_size(dp(12.))
        .text_color(theme.text_2)
        .child(text)
}

/// Faint small text, for details that can be skipped.
pub fn faint(text: impl IntoElement, theme: &Theme) -> Div {
    div()
        .text_size(dp(11.))
        .text_color(theme.text_3)
        .child(text)
}

pub fn section_label(text: impl IntoElement, theme: &Theme) -> Div {
    faint(text, theme).font_weight(gpui::FontWeight::SEMIBOLD)
}
