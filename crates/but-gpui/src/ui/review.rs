//! How a pull request and its CI read at a glance, shared by the navigator and the branch
//! view: a glyph and colour per state, and one short line about the checks.

use gpui::Hsla;

use super::Icon;
use crate::backend::{Checks, ReviewState};
use crate::theme::Theme;

pub fn state_glyph(state: ReviewState, theme: &Theme) -> (Icon, Hsla) {
    match state {
        ReviewState::Open => (Icon::Pr, theme.safe),
        ReviewState::Draft => (Icon::PrDraft, theme.text_2),
        ReviewState::Merged => (Icon::BranchMerge, theme.purple),
        ReviewState::Closed => (Icon::PrClose, theme.danger),
    }
}

/// A pull request's title where it names its branch: muted while a draft, fainter once merged
/// or closed.
pub fn title_color(state: ReviewState, theme: &Theme) -> Hsla {
    match state {
        ReviewState::Open => theme.text_1,
        ReviewState::Draft => theme.text_2,
        ReviewState::Merged | ReviewState::Closed => theme.text_3,
    }
}

pub fn state_label(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Open => "Open",
        ReviewState::Draft => "Draft",
        ReviewState::Merged => "Merged",
        ReviewState::Closed => "Closed",
    }
}

/// The checks' glyph and colour, and what they come to: running first, since that's what
/// changes next, then failures.
pub fn checks_summary(checks: Checks, theme: &Theme) -> (Icon, Hsla, String) {
    let total = checks.total();
    if checks.running > 0 {
        let mut text = format!("{} of {total} running", checks.running);
        if checks.failed > 0 {
            text.push_str(&format!(", {} failed", checks.failed));
        }
        (Icon::Clock, theme.warn, text)
    } else if checks.failed > 0 {
        (
            Icon::Cross,
            theme.danger,
            format!("{} of {total} failed", checks.failed),
        )
    } else {
        (Icon::Tick, theme.safe, "All checks passed".to_owned())
    }
}
