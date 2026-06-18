use std::time::Duration;

use anyhow::Context as _;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use nonempty::NonEmpty;
use ratatui::{
    Frame, Terminal,
    layout::Rect,
    prelude::Backend,
    style::Stylize,
    text::{Line, Span},
    widgets::{Clear, List},
};

use crate::{tui::TerminalGuard as _, utils::InputOutputChannel};

pub struct PickerOptions {
    pub allow_multiple: bool,
}

pub fn run_picker<'a, Key, Value>(
    _out: &mut InputOutputChannel<'_>,
    prompt: &str,
    items: &'a NonEmpty<(Key, Value)>,
    options: PickerOptions,
) -> anyhow::Result<Option<Vec<&'a Value>>>
where
    Key: std::fmt::Display,
{
    let PickerOptions { allow_multiple } = options;

    let picks = {
        let mut guard = super::CrosstermTerminalGuard::inline((1 + items.len()) as _)
            .context("failed to setup picker tui")?;

        let items = items.as_ref().map(|(key, value)| PickerItem {
            key,
            value,
            selected: false,
        });

        let app = App {
            should_render: true,
            should_quit: false,
            should_confirm: false,
            allow_multiple,
            prompt: prompt.to_owned(),
            cursor: 0,
            items,
        };

        app.run(guard.terminal_mut())?
    };

    Ok(picks)
}

struct App<'a, Key, Value> {
    should_render: bool,
    should_quit: bool,
    should_confirm: bool,
    allow_multiple: bool,
    prompt: String,
    cursor: usize,
    items: NonEmpty<PickerItem<'a, Key, Value>>,
}

struct PickerItem<'a, Key, Value> {
    key: &'a Key,
    value: &'a Value,
    selected: bool,
}

impl<'a, Key, Value> App<'a, Key, Value>
where
    Key: std::fmt::Display,
{
    fn run<B>(mut self, terminal: &mut Terminal<B>) -> anyhow::Result<Option<Vec<&'a Value>>>
    where
        B: Backend<Error: Send + Sync + 'static>,
    {
        loop {
            if self.should_quit {
                render_final_frame(terminal, |frame, area| {
                    frame.render_widget(
                        Line::from_iter([
                            Span::raw(self.prompt),
                            Span::raw(" - "),
                            Span::raw("Aborted").red(),
                        ]),
                        area,
                    );
                })?;
                break Ok(None);
            }

            if self.should_confirm {
                if self.allow_multiple {
                    let picks = self
                        .items
                        .iter()
                        .filter(|item| item.selected)
                        .map(|item| (item.key, item.value))
                        .collect::<Vec<_>>();
                    render_final_frame(terminal, |frame, area| {
                        frame.render_widget(
                            Line::from_iter([
                                Span::raw(self.prompt),
                                Span::raw(" - "),
                                if picks.is_empty() {
                                    Span::raw("None").red()
                                } else {
                                    Span::raw(
                                        picks
                                            .iter()
                                            .map(|(key, _)| key.to_string())
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                    )
                                },
                            ]),
                            area,
                        );
                    })?;
                    break Ok(Some(picks.into_iter().map(|(_, value)| value).collect()));
                } else {
                    let pick = &self.items[self.cursor];
                    render_final_frame(terminal, |frame, area| {
                        frame.render_widget(
                            Line::from_iter([
                                Span::raw(self.prompt),
                                Span::raw(" - "),
                                Span::raw(pick.key.to_string()),
                            ]),
                            area,
                        );
                    })?;
                    break Ok(Some(Vec::from([pick.value])));
                }
            }

            if std::mem::take(&mut self.should_render) {
                terminal.draw(|frame| self.render(frame))?;
            }

            if event::poll(Duration::from_millis(30))
                .context("failed to poll for terminal events")?
            {
                match event::read()? {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        match key_event.code {
                            KeyCode::Char(c) => match c {
                                'q' => {
                                    self.quit();
                                }
                                'c' | 'd' if key_event.modifiers == KeyModifiers::CONTROL => {
                                    self.quit()
                                }
                                'k' => self.move_up(),
                                'j' => self.move_down(),
                                ' ' => self.toggle_selection(),
                                _ => {}
                            },
                            KeyCode::Enter => self.confirm(),
                            KeyCode::Up => self.move_up(),
                            KeyCode::Down => self.move_down(),
                            KeyCode::Esc => self.quit(),
                            _ => {}
                        }
                    }
                    Event::Key(..) => {}
                    Event::Paste(_) | Event::Resize(_, _) | Event::FocusGained => {
                        self.should_render = true;
                    }
                    Event::FocusLost | Event::Mouse(_) => {}
                }
                self.should_render = true;
            }
        }
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn confirm(&mut self) {
        self.should_confirm = true;
    }

    fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_down(&mut self) {
        self.cursor = std::cmp::min(self.cursor + 1, self.items.len() - 1);
    }

    fn toggle_selection(&mut self) {
        if !self.allow_multiple {
            return;
        }

        let selection = &mut self.items[self.cursor];
        selection.selected = !selection.selected;

        self.move_down();
    }

    fn render(&self, frame: &mut Frame<'_>) {
        let area = frame.area();

        let mut items = Vec::new();
        items.push(self.prompt.clone());

        for (idx, item) in self.items.iter().enumerate() {
            let PickerItem {
                key,
                selected,
                value: _,
            } = item;

            if self.allow_multiple {
                items.push(
                    Line::from_iter([
                        if self.cursor == idx {
                            Span::raw("> ")
                        } else {
                            Span::raw("  ")
                        },
                        if *selected {
                            Span::raw("[x] ")
                        } else {
                            Span::raw("[ ] ")
                        },
                        Span::raw(key.to_string()),
                    ])
                    .into(),
                );
            } else {
                items.push(
                    Line::from_iter([
                        if self.cursor == idx {
                            Span::raw("> ")
                        } else {
                            Span::raw("  ")
                        },
                        Span::raw(key.to_string()),
                    ])
                    .into(),
                );
            }
        }

        frame.render_widget(List::new(items), area);
    }
}

fn render_final_frame<B, F>(terminal: &mut Terminal<B>, f: F) -> anyhow::Result<()>
where
    B: Backend<Error: Send + Sync + 'static>,
    F: FnOnce(&mut Frame<'_>, Rect),
{
    terminal.draw(|frame| {
        let area = frame.area();

        frame.render_widget(Clear, area);
        f(frame, area);

        // so subsequent prints show up in the right place
        frame.set_cursor_position((0, area.y + 1));
    })?;
    Ok(())
}
