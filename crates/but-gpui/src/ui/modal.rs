//! Modals. The window hosts one at a time over a dimmed backdrop; a modal closes by
//! emitting `DismissEvent`.

use gpui::{Div, Styled as _};

use super::dp;
use super::popover;
use crate::theme::Theme;

/// How wide a modal is, by what it holds.
#[derive(Debug, Clone, Copy)]
pub enum ModalSize {
    /// A form around one task, like a pull request's title and description.
    Medium,
    /// Settings, the conflict resolver: more than one group.
    Large,
}

/// How round a modal's corners are.
pub const MODAL_RADIUS: f32 = 10.;

/// The card a modal is drawn on, as wide as its size says but never wider than the window
/// leaves it (see `render_modal`). Clipping its content keeps it in the card's square, not
/// its rounded corners: a part with a ground of its own along an edge rounds those corners
/// itself, by [`MODAL_RADIUS`] less the border.
pub fn modal(size: ModalSize, theme: &Theme) -> Div {
    let width = match size {
        ModalSize::Medium => 520.,
        ModalSize::Large => 820.,
    };
    popover(theme)
        .w(dp(width))
        .max_w_full()
        .max_h_full()
        .rounded(dp(MODAL_RADIUS))
        .overflow_hidden()
}
