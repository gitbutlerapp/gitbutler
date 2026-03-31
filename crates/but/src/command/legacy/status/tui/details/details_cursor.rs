use ratatui::layout::Rect;

use crate::command::legacy::status::tui::details::DetailsAndDiffWidget;

#[derive(Default, Debug, Copy, Clone)]
pub(super) struct DetailsCursor {
    scroll_top: usize,
}

impl DetailsCursor {
    pub(super) fn scroll_top(self) -> usize {
        self.scroll_top
    }

    pub(super) fn scroll_up(self, amount: usize) -> Self {
        Self {
            scroll_top: self.scroll_top.saturating_sub(amount),
        }
    }

    pub(super) fn scroll_down(self, amount: usize) -> Self {
        Self {
            scroll_top: self.scroll_top.saturating_add(amount),
        }
    }

    pub(super) fn clamp(self, viewport: Rect, widget: Option<&DetailsAndDiffWidget>) -> Self {
        // `render()` reserves one column for the left border before passing the remaining
        // area to `DetailsAndDiffWidget::render`. Clamp using the same content width so wrapped
        // commit messages compute the same number of rows in both places.
        let content_width = viewport.width.saturating_sub(1).max(1);

        let max_scroll_top = widget
            .map(|diff| {
                diff.total_rows(content_width)
                    .saturating_sub(viewport.height as usize)
            })
            .unwrap_or(0);

        Self {
            scroll_top: self.scroll_top.min(max_scroll_top),
        }
    }
}
