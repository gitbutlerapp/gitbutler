use std::borrow::Cow;

use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Padding},
};
use unicode_width::UnicodeWidthStr;

use crate::command::legacy::status::tui::Message;

#[derive(Debug)]
pub(super) struct Confirm {
    text: Cow<'static, str>,
    yes_selected: bool,
    message_if_yes: Message,
}

impl Confirm {
    #[expect(dead_code)]
    pub(super) fn new(text: impl Into<Cow<'static, str>>, message_if_yes: Message) -> Self {
        Self {
            text: text.into(),
            yes_selected: true,
            message_if_yes,
        }
    }

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let padding = Padding::new(3, 6, 1, 1);
        let horizontal_padding = padding.left + padding.right;
        let vertical_padding = padding.top + padding.bottom;

        let space_taken_up_by_border: u16 = 2;

        let items = Vec::from([
            ListItem::new(&*self.text),
            ListItem::new(""),
            ListItem::new(Line::from_iter([
                style_button(Span::raw("  Yes  "), self.yes_selected),
                style_button(Span::raw("  No  "), !self.yes_selected),
            ])),
        ]);

        let horizontal_layout = Layout::horizontal([Constraint::Length(
            (self.text.width() as u16) + space_taken_up_by_border + horizontal_padding,
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
                .border_style(Style::default().dark_gray()),
        );

        frame.render_widget(Clear, centered_layout[0]);

        frame.render_widget(content, centered_layout[0]);
    }

    pub(super) fn handle_message(
        self,
        msg: ConfirmMessage,
        messages: &mut Vec<Message>,
    ) -> Option<Self> {
        match msg {
            ConfirmMessage::Left => Some(Self {
                yes_selected: true,
                ..self
            }),
            ConfirmMessage::Right => Some(Self {
                yes_selected: false,
                ..self
            }),
            ConfirmMessage::Yes => {
                messages.push(self.message_if_yes);
                None
            }
            ConfirmMessage::No => None,
            ConfirmMessage::Confirm => {
                if self.yes_selected {
                    messages.push(self.message_if_yes);
                }
                None
            }
        }
    }
}

fn style_button(span: Span<'static>, selected: bool) -> Span<'static> {
    if selected {
        span.white().on_dark_gray()
    } else {
        span.dim()
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
