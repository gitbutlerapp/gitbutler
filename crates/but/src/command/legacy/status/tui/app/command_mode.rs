use std::{ffi::OsString, process::Command};

use anyhow::{Context as _, anyhow};
use crossterm::event::Event;
use ratatui::{backend::Backend, prelude::*};
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    command::legacy::status::tui::{
        App, Message, Mode, NormalMode, ReloadCause, ToastKind, TuiInputOutputChannel,
        format_error_for_tui,
        mode::{DetailsMode, ModeRef},
        render::ModeRender,
    },
    tui::TerminalGuard,
    utils::binary_path::current_exe_for_but_exec,
};

#[derive(Debug, Clone)]
pub struct CommandMode {
    pub textarea: Box<TextArea<'static>>,
    pub kind: CommandModeKind,
    pub return_mode: CommandReturnMode,
}

#[derive(Debug, Clone)]
pub enum CommandReturnMode {
    Normal(NormalMode),
    Details(DetailsMode),
}

impl CommandReturnMode {
    pub fn as_ref(&self) -> ModeRef<'_> {
        match self {
            CommandReturnMode::Normal(inner) => ModeRef::Normal(inner),
            CommandReturnMode::Details(inner) => ModeRef::Details(inner),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum CommandModeKind {
    But,
    Shell,
}

#[derive(Debug)]
pub enum CommandMessage {
    Start(CommandModeKind),
    Input(Event),
    Confirm,
}

impl ModeRender for CommandMode {
    fn render_hot_bar_content(&self, _app: &App, area: Rect, frame: &mut Frame) {
        let command_layout = Layout::horizontal([
            match self.kind {
                CommandModeKind::But => Constraint::Length(4),
                CommandModeKind::Shell => Constraint::Length(2),
            },
            Constraint::Min(1),
        ])
        .split(area);

        match self.kind {
            CommandModeKind::But => {
                frame.render_widget("but ", command_layout[0]);
            }
            CommandModeKind::Shell => {
                frame.render_widget("$ ", command_layout[0]);
            }
        }
        frame.render_widget(&*self.textarea, command_layout[1]);
    }
}

impl App {
    pub fn handle_command<T>(
        &mut self,
        message: CommandMessage,
        terminal_guard: &mut T,
        out: &mut dyn TuiInputOutputChannel,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        match message {
            CommandMessage::Start(kind) => self.handle_command_start(kind),
            CommandMessage::Input(ev) => self.handle_command_input(ev),
            CommandMessage::Confirm => {
                self.handle_command_confirm(terminal_guard, out, messages)?
            }
        }

        Ok(())
    }

    fn handle_command_start(&mut self, kind: CommandModeKind) {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(self.theme.default);
        textarea.move_cursor(CursorMove::End);

        self.mode.update(&mut self.backstack, |backstack, mode| {
            let previous_mode = std::mem::take(mode);
            let return_mode = match previous_mode {
                Mode::Normal(normal_mode) => CommandReturnMode::Normal(normal_mode),
                Mode::Details(details_mode) => CommandReturnMode::Details(details_mode),
                Mode::Rub(..)
                | Mode::InlineReword(..)
                | Mode::Command(..)
                | Mode::Commit(..)
                | Mode::Move(..)
                | Mode::Stack(..)
                | Mode::MoveStack(..)
                | Mode::PickChanges(..)
                | Mode::Jump(..) => CommandReturnMode::Normal(NormalMode::default()),
            };
            backstack.push_leave_command_mode();

            *mode = Mode::Command(CommandMode {
                textarea: Box::new(textarea),
                kind,
                return_mode,
            });
        });
    }

    pub(super) fn restore_mode_before_command(&mut self) -> bool {
        self.mode.update(&mut self.backstack, |backstack, mode| {
            let _ = backstack;
            let previous_mode = std::mem::take(mode);
            let Mode::Command(CommandMode { return_mode, .. }) = previous_mode else {
                *mode = previous_mode;
                return false;
            };

            *mode = match return_mode {
                CommandReturnMode::Normal(normal_mode) => Mode::Normal(normal_mode),
                CommandReturnMode::Details(details_mode) => Mode::Details(details_mode),
            };
            true
        })
    }

    fn handle_command_input(&mut self, ev: Event) {
        if let Mode::Command(CommandMode { textarea, .. }) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        {
            textarea.input(ev);
        }
    }

    fn handle_command_confirm<T>(
        &mut self,
        terminal_guard: &mut T,
        out: &mut dyn TuiInputOutputChannel,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        // `cfg!(test)` is false for integration tests but we currently don't have integration
        // tests of the TUI so thats fine for now.
        const IN_TEST: bool = cfg!(test);

        let Mode::Command(CommandMode { textarea, kind, .. }) = &*self.mode else {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        };

        let Some(input) = textarea.lines().first() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;

        let mut cmd = match kind {
            CommandModeKind::But => {
                let binary_path = current_exe_for_but_exec()?;
                let args = match shell_words::split(input) {
                    Ok(args) => args.into_iter().map(OsString::from),
                    Err(err) => {
                        self.push_transient_error(err.into());
                        return Ok(());
                    }
                };
                let mut cmd = Command::new(binary_path);
                cmd.args(args);
                cmd
            }
            CommandModeKind::Shell => {
                let mut args = match shell_words::split(input) {
                    Ok(args) => args.into_iter().map(OsString::from),
                    Err(err) => {
                        self.push_transient_error(err.into());
                        return Ok(());
                    }
                };
                let Some(binary) = args.next() else {
                    return Ok(());
                };
                let mut cmd = Command::new(binary);
                cmd.args(args);
                cmd
            }
        };

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    self.push_transient_error(anyhow!("command not found"));
                    return Ok(());
                } else {
                    return Err(err).context("failed to start command");
                }
            }
        };
        let status = child.wait()?;

        if !IN_TEST {
            out.prompt_single_line("\npress enter to continue...")?;
        }

        if status.success() {
            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(None, ReloadCause::Mutation),
            ]);
        } else {
            self.push_transient_error(anyhow!(
                "command exited with status {}",
                format_exit_status(status)
            ));
        }

        drop(_suspend_guard);

        Ok(())
    }

    /// Adds a transient error toast message that auto-dismisses after a short duration.
    fn push_transient_error(&mut self, err: anyhow::Error) {
        self.toasts
            .insert(ToastKind::Error, format_error_for_tui(&err));
    }
}

/// Formats an exit status for human-readable error messages.
fn format_exit_status(status: std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        code.to_string()
    } else {
        status.to_string()
    }
}
