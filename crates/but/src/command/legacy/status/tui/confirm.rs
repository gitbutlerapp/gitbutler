use std::{
    cell::Cell,
    time::{Duration, Instant},
};

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

/// The widest a wrapped confirmation body gets, even on a wide terminal, so it stays readable.
const MAX_WRAP_WIDTH: u16 = 72;

#[derive(Debug)]
enum Content {
    /// Lines rendered as given.
    Lines(NonEmpty<Line<'static>>),
    /// Text wrapped to the popup's width at render time; `\n` separates paragraphs.
    Wrapped(String),
}

#[derive(Debug)]
pub struct Confirm {
    content: Content,
    /// The first content line shown, when the content is taller than the popup.
    scroll: u16,
    /// The largest useful `scroll` at the last render, so scrolling down stops at the end.
    max_scroll: Cell<u16>,
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
        Self::with_content(Content::Lines(lines), theme, on_yes)
    }

    /// A confirmation whose `text` is wrapped to fit the popup when it renders, and scrolls when
    /// it's taller than the screen, so nothing the user confirms is clipped. `\n` separates
    /// paragraphs.
    pub fn new_wrapped<F>(text: String, theme: &'static Theme, on_yes: F) -> Self
    where
        F: FnOnce(&mut Context, &mut Vec<Message>) -> anyhow::Result<()> + Send + 'static,
    {
        Self::with_content(Content::Wrapped(text), theme, on_yes)
    }

    fn with_content<F>(content: Content, theme: &'static Theme, on_yes: F) -> Self
    where
        F: FnOnce(&mut Context, &mut Vec<Message>) -> anyhow::Result<()> + Send + 'static,
    {
        Self {
            content,
            scroll: 0,
            max_scroll: Cell::new(0),
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

        // Borders and padding around the content, and the gap and button row below it.
        let chrome_width = 2 + padding.left + padding.right;
        let chrome_height = 2 + padding.top + padding.bottom + 2;
        let max_content_width = area.width.saturating_sub(chrome_width).max(1);

        let lines: Vec<Line<'static>> = match &self.content {
            Content::Lines(lines) => lines.iter().cloned().collect(),
            Content::Wrapped(text) => {
                let width = usize::from(max_content_width.min(MAX_WRAP_WIDTH));
                text.split('\n')
                    .flat_map(|paragraph| {
                        if paragraph.is_empty() {
                            vec![Line::default()]
                        } else {
                            textwrap::wrap(paragraph, width)
                                .into_iter()
                                .map(|line| Line::from(line.into_owned()))
                                .collect()
                        }
                    })
                    .collect()
            }
        };

        // Content taller than the screen scrolls, with its last row saying how much is hidden.
        let max_rows = area.height.saturating_sub(chrome_height).max(1);
        let total = lines.len() as u16;
        let (text_rows, overflows) = if total > max_rows {
            (max_rows.saturating_sub(1).max(1), true)
        } else {
            (total, false)
        };
        let max_scroll = total.saturating_sub(text_rows);
        self.max_scroll.set(max_scroll);
        let scroll = self.scroll.min(max_scroll);

        let line_width = lines
            .iter()
            .map(|line| line.width() as u16)
            .max()
            .unwrap_or(0)
            .max(button_width);
        let popup_width = line_width.saturating_add(chrome_width);
        let popup_height = (text_rows + u16::from(overflows)).saturating_add(chrome_height);
        let popup = Popup::new(self.theme, popup_width, popup_height)
            .padding(padding)
            .render(area, frame);

        let mut rows = available_lines_in_area(popup.inner);
        for (line, row) in lines
            .iter()
            .skip(usize::from(scroll))
            .take(usize::from(text_rows))
            .zip(rows.by_ref())
        {
            frame.render_widget(line, row);
        }
        if overflows && let Some(row) = rows.next() {
            let above = scroll;
            let below = max_scroll - scroll;
            let hint = match (above, below) {
                (0, below) => format!("↓ {below} more line(s) — j/k to scroll"),
                (above, 0) => format!("↑ {above} more line(s) — j/k to scroll"),
                (above, below) => format!("↑ {above} / ↓ {below} more line(s) — j/k to scroll"),
            };
            frame.render_widget(Line::from(Span::styled(hint, self.theme.hint)), row);
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
            ConfirmMessage::ScrollUp => Ok(Some(Self {
                scroll: self.scroll.min(self.max_scroll.get()).saturating_sub(1),
                ..self
            })),
            ConfirmMessage::ScrollDown => Ok(Some(Self {
                scroll: (self.scroll + 1).min(self.max_scroll.get()),
                ..self
            })),
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
    ScrollUp,
    ScrollDown,
    Yes,
    Close,
}
