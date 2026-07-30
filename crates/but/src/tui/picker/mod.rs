use std::{collections::HashSet, ops::ControlFlow, time::Duration};

use anyhow::Context as _;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    layout::Rect,
    prelude::Backend,
    style::Stylize,
    text::{Line, Span},
    widgets::{Clear, List},
};

use crate::{
    theme,
    tui::{EmptyContext, Tui, event_polling::CrosstermEventPolling},
    utils::InputOutputChannel,
};

use super::{
    CrosstermTerminalGuard, TerminalGuard, TuiInputOutputChannel, event_polling::EventPolling,
};

#[cfg(test)]
mod tests;

pub struct PickerOptions {
    pub allow_multiple: bool,
    pub default_selected: Vec<usize>,
    /// Indices of rows the user cannot toggle. They render dimmed and are never
    /// returned as picks; the cursor may still rest on them to read their help.
    pub disabled: Vec<usize>,
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
    run_picker_with_help(_out, prompt, items, options, |_| None::<&str>)
}

pub fn run_picker_with_help<'a, Key, Value>(
    out: &mut InputOutputChannel<'_>,
    prompt: &str,
    items: &'a NonEmpty<(Key, Value)>,
    options: PickerOptions,
    help: impl Fn(&Key) -> Option<&str>,
) -> anyhow::Result<Option<Vec<&'a Value>>>
where
    Key: std::fmt::Display,
{
    let PickerOptions {
        allow_multiple,
        default_selected,
        disabled,
    } = options;

    let picks = {
        let picker_items = build_picker_items(items, &default_selected, &disabled, help);
        // Reserve a stable two-line footer (blank separator + caption) when any
        // row carries help, so the description sits below the list and the rows
        // never reflow as the cursor moves.
        let has_help = picker_items.iter().any(|item| item.help.is_some());
        let height = 1 + picker_items.len() + if has_help { 2 } else { 0 };
        let default_cursor = initial_cursor(allow_multiple, &default_selected, picker_items.len());

        let mut guard =
            CrosstermTerminalGuard::inline(height as _).context("failed to setup picker tui")?;

        let mut app = App {
            should_render: true,
            should_quit: false,
            should_confirm: false,
            allow_multiple,
            prompt: prompt.to_owned(),
            cursor: default_cursor,
            items: picker_items,
        };

        let mut event_polling = CrosstermEventPolling::default();
        let mut events = Vec::new();

        loop {
            match app.run_once(&mut guard, &mut event_polling, &mut events, out)? {
                ControlFlow::Continue(_) => {}
                ControlFlow::Break(picks) => break picks,
            }
        }
    };

    Ok(picks)
}

/// Build the picker rows, marking each row whose index appears in
/// `default_selected` as pre-selected and each in `disabled` as not togglable.
fn build_picker_items<'a, Key, Value>(
    items: &'a NonEmpty<(Key, Value)>,
    default_selected: &[usize],
    disabled: &[usize],
    help: impl Fn(&Key) -> Option<&str>,
) -> NonEmpty<PickerItem<'a, Key, Value>> {
    let default_selected_set: HashSet<usize> = default_selected.iter().copied().collect();
    let disabled_set: HashSet<usize> = disabled.iter().copied().collect();
    let mut idx = 0;
    items.as_ref().map(|(key, value)| {
        let disabled = disabled_set.contains(&idx);
        // A disabled row can never be selected, so it is never returned as a pick
        // even if a caller also lists its index in `default_selected`.
        let selected = !disabled && default_selected_set.contains(&idx);
        idx += 1;
        PickerItem {
            key,
            help: help(key).map(str::to_owned),
            value,
            selected,
            disabled,
        }
    })
}

/// The cursor's initial row. Multi-select always starts at the top; single-select
/// starts on the top-most (smallest-index) default-selected row that is in range,
/// independent of the order the caller listed the indices, else the top.
fn initial_cursor(allow_multiple: bool, default_selected: &[usize], item_count: usize) -> usize {
    if allow_multiple {
        return 0;
    }
    default_selected
        .iter()
        .copied()
        .filter(|index| *index < item_count)
        .min()
        .unwrap_or(0)
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
    help: Option<String>,
    value: &'a Value,
    selected: bool,
    disabled: bool,
}

