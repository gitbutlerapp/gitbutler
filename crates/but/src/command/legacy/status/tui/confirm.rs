use std::time::{Duration, Instant};

use but_ctx::Context;
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Padding,
};

use crate::{
    command::legacy::status::tui::{
        Message,
        popup::Popup,
        render::{RenderSingleLineSpans, available_lines_in_area, render_spinner},
    },
    theme::Theme,
    utils::DebugAsType,
};

#[derive(Debug)]
pub struct Confirm {
    lines: NonEmpty<Line<'static>>,
    yes_selected: bool,
    on_yes: DebugAsType<
        Option<Box<dyn FnOnce(&mut Context, &mut Vec<Message>) -> anyhow::Result<()> + Send>>,
    >,
    theme: &'static Theme,
    waiting_on_callback_since: Option<Instant>,
}

impl Confirm {
    pub fn new<F>(lines: NonEmpty<Line<'static>>, theme: &'static Theme, on_yes: F) -> Self
    where
        F: FnOnce(&mut Context, &mut Vec<Message>) -> anyhow::Result<()> + Send + 'static,
    {
        Self {
            lines,
            yes_selected: true,
            on_yes: DebugAsType(Some(Box::new(on_yes))),
            theme,
            waiting_on_callback_since: None,
        }
    }

    pub fn waiting_on_callback(&self) -> bool {
        self.waiting_on_callback_since.is_some()
    }

    pub fn render(&self, has_focus: bool, area: Rect, frame: &mut Frame) {
        let padding = Padding::new(3, 6, 1, 1);

        let buttons = [
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
        ];
        let button_width = buttons.iter().map(Span::width).sum::<usize>() as u16;

        let line_width = self
            .lines
            .iter()
            .map(|line| line.width() as u16)
            .max()
            .unwrap_or(0)
            .max(button_width);
        let popup_width = line_width
            .saturating_add(2)
            .saturating_add(padding.left)
            .saturating_add(padding.right);
        let popup_height = (self.lines.len() as u16)
            .saturating_add(2)
            .saturating_add(2)
            .saturating_add(padding.top)
            .saturating_add(padding.bottom);
        let popup = Popup::new(self.theme, popup_width, popup_height)
            .padding(padding)
            .render(area, frame);

        let mut rows = available_lines_in_area(popup.inner);
        for (line, row) in self.lines.iter().zip(rows.by_ref()) {
            frame.render_widget(line, row);
        }
        if let Some(row) = rows.nth(1) {
            let mut line = RenderSingleLineSpans::new(frame, row);
            if let Some(waiting_on_callback_since) = self.waiting_on_callback_since
                && waiting_on_callback_since.elapsed() > Duration::from_millis(250)
            {
                render_spinner(self.theme, &mut line, waiting_on_callback_since);
            } else {
                line.extend(buttons);
            }
        }
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
            ConfirmMessage::Yes => {
                self.run_callback(ctx, messages)?;
                Ok(Some(self))
            }
            ConfirmMessage::Close => Ok(None),
            ConfirmMessage::Confirm => {
                if self.yes_selected {
                    self.run_callback(ctx, messages)?;
                    Ok(Some(self))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub fn run_callback(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(on_yes) = self.on_yes.0.take() else {
            return Ok(());
        };

        if cfg!(test) {
            on_yes(ctx, messages)?;
            messages.push(Message::Confirm(ConfirmMessage::Close));
        } else {
            let ctx = ctx.to_sync();
            let (tx, rx) = std::sync::mpsc::channel::<Message>();
            messages.push(Message::RegisterOutOfBandMessage(rx));

            std::thread::spawn(move || {
                let mut ctx = ctx.into_thread_local();

                let mut messages = Vec::<Message>::new();
                match on_yes(&mut ctx, &mut messages) {
                    Ok(()) => {
                        for message in messages {
                            if tx.send(message).is_err() {
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        _ = tx.send(Message::ShowError(err));
                    }
                }

                _ = tx.send(Message::Confirm(ConfirmMessage::Close));
            });

            self.waiting_on_callback_since = Some(Instant::now());
        }

        Ok(())
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
    Close,
}
