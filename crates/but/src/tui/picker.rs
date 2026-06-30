use std::{collections::HashSet, time::Duration};

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

use crate::{theme, tui::TerminalGuard as _, utils::InputOutputChannel};

pub struct PickerOptions {
    pub allow_multiple: bool,
    pub default_selected: Vec<usize>,
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
    _out: &mut InputOutputChannel<'_>,
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
    } = options;

    let picks = {
        let picker_items = build_picker_items(items, &default_selected, help);
        // Reserve a stable two-line footer (blank separator + caption) when any
        // row carries help, so the description sits below the list and the rows
        // never reflow as the cursor moves.
        let has_help = picker_items.iter().any(|item| item.help.is_some());
        let height = 1 + picker_items.len() + if has_help { 2 } else { 0 };
        let default_cursor = initial_cursor(allow_multiple, &default_selected, picker_items.len());

        let mut guard = super::CrosstermTerminalGuard::inline(height as _)
            .context("failed to setup picker tui")?;

        let app = App {
            should_render: true,
            should_quit: false,
            should_confirm: false,
            allow_multiple,
            prompt: prompt.to_owned(),
            cursor: default_cursor,
            items: picker_items,
        };

        app.run(guard.terminal_mut())?
    };

    Ok(picks)
}

/// Build the picker rows, marking each row whose index appears in
/// `default_selected` as pre-selected.
fn build_picker_items<'a, Key, Value>(
    items: &'a NonEmpty<(Key, Value)>,
    default_selected: &[usize],
    help: impl Fn(&Key) -> Option<&str>,
) -> NonEmpty<PickerItem<'a, Key, Value>> {
    let default_selected_set: HashSet<usize> = default_selected.iter().copied().collect();
    let mut idx = 0;
    items.as_ref().map(|(key, value)| {
        let selected = default_selected_set.contains(&idx);
        idx += 1;
        PickerItem {
            key,
            help: help(key).map(str::to_owned),
            value,
            selected,
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
                let t = theme::get();
                render_final_frame(terminal, |frame, area| {
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
                break Ok(None);
            }

            if self.should_confirm {
                let t = theme::get();
                if self.allow_multiple {
                    let picks = self
                        .items
                        .iter()
                        .filter(|item| item.selected)
                        .map(|item| (item.key, item.value))
                        .collect::<Vec<_>>();
                    render_final_frame(terminal, |frame, area| {
                        if picks.is_empty() {
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
                        // Show the prompt, then one checked line per pick, so a
                        // multi-select receipt stays readable instead of a long
                        // comma-joined line.
                        let mut lines = Vec::with_capacity(picks.len() + 1);
                        lines.push(Line::from(Span::styled(self.prompt.clone(), t.hint)));
                        for (key, _) in &picks {
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
                    break Ok(Some(picks.into_iter().map(|(_, value)| value).collect()));
                } else {
                    let pick = &self.items[self.cursor];
                    render_final_frame(terminal, |frame, area| {
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
        frame.render_widget(List::new(self.view_lines()), frame.area());
    }

    /// Build the picker's display lines: the prompt, one line per row, then a
    /// pinned two-line footer (blank separator + the current row's help) when
    /// any row carries help. The footer keeps the help on its own line below the
    /// list so the rows never reflow as the cursor moves.
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
            // Emphasize the key under the cursor so the active row reads clearly.
            let key_style = if on_cursor { t.important } else { t.default };

            if self.allow_multiple {
                let checkbox = if item.selected {
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
}

/// Render the picker's final (collapsed) frame. The closure draws the summary
/// and returns how many rows it used, so the cursor can be parked just below it
/// and subsequent output overwrites the now-unused rows of the inline viewport.
fn render_final_frame<B, F>(terminal: &mut Terminal<B>, f: F) -> anyhow::Result<()>
where
    B: Backend<Error: Send + Sync + 'static>,
    F: FnOnce(&mut Frame<'_>, Rect) -> u16,
{
    terminal.draw(|frame| {
        let area = frame.area();

        frame.render_widget(Clear, area);
        let used = f(frame, area).clamp(1, area.height.max(1));

        // so subsequent prints show up in the right place
        frame.set_cursor_position((0, area.y + used));
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    type Row = (&'static str, Option<&'static str>, bool);

    /// Build a picker from `rows`, render its view lines, and flatten each line
    /// to plain text (styling dropped) for assertions.
    fn render_texts(allow_multiple: bool, cursor: usize, rows: &[Row]) -> Vec<String> {
        let keys: Vec<String> = rows.iter().map(|(key, _, _)| key.to_string()).collect();
        let items = rows
            .iter()
            .copied()
            .enumerate()
            .map(|(i, (_, help, selected))| PickerItem {
                key: &keys[i],
                help: help.map(str::to_owned),
                value: &(),
                selected,
            })
            .collect::<Vec<_>>();
        let app = App {
            should_render: true,
            should_quit: false,
            should_confirm: false,
            allow_multiple,
            prompt: "Pick one".to_string(),
            cursor,
            items: NonEmpty::from_vec(items).expect("non-empty rows"),
        };
        app.view_lines()
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn help_is_a_pinned_footer_and_rows_do_not_reflow() {
        let rows: [Row; 3] = [
            ("Codex", Some("Use Codex."), true),
            ("Claude", Some("Use Claude."), false),
            ("Cursor", Some("Use Cursor."), false),
        ];

        let top = render_texts(true, 0, &rows);
        // prompt + 3 rows + blank separator + caption.
        assert_eq!(top.len(), 6);
        assert_eq!(top[0], "Pick one");
        assert!(top[1].contains("Codex"));
        assert!(top[2].contains("Claude"));
        assert!(top[3].contains("Cursor"));
        assert_eq!(top[4], "", "separator line is blank");
        assert!(
            top[5].contains("Use Codex."),
            "footer shows cursor-row help"
        );

        // Moving the cursor keeps every row at the same index (no reflow) and
        // only swaps the footer caption.
        let mid = render_texts(true, 1, &rows);
        assert_eq!(mid.len(), 6);
        assert!(mid[1].contains("Codex"));
        assert!(mid[2].contains("Claude"));
        assert!(mid[3].contains("Cursor"));
        assert!(mid[5].contains("Use Claude."), "footer tracks the cursor");
    }

    #[test]
    fn no_footer_when_no_row_has_help() {
        let rows: [Row; 2] = [("Codex", None, false), ("Claude", None, false)];
        // prompt + 2 rows, no footer reserved.
        assert_eq!(render_texts(true, 0, &rows).len(), 3);
    }

    #[test]
    fn single_select_cursor_starts_at_topmost_default_in_range() {
        assert_eq!(initial_cursor(false, &[2], 5), 2);
        assert_eq!(initial_cursor(false, &[], 5), 0, "no default starts at top");
        assert_eq!(
            initial_cursor(false, &[9], 5),
            0,
            "out-of-range default falls back to top"
        );
        assert_eq!(
            initial_cursor(false, &[3, 1], 5),
            1,
            "picks the top-most selected row, not the first listed"
        );
        assert_eq!(
            initial_cursor(false, &[4, 9], 5),
            4,
            "ignores out-of-range indices when picking the top-most"
        );
        assert_eq!(
            initial_cursor(true, &[2], 5),
            0,
            "multi-select always starts at top"
        );
    }

    #[test]
    fn multi_select_marks_default_indices_selected() {
        let items = NonEmpty::from_vec(vec![("a", ()), ("b", ()), ("c", ()), ("d", ())])
            .expect("non-empty");
        let built = build_picker_items(&items, &[0, 2], |_| None::<&str>);
        let selected = built.iter().map(|item| item.selected).collect::<Vec<_>>();
        assert_eq!(selected, vec![true, false, true, false]);
    }

    #[test]
    fn single_select_rows_have_no_checkbox() {
        let rows: [Row; 2] = [
            ("Apply", Some("Do it."), false),
            ("Cancel", Some("Stop."), false),
        ];
        let lines = render_texts(false, 0, &rows);
        // prompt + 2 rows + blank separator + caption.
        assert_eq!(lines.len(), 5);
        assert!(!lines[1].contains("[x]") && !lines[1].contains("[ ]"));
        assert!(lines[1].contains("Apply"));
        assert!(
            lines[4].contains("Do it."),
            "footer caption is the last line"
        );
    }
}
