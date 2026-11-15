//! Modals, as but-ui-react's `Modal`. A modal closes by emitting `DismissEvent`.

use gpui::{Div, Styled as _};

use crate::dp;
use crate::popup::popover;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy)]
pub enum ModalSize {
    Medium,
    Large,
}

/// Content is clipped to the card's square, not its rounded corners: a part with a ground of its
/// own along an edge rounds those corners itself, by the popup radius less the border.
pub fn modal(size: ModalSize, theme: &Theme) -> Div {
    let width = match size {
        ModalSize::Medium => 520.,
        ModalSize::Large => 820.,
    };
    popover(theme)
        .w(dp(width))
        .max_w_full()
        .max_h_full()
        .rounded(dp(theme.shape.popup))
        .overflow_hidden()
}
