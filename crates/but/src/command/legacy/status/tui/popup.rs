use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Block, BorderType, Clear, Padding},
};

use crate::theme::Theme;

/// The occupied area of a rendered popup.
#[derive(Debug, Clone, Copy)]
pub(super) struct PopupArea {
    /// The full popup area, including borders and padding.
    pub(super) outer: Rect,
    /// The area inside the popup's borders and padding where content should be rendered.
    pub(super) inner: Rect,
}

/// A centered popup with a themed rounded border.
///
/// The configured width and height are the outer dimensions of the popup,
/// including borders and padding. Callers render their contents into the returned
/// [`PopupArea::inner`] rectangle.
#[derive(Debug, Clone, Copy)]
pub(super) struct Popup {
    width: u16,
    height: u16,
    padding: Padding,
    theme: &'static Theme,
}

impl Popup {
    /// Create a popup with the given outer dimensions.
    pub(super) fn new(theme: &'static Theme, width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            padding: Padding::ZERO,
            theme,
        }
    }

    /// Set the padding between the border and content area.
    pub(super) fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    /// Clear, draw, and center the popup within `area`.
    pub(super) fn render(self, area: Rect, frame: &mut Frame) -> PopupArea {
        let outer = self.outer_area(area);

        frame.render_widget(Clear, outer);

        let block = Block::bordered()
            .padding(self.padding)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.border);
        let inner = block.inner(outer);
        frame.render_widget(block, outer);

        PopupArea { outer, inner }
    }

    /// Return the centered outer popup area within `area`.
    fn outer_area(self, area: Rect) -> Rect {
        let horizontal_layout = Layout::horizontal([Constraint::Length(self.width)])
            .flex(Flex::Center)
            .split(area);

        let centered_layout = Layout::vertical([Constraint::Length(self.height)])
            .flex(Flex::Center)
            .split(horizontal_layout[0]);

        centered_layout[0]
    }
}
