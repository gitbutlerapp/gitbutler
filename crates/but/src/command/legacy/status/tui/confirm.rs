use but_ctx::Context;
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, Padding},
};

use crate::{
    command::legacy::status::tui::{Message, popup::Popup},
    theme::Theme,
    utils::DebugAsType,
};

type OnYes =
    Box<dyn FnOnce(&mut Context, &mut Vec<Message>, Vec<usize>) -> anyhow::Result<()> + Send>;

#[derive(Debug)]
pub struct Confirm {
    lines: NonEmpty<Line<'static>>,
    /// Tickboxes shown between the lines and the buttons; empty for a plain yes/no confirmation.
    choices: Vec<Choice>,
    /// The index of the choice under the cursor.
    choice_cursor: usize,
    yes_selected: bool,
    on_yes: DebugAsType<OnYes>,
    theme: &'static Theme,
}

/// A tickbox row in a [`Confirm`].
#[derive(Debug)]
pub struct Choice {
    pub label: Line<'static>,
    pub ticked: bool,
    /// A disabled choice is shown unticked and can't be toggled; its label should say why.
    pub disabled: bool,
}

impl Confirm {
    pub fn new<F>(lines: NonEmpty<Line<'static>>, theme: &'static Theme, on_yes: F) -> Self
    where
        F: FnOnce(&mut Context, &mut Vec<Message>) -> anyhow::Result<()> + Send + 'static,
    {
        Self::with_choices(lines, Vec::new(), theme, move |ctx, messages, _| {
            on_yes(ctx, messages)
        })
    }

    /// A confirmation that also lets the user tick `choices`. `on_yes` receives the indices of the
    /// ticked choices, and only runs when at least one is ticked or there are no choices at all.
    pub fn with_choices<F>(
        lines: NonEmpty<Line<'static>>,
        choices: Vec<Choice>,
        theme: &'static Theme,
        on_yes: F,
    ) -> Self
    where
        F: FnOnce(&mut Context, &mut Vec<Message>, Vec<usize>) -> anyhow::Result<()>
            + Send
            + 'static,
    {
        let choice_cursor = choices
            .iter()
            .position(|choice| !choice.disabled)
            .unwrap_or_default();
        Self {
            lines,
            choices,
            choice_cursor,
            yes_selected: true,
            on_yes: DebugAsType(Box::new(on_yes)),
            theme,
        }
    }

    pub fn has_choices(&self) -> bool {
        !self.choices.is_empty()
    }

    fn accept(self, ctx: &mut Context, messages: &mut Vec<Message>) -> anyhow::Result<()> {
        let ticked: Vec<usize> = self
            .choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.ticked && !choice.disabled)
            .map(|(idx, _)| idx)
            .collect();
        if self.choices.is_empty() || !ticked.is_empty() {
            (self.on_yes.0)(ctx, messages, ticked)?;
        }
        Ok(())
    }

    fn move_choice_cursor(&mut self, down: bool) {
        let len = self.choices.len();
        let mut idx = self.choice_cursor;
        for _ in 0..len {
            idx = if down {
                (idx + 1) % len
            } else {
                (idx + len - 1) % len
            };
            if !self.choices[idx].disabled {
                self.choice_cursor = idx;
                return;
            }
        }
    }

    pub fn render(&self, has_focus: bool, area: Rect, frame: &mut Frame) {
        let padding = Padding::new(3, 6, 1, 1);

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

        let choice_lines = self
            .choices
            .iter()
            .enumerate()
            .map(|(idx, choice)| {
                let checkbox = if choice.disabled {
                    Span::styled("[-] ", self.theme.hint)
                } else if choice.ticked {
                    Span::styled("[x] ", self.theme.success)
                } else {
                    Span::styled("[ ] ", self.theme.hint)
                };
                let mut line = Line::from_iter([checkbox]);
                line.spans.extend(choice.label.spans.iter().cloned());
                if idx == self.choice_cursor && has_focus && !choice.disabled {
                    line.style(self.theme.selection_highlight)
                } else {
                    line
                }
            })
            .collect::<Vec<_>>();
        let spacer = (!choice_lines.is_empty()).then(|| ListItem::new(""));

        let items = self
            .lines
            .iter()
            .map(|line| ListItem::new(line.clone()))
            .chain(spacer)
            .chain(choice_lines.iter().cloned().map(ListItem::new))
            .chain([ListItem::new(""), ListItem::new(button_line)])
            .collect::<Vec<_>>();

        let line_width = self
            .lines
            .iter()
            .chain(&choice_lines)
            .map(|line| line.width() as u16)
            .max()
            .unwrap_or(0)
            .max(button_width);
        let popup_width = line_width
            .saturating_add(2)
            .saturating_add(padding.left)
            .saturating_add(padding.right);
        let popup_height = (items.len() as u16)
            .saturating_add(2)
            .saturating_add(padding.top)
            .saturating_add(padding.bottom);
        let popup = Popup::new(self.theme, popup_width, popup_height)
            .padding(padding)
            .render(area, frame);

        frame.render_widget(List::new(items), popup.inner);
    }

    pub fn handle_message(
        mut self,
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
            ConfirmMessage::Up | ConfirmMessage::Down => {
                self.move_choice_cursor(matches!(msg, ConfirmMessage::Down));
                Ok(Some(self))
            }
            ConfirmMessage::Toggle => {
                if let Some(choice) = self.choices.get_mut(self.choice_cursor)
                    && !choice.disabled
                {
                    choice.ticked = !choice.ticked;
                }
                Ok(Some(self))
            }
            ConfirmMessage::Yes => {
                self.accept(ctx, messages)?;
                Ok(None)
            }
            ConfirmMessage::No => Ok(None),
            ConfirmMessage::Confirm => {
                if self.yes_selected {
                    self.accept(ctx, messages)?;
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

#[derive(Debug)]
pub enum ConfirmMessage {
    Confirm,
    Left,
    Right,
    Yes,
    No,
    /// Move to the previous tickbox.
    Up,
    /// Move to the next tickbox.
    Down,
    /// Tick or untick the tickbox under the cursor.
    Toggle,
}