impl<'a, Key, Value> App<'a, Key, Value>
where
    Key: std::fmt::Display,
{
    fn run_once<T, E>(
        &mut self,
        terminal_guard: &mut T,
        event_polling: E,
        events: &mut Vec<crossterm::event::Event>,
        out: &mut dyn TuiInputOutputChannel,
    ) -> anyhow::Result<ControlFlow<Option<Vec<&'a Value>>>>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
        E: EventPolling,
    {
        self.render(terminal_guard)?;

        if self.should_quit {
            return Ok(ControlFlow::Break(None));
        } else if self.should_confirm {
            if self.allow_multiple {
                return Ok(ControlFlow::Break(Some(
                    self.picks().map(|(_, value)| value).collect(),
                )));
            } else {
                let pick = self.pick();
                return Ok(ControlFlow::Break(Some(Vec::from([pick.value]))));
            }
        }

        self.update(
            terminal_guard,
            event_polling,
            events,
            out,
            &mut EmptyContext,
        )?;

        Ok(ControlFlow::Continue(()))
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn confirm(&mut self) {
        // In single-select, Enter returns the row under the cursor, so never
        // confirm on a disabled row — it must never be returned as a pick.
        // (Multi-select returns the checked rows, and disabled rows can't be
        // checked, so confirming is always fine there.)
        if !self.allow_multiple && self.items[self.cursor].disabled {
            return;
        }
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
        if selection.disabled {
            return;
        }
        selection.selected = !selection.selected;

        self.move_down();
    }

    fn view_lines(&self) -> Vec<Line<'_>> {
        let t = theme::get();

        let mut lines: Vec<Line<'_>> = Vec::new();
        lines.push(Line::from(Span::styled(self.prompt.as_str(), t.important)));

        for (idx, item) in self.items.iter().enumerate() {
            let on_cursor = self.cursor == idx;
            let cursor = if on_cursor {
                Span::styled("> ", t.info)
            } else {
                Span::raw("  ")
            };
            // Emphasize the key under the cursor so the active row reads clearly;
            // dim disabled rows so they read as unavailable.
            let key_style = if item.disabled {
                t.hint
            } else if on_cursor {
                t.important
            } else {
                t.default
            };

            if self.allow_multiple {
                let checkbox = if item.disabled {
                    Span::styled("[-] ", t.hint)
                } else if item.selected {
                    Span::styled("[x] ", t.success)
                } else {
                    Span::styled("[ ] ", t.hint)
                };
                lines.push(Line::from_iter([
                    cursor,
                    checkbox,
                    Span::styled(item.key.to_string(), key_style),
                ]));
            } else {
                lines.push(Line::from_iter([
                    cursor,
                    Span::styled(item.key.to_string(), key_style),
                ]));
            }
        }

        if self.items.iter().any(|item| item.help.is_some()) {
            lines.push(Line::default());
            let caption = match self.items[self.cursor].help.as_deref() {
                Some(help) => Line::from_iter([Span::raw("  "), Span::styled(help, t.hint)]),
                None => Line::default(),
            };
            lines.push(caption);
        }

        lines
    }

    fn picks(&self) -> impl Iterator<Item = (&'a Key, &'a Value)> {
        self.items
            .iter()
            .filter(|item| item.selected)
            .map(|item| (item.key, item.value))
    }

    fn pick(&self) -> &PickerItem<'a, Key, Value> {
        &self.items[self.cursor]
    }
}

impl<'a, Key, Value> Tui for App<'a, Key, Value>
where
    Key: std::fmt::Display,
{
    type UpdateContext<'b> = EmptyContext;

    fn update<T, E>(
        &mut self,
        _terminal_guard: &mut T,
        event_polling: E,
        events: &mut Vec<crossterm::event::Event>,
        _out: &mut dyn TuiInputOutputChannel,
        _: &mut Self::UpdateContext<'_>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
        E: EventPolling,
    {
        events.clear();
        event_polling.poll_into(Duration::from_millis(30), events)?;
        for event in events.drain(..) {
            if self.should_confirm || self.should_quit {
                break;
            }

            match event {
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

        Ok(())
    }

    fn render<T>(&mut self, terminal_guard: &mut T) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let t = theme::get();

        if self.should_quit {
            render_final_frame(terminal_guard, |frame, area| {
                frame.render_widget(
                    Line::from_iter([
                        Span::styled(self.prompt.clone(), t.hint),
                        Span::styled(" · ", t.hint),
                        Span::raw("Aborted").red(),
                    ]),
                    area,
                );
                1
            })?;
            return Ok(());
        }

        if self.should_confirm {
            if self.allow_multiple {
                render_final_frame(terminal_guard, |frame, area| {
                    let mut picks = self.picks().peekable();
                    if picks.peek().is_none() {
                        frame.render_widget(
                            Line::from_iter([
                                Span::styled(self.prompt.clone(), t.hint),
                                Span::styled(" · ", t.hint),
                                Span::raw("None").red(),
                            ]),
                            area,
                        );
                        return 1;
                    }

                    let mut lines = Vec::new();
                    lines.push(Line::from(Span::styled(self.prompt.clone(), t.hint)));
                    for (key, _) in picks {
                        lines.push(Line::from_iter([
                            Span::raw("  "),
                            Span::styled("[x] ", t.success),
                            Span::raw(key.to_string()),
                        ]));
                    }
                    let used = lines.len() as u16;
                    frame.render_widget(List::new(lines), area);
                    used
                })?;
            } else {
                render_final_frame(terminal_guard, |frame, area| {
                    let pick = self.pick();
                    frame.render_widget(
                        Line::from_iter([
                            Span::styled(self.prompt.clone(), t.hint),
                            Span::styled(" · ", t.hint),
                            Span::styled(pick.key.to_string(), t.success),
                        ]),
                        area,
                    );
                    1
                })?;
            }
            return Ok(());
        }

        if std::mem::take(&mut self.should_render) {
            terminal_guard.terminal_mut().draw(|frame| {
                frame.render_widget(List::new(self.view_lines()), frame.area());
            })?;
        }

        Ok(())
    }
}

/// Render the picker's final (collapsed) frame. The closure draws the summary
/// and returns how many rows it used, so the cursor can be parked just below it
/// and subsequent output overwrites the now-unused rows of the inline viewport.
fn render_final_frame<T, F>(terminal_guard: &mut T, f: F) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    F: FnOnce(&mut Frame<'_>, Rect) -> u16,
{
    terminal_guard.terminal_mut().draw(|frame| {
        let area = frame.area();

        frame.render_widget(Clear, area);
        let used = f(frame, area).clamp(1, area.height.max(1));

        // so subsequent prints show up in the right place
        frame.set_cursor_position((0, area.y + used));
    })?;

    Ok(())
}
