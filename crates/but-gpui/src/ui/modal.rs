//! Modals. The window hosts one at a time over a dimmed backdrop; a modal closes by
//! emitting `DismissEvent`.

use gpui::{Div, Styled as _, px, relative};

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

/// The card a modal is drawn on, at most most of the window.
pub fn modal(size: ModalSize, theme: &Theme) -> Div {
    let width = match size {
        ModalSize::Medium => 640.,
        ModalSize::Large => 820.,
    };
    popover(theme)
        .w(px(width))
        .max_w(relative(0.92))
        .max_h_full()
        .rounded(px(10.))
        .overflow_hidden()
}
