//! Modals. The window hosts one at a time over a dimmed backdrop; a modal closes by
//! emitting `DismissEvent`.

use gpui::{Div, Styled as _, px, relative};

use super::popover;
use crate::theme::Theme;

/// How wide a modal is, by what it holds.
#[derive(Debug, Clone, Copy)]
pub enum ModalSize {
    /// Settings, the conflict resolver: more than one group.
    Large,
}

/// The card a modal is drawn on, at most most of the window.
pub fn modal(size: ModalSize, theme: &Theme) -> Div {
    let width = match size {
        ModalSize::Large => 820.,
    };
    popover(theme)
        .w(px(width))
        .max_w(relative(0.92))
        .max_h(relative(0.85))
        .rounded(px(10.))
        .overflow_hidden()
}
