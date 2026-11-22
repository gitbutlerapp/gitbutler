//! A small pill carrying a count, a status or a short label, as but-ui-react's `Badge`: an icon
//! leads the label or stands alone, and a badge is never a control.

use gpui::{Div, Hsla, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// As React's `BadgeVariant`. The label carries the colour over a soft tint of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeVariant {
    /// The default: a count or a neutral label, with no verdict.
    FillGray,
    /// `FillGray`'s quieter twin, for the same counts where the solid one is too loud.
    LightGray,
    /// Something that came off or is fine.
    Safe,
    /// Something waiting on the user or at risk.
    Warn,
    /// Something that failed.
    Danger,
    /// Work that has landed, and what an agent wrote.
    Purple,
    /// Something calm and informational.
    Blue,
    /// The accent: at most one per surface.
    Pop,
}

impl BadgeVariant {
    /// The pill's fill and the colour its label and icon take, from the tokens React's CSS reads.
    pub fn colors(self, theme: &Theme) -> (Hsla, Hsla) {
        match self {
            BadgeVariant::FillGray => (theme.inverse, theme.inverse_foreground),
            BadgeVariant::LightGray => (theme.neutral_soft, theme.foreground),
            BadgeVariant::Safe => (theme.success_soft, theme.success_soft_foreground),
            BadgeVariant::Warn => (theme.warning_soft, theme.warning_soft_foreground),
            BadgeVariant::Danger => (theme.danger_soft, theme.danger_soft_foreground),
            BadgeVariant::Purple => (theme.done_soft, theme.done_soft_foreground),
            BadgeVariant::Blue => (theme.info_soft, theme.info_soft_foreground),
            BadgeVariant::Pop => (theme.primary_soft, theme.primary_soft_foreground),
        }
    }
}

/// As React's `BadgeSize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeSize {
    #[default]
    Regular,
    Large,
}

/// A badge; its label and icon are its children. An icon takes `variant.colors(theme).1`, as
/// `svg` doesn't inherit the text colour.
pub fn badge(variant: BadgeVariant, size: BadgeSize, theme: &Theme) -> Div {
    let (fill, text) = variant.colors(theme);
    let pill = div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .min_w(dp(16.))
        .rounded_full()
        .bg(fill)
        .text_color(text);
    match size {
        BadgeSize::Regular => pill.h(dp(16.)).px(dp(5.)).gap(dp(2.)).text_11(),
        // `--but-size-tag`, which is the small control's height and follows the density.
        BadgeSize::Large => pill
            .h(dp(theme.sizes.control_sm))
            .px(dp(7.))
            .gap(dp(4.))
            .text_12(),
    }
    .semibold()
}

/// A count, as React writes `<Badge variant="lightGray">`.
pub fn count_badge(count: usize, theme: &Theme) -> Div {
    badge(BadgeVariant::LightGray, BadgeSize::Regular, theme).child(count.to_string())
}
