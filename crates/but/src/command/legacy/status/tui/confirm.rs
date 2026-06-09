use but_ctx::Context;
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Padding},
};

use crate::{command::legacy::status::tui::Message, theme::Theme, utils::DebugAsType};

#[derive(Debug)]
pub(super) struct Confirm {
    lines: NonEmpty<Line<'static>>,
    yes_selected: bool,
    on_yes: DebugAsType<Box<dyn FnOnce(&mut Context, &mut Vec<Message>) -> anyhow::Result<()>>>,
    theme: &'static Theme,
}

impl Confirm {
    pub(super) fn new<F>(lines: NonEmpty<Line<'static>>, theme: &'static Theme, on_yes: F) -> Self
    where
        F: FnOnce(&mut Context, &mut Vec<Message>) -> anyhow::Result<()> + 'static,
    {
        Self {
            lines,
            yes_selected: true,
            on_yes: DebugAsType(Box::new(on_yes)),
            theme,
        }
    }

    pub(super) fn render(&self, has_focus: bool, area: Rect, frame: &mut Frame) {
        let padding = Padding::new(3, 6, 1, 1);
        let horizontal_padding = padding.left + padding.right;
        let vertical_padding = padding.top + padding.bottom;

        let space_taken_up_by_border: u16 = 2;

        let button_line = Line::from_iter([
            style_button(
                Span::raw("  Yes  "),
                self.yes_selected,
                has_focus,
                self.theme,
            ),
            style_button(
                Span::raw("  No  "),
                !self.yes_selected,
                has_focus,
                self.theme,
            ),
        ]);
        let button_width = button_line.width() as u16;

        let items = self
            .lines
            .iter()
            .map(|line| ListItem::new(line.clone()))
            .chain([ListItem::new(""), ListItem::new(button_line)])
            .collect::<Vec<_>>();

        let line_width = self
            .lines
            .iter()
            .map(|line| line.width() as u16)
            .max()
            .unwrap_or(0)
            .max(button_width);
        let horizontal_layout = Layout::horizontal([Constraint::Length(
            line_width + space_taken_up_by_border + horizontal_padding,
        )])
        .flex(Flex::Center)
        .split(area);

        let centered_layout = Layout::vertical([Constraint::Length(
            (items.len() as u16) + space_taken_up_by_border + vertical_padding,
        )])
        .flex(Flex::Center)
        .split(horizontal_layout[0]);

        let content = List::new(items).block(
            Block::bordered()
                .padding(padding)
                .border_type(BorderType::Rounded)
                .border_style(self.theme.border),
        );

        frame.render_widget(Clear, centered_layout[0]);

        frame.render_widget(content, centered_layout[0]);
    }

    pub(super) fn handle_message(
        self,
        msg: ConfirmMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<Option<Self>> {
        match msg {
            ConfirmMessage::Left => Ok(Some(Self {
                yes_selected: true,
                ..self
            })),
            ConfirmMessage::Right => Ok(Some(Self {
                yes_selected: false,
                ..self
            })),
            ConfirmMessage::Yes => {
                (self.on_yes.0)(ctx, messages)?;
                Ok(None)
            }
            ConfirmMessage::No => Ok(None),
            ConfirmMessage::Confirm => {
                if self.yes_selected {
                    (self.on_yes.0)(ctx, messages)?;
                }
                Ok(None)
            }
        }
    }
}

fn style_button(
    span: Span<'static>,
    selected: bool,
    has_focus: bool,
    theme: &'static Theme,
) -> Span<'static> {
    if selected && has_focus {
        span.style(theme.selection_highlight)
    } else {
        span.style(theme.hint)
    }
}

#[derive(Debug, Clone)]
pub(super) enum ConfirmMessage {
    Confirm,
    Left,
    Right,
    Yes,
    No,
}
