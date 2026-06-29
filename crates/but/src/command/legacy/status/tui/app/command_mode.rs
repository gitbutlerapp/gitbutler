use std::{ffi::OsString, process::Command};

use crossterm::event::Event;
use ratatui::backend::Backend;
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    command::legacy::status::tui::{
        App, Message, Mode, ReloadCause, ToastKind, TuiInputOutputChannel, format_error_for_tui,
    },
    tui::TerminalGuard,
    utils::binary_path::current_exe_for_but_exec,
};

#[derive(Debug, Clone)]
pub struct CommandMode {
    pub textarea: Box<TextArea<'static>>,
    pub kind: CommandModeKind,
}

#[derive(Debug, Copy, Clone)]
pub enum CommandModeKind {
    But,
    Shell,
}

#[derive(Debug, Clone)]
pub enum CommandMessage {
    Start(CommandModeKind),
    Input(Event),
    Confirm,
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

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Command(CommandMode {
                    textarea: Box::new(textarea),
                    kind,
                });
            });
    }

    fn handle_command_input(&mut self, ev: Event) {
        if let Mode::Command(CommandMode { textarea, .. }) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
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
        //
        // `cfg!(test)` is false for integration tests but we currently don't have integration
        // tests of the TUI so thats fine for now.
        const IN_TEST: bool = cfg!(test);

        let Mode::Command(CommandMode { textarea, kind }) = &*self.mode else {
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
                let args = shell_words::split(input)?.into_iter().map(OsString::from);
                let mut cmd = Command::new(binary_path);
                cmd.args(args);
                cmd
            }
            CommandModeKind::Shell => {
                let mut args = shell_words::split(input)?.into_iter().map(OsString::from);
                let Some(binary) = args.next() else {
                    messages.push(Message::EnterNormalModeAfterConfirmingOperation);
                    return Ok(());
                };
                let mut cmd = Command::new(binary);
                cmd.args(args);
                cmd
            }
        };

        let status = cmd.spawn()?.wait()?;

        if !IN_TEST {
            out.prompt_single_line("\npress enter to continue...")?;
        }

        if status.success() {
            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(None, ReloadCause::Mutation),
            ]);
        } else {
            self.push_transient_error(anyhow::Error::msg(format!(
                "command exited with status {}",
                format_exit_status(status)
            )));
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
