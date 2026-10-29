use std::{ops::ControlFlow, time::Duration};

use anyhow::Context as _;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use nonempty::NonEmpty;
use ratatui::{prelude::Backend, style::Style};

use crate::{
    command::legacy::{
        status::fuzzy_picker::{FuzzyPickerState, render_fuzzy_picker},
        switch::SwitchBranchItem,
    },
    theme::Theme,
    tui::{
        CrosstermTerminalGuard, EmptyContext, TerminalGuard, Tui, TuiInputOutputChannel,
        event_polling::{CrosstermEventPolling, EventPolling},
        render_final_frame,
    },
    utils::InputOutputChannel,
};

#[cfg(test)]
mod tests;

pub fn run_fuzzy_branch_picker(
    out: &mut InputOutputChannel<'_>,
    items: NonEmpty<SwitchBranchItem>,
) -> anyhow::Result<Option<SwitchBranchItem>> {
    let mut guard =
        CrosstermTerminalGuard::inline(10).context("failed to setup fuzzy picker tui")?;

    let mut app = App::new(items);

    let mut event_polling = CrosstermEventPolling::default();
    let mut events = Vec::new();

    let pick = loop {
        match app.run_once(&mut guard, &mut event_polling, &mut events, out)? {
            ControlFlow::Continue(_) => {}
            ControlFlow::Break(pick) => break pick,
        }
    };

    Ok(pick)
}

struct App {
    state: FuzzyPickerState<SwitchBranchItem>,
    should_confirm: bool,
    should_quit: bool,
    should_render: bool,
    theme: Theme,
}

impl App {
    fn new(items: NonEmpty<SwitchBranchItem>) -> App {
        let mut theme = crate::theme::get().clone();
        theme.default = Style::default();
        theme.local_branch = Style::default();
        theme.info = theme.hint;
        theme.selection_highlight = Style {
            bg: theme.selection_highlight.bg,
            ..Style::default()
        };

        let state = FuzzyPickerState::new(items, &theme, Some("Select branch"));

        App {
            state,
            theme,
            should_confirm: false,
            should_quit: false,
            should_render: true,
        }
    }

    fn run_once<T, E>(
        &mut self,
        terminal_guard: &mut T,
        event_polling: E,
        events: &mut Vec<crossterm::event::Event>,
        out: &mut dyn TuiInputOutputChannel,
    ) -> anyhow::Result<ControlFlow<Option<SwitchBranchItem>>>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
        E: EventPolling,
    {
        self.render(terminal_guard)?;

        if self.should_quit {
            return Ok(ControlFlow::Break(None));
        } else if self.should_confirm {
            return Ok(ControlFlow::Break(self.state.selected_item().cloned()));
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
}

impl Tui for App {
    type UpdateContext<'b> = EmptyContext;

    fn update<T, E>(
        &mut self,
        _terminal_guard: &mut T,
        event_polling: E,
        events: &mut Vec<crossterm::event::Event>,
        _out: &mut dyn TuiInputOutputChannel,
        _update_ctx: &mut Self::UpdateContext<'_>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as ratatui::prelude::Backend>::Error>,
        E: EventPolling,
    {
        events.clear();
        event_polling.poll_into(Duration::from_millis(30), events)?;
        for event in events.drain(..) {
            if self.should_confirm || self.should_quit {
                break;
            }

            let mut handled = false;
            match event {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    handled = true;
                    match key_event.code {
                        KeyCode::Char(c) => match c {
                            'c' | 'd' if key_event.modifiers == KeyModifiers::CONTROL => {
                                self.quit();
                            }
                            'n' if key_event.modifiers == KeyModifiers::CONTROL => {
                                self.state.move_cursor_down();
                            }
                            'p' if key_event.modifiers == KeyModifiers::CONTROL => {
                                self.state.move_cursor_up();
                            }
                            _ => handled = false,
                        },
                        KeyCode::Enter => {
                            self.should_confirm = self.state.selected_item().is_some();
                        }
                        KeyCode::Esc => {
                            self.quit();
                        }
                        KeyCode::Up => {
                            self.state.move_cursor_up();
                        }
                        KeyCode::Down => {
                            self.state.move_cursor_down();
                        }
                        _ => handled = false,
                    }
                }
                Event::Key(..)
                | Event::Paste(_)
                | Event::Resize(_, _)
                | Event::FocusGained
                | Event::FocusLost
                | Event::Mouse(_) => {}
            }

            if !handled {
                self.state.input(event);
            }

            self.should_render = true;
        }

        Ok(())
    }

    fn render<T>(&mut self, terminal_guard: &mut T) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as ratatui::prelude::Backend>::Error>,
    {
        if self.should_quit {
            render_final_frame(terminal_guard, |_frame, _area| 0)?;
            return Ok(());
        }

        if self.should_confirm {
            render_final_frame(terminal_guard, |_frame, _area| 0)?;
            return Ok(());
        }

        if std::mem::take(&mut self.should_render) {
            terminal_guard.terminal_mut().draw(|frame| {
                render_fuzzy_picker(&self.state, &self.theme, true, frame.area(), frame);
            })?;
        }

        Ok(())
    }
}
