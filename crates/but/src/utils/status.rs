use but_core::{TreeStatus, TreeStatusKind, ui};
use ratatui::text::Span;

pub(crate) fn status_letter(status: &TreeStatus, t: &'static crate::theme::Theme) -> Span<'static> {
    status_letter_kind(status.kind(), t)
}

pub(crate) fn status_letter_ui(
    status: &ui::TreeStatus,
    t: &'static crate::theme::Theme,
) -> Span<'static> {
    match status {
        ui::TreeStatus::Addition { .. } => Span::styled("A", t.addition),
        ui::TreeStatus::Deletion { .. } => Span::styled("D", t.deletion),
        ui::TreeStatus::Modification { .. } => Span::styled("M", t.modification),
        ui::TreeStatus::Rename { .. } => Span::styled("R", t.renaming),
    }
}

pub(crate) fn status_letter_kind(
    kind: TreeStatusKind,
    t: &'static crate::theme::Theme,
) -> Span<'static> {
    match kind {
        TreeStatusKind::Addition => Span::styled("A", t.addition),
        TreeStatusKind::Deletion => Span::styled("D", t.deletion),
        TreeStatusKind::Modification => Span::styled("M", t.modification),
        TreeStatusKind::Rename => Span::styled("R", t.renaming),
    }
}
