//! What happened to a file, as one letter, as but-ui-react's `FileStatusBadge`: A added,
//! M modified, D deleted, R renamed, in the change status colours.

use gpui::{Div, Hsla, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// What happened to a file, as but-ui-react's `FileStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Addition,
    Deletion,
    Modification,
    Rename,
}

impl FileStatus {
    /// The letter the row's last cell shows.
    pub fn letter(self) -> &'static str {
        match self {
            FileStatus::Addition => "A",
            FileStatus::Deletion => "D",
            FileStatus::Modification => "M",
            FileStatus::Rename => "R",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            FileStatus::Addition => "Addition",
            FileStatus::Deletion => "Deletion",
            FileStatus::Modification => "Modification",
            FileStatus::Rename => "Rename",
        }
    }

    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            FileStatus::Addition => theme.change_status_addition,
            FileStatus::Deletion => theme.change_status_deletion,
            FileStatus::Modification => theme.change_status_modification,
            FileStatus::Rename => theme.change_status_rename,
        }
    }
}

/// The status's letter, in a cell as wide as React's, so a list's letters line up.
pub fn file_status_badge(status: FileStatus, theme: &Theme) -> Div {
    div()
        .flex_none()
        .w(dp(12.))
        .flex()
        .justify_center()
        .text_11()
        .semibold()
        .text_color(status.color(theme))
        .child(status.letter())
}
