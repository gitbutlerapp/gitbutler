use but_api::diff::ComputeLineStats;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    CliId,
    command::legacy::{
        reword::get_branch_name_from_editor,
        status::tui::{App, Message, Mode, ReloadCause, SelectAfterReload, operations},
    },
    tui::TerminalGuard,
};

#[derive(Debug, Clone)]
pub enum InlineRewordMode {
    Commit {
        commit_id: gix::ObjectId,
        textarea: Box<TextArea<'static>>,
    },
    Branch {
        name: String,
        stack_id: StackId,
        textarea: Box<TextArea<'static>>,
    },
}

impl InlineRewordMode {
    pub fn textarea(&self) -> &TextArea<'static> {
        match self {
            InlineRewordMode::Commit { textarea, .. }
            | InlineRewordMode::Branch { textarea, .. } => textarea,
        }
    }

    pub fn textarea_mut(&mut self) -> &mut TextArea<'static> {
        match self {
            InlineRewordMode::Commit { textarea, .. }
            | InlineRewordMode::Branch { textarea, .. } => textarea,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RewordMessage {
    WithEditor,
    OpenEditor,
    InlineStart,
    InlineInput(Event),
    InlineConfirm,
}

impl App {
    pub fn handle_reword<T>(
        &mut self,
        message: RewordMessage,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        match message {
            RewordMessage::WithEditor => {
                self.handle_reword_with_editor(ctx, terminal_guard, messages)?;
            }
            RewordMessage::InlineStart => self.handle_reword_inline_start(ctx, messages)?,
            RewordMessage::InlineInput(ev) => self.handle_reword_inline_input(ev),
            RewordMessage::InlineConfirm => self.handle_reword_inline_confirm(ctx, messages)?,
            RewordMessage::OpenEditor => {
                self.handle_reword_open_editor(ctx, terminal_guard, messages)?;
            }
        }

        Ok(())
    }

    /// Handles opening the full-screen commit reword editor for the selected commit.
    fn handle_reword_with_editor<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Some(commit_id) = self.selected_commit_id() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;

        let Some(reword_result) = operations::reword_commit_with_editor_legacy(ctx, commit_id)?
        else {
            return Ok(());
        };

        messages.push(Message::Reload(
            Some(SelectAfterReload::Commit(reword_result.new_commit)),
            ReloadCause::Mutation,
        ));

        Ok(())
    }

    fn handle_reword_inline_start(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return Ok(());
        };

        let inline_reword_mode = match &**cli_id {
            CliId::Branch { name, stack_id, .. } => {
                let Some(stack_id) = stack_id else {
                    return Ok(());
                };
                let mut textarea = TextArea::from([name]);
                textarea.set_cursor_line_style(self.theme.local_branch);
                textarea.move_cursor(CursorMove::End);

                InlineRewordMode::Branch {
                    name: name.to_owned(),
                    stack_id: *stack_id,
                    textarea: Box::new(textarea),
                }
            }
            CliId::Commit { commit_id, .. } => {
                let current_message = operations::current_commit_message(ctx, *commit_id)?;

                if operations::commit_message_has_multiple_lines_legacy(&current_message) {
                    messages.push(Message::Reword(RewordMessage::WithEditor));
                    return Ok(());
                }

                let first_line = current_message.lines().next().unwrap_or("").to_string();
                let mut textarea = TextArea::from([first_line]);
                textarea.set_cursor_line_style(self.theme.default);
                textarea.move_cursor(CursorMove::End);

                InlineRewordMode::Commit {
                    commit_id: *commit_id,
                    textarea: Box::new(textarea),
                }
            }
            CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Uncommitted { .. }
            | CliId::Stack { .. } => return Ok(()),
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::InlineReword(inline_reword_mode);
            });

        Ok(())
    }

    /// Handles key input while inline reword mode is active.
    fn handle_reword_inline_input(&mut self, ev: Event) {
        if let Mode::InlineReword(inline_reword_mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        {
            let ev = match inline_reword_mode {
                InlineRewordMode::Branch { .. } => {
                    if let Event::Key(key_ev) = ev
                        && key_ev.is_press()
                        && key_ev.modifiers == event::KeyModifiers::NONE
                        && let KeyCode::Char(' ') = key_ev.code
                    {
                        Event::Key(KeyEvent {
                            code: KeyCode::Char('-'),
                            modifiers: key_ev.modifiers,
                            kind: key_ev.kind,
                            state: key_ev.state,
                        })
                    } else {
                        ev
                    }
                }
                InlineRewordMode::Commit { .. } => ev,
            };

            inline_reword_mode.textarea_mut().input(ev);
        }
    }

    fn handle_reword_inline_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let inline_reword_mode = if let Mode::InlineReword(inline_reword_mode) = &*self.mode {
            inline_reword_mode
        } else {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        };

        let first_line = inline_reword_mode
            .textarea()
            .lines()
            .first()
            .map(std::string::String::as_str)
            .unwrap_or("");

        match inline_reword_mode {
            InlineRewordMode::Commit { commit_id, .. } => {
                let Some(reword_result) =
                    operations::reword_commit_legacy(ctx, *commit_id, first_line)?
                else {
                    messages.push(Message::EnterNormalModeAfterConfirmingOperation);
                    return Ok(());
                };

                messages.extend([
                    Message::EnterNormalModeAfterConfirmingOperation,
                    Message::Reload(
                        Some(SelectAfterReload::Commit(reword_result.new_commit)),
                        ReloadCause::Mutation,
                    ),
                ]);
            }
            InlineRewordMode::Branch { name, stack_id, .. } => {
                let new_name = operations::reword_branch_legacy(
                    ctx,
                    *stack_id,
                    name.to_owned(),
                    first_line.to_owned(),
                )?;

                messages.extend([
                    Message::EnterNormalModeAfterConfirmingOperation,
                    Message::Reload(
                        Some(SelectAfterReload::Branch(new_name)),
                        ReloadCause::Mutation,
                    ),
                ]);
            }
        }

        Ok(())
    }

    fn handle_reword_open_editor<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Mode::InlineReword(inline_reword_mode) = &*self.mode else {
            return Ok(());
        };

        let textarea = inline_reword_mode.textarea();
        let Some(line) = textarea.lines().first() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;
        let what_to_select = match inline_reword_mode {
            InlineRewordMode::Commit { commit_id, .. } => {
                let commit_details =
                    but_api::diff::commit_details(ctx, *commit_id, ComputeLineStats::No)?;
                if let Some(reword_result) =
                    operations::reword_commit_with_editor_with_message_legacy(
                        ctx,
                        commit_details,
                        line.to_owned(),
                    )?
                {
                    SelectAfterReload::Commit(reword_result.new_commit)
                } else {
                    SelectAfterReload::Commit(*commit_id)
                }
            }
            InlineRewordMode::Branch { name, stack_id, .. } => {
                let new_name = get_branch_name_from_editor(line)?;
                let normalized_name =
                    operations::reword_branch_legacy(ctx, *stack_id, name.clone(), new_name)?;
                SelectAfterReload::Branch(normalized_name)
            }
        };
        drop(_suspend_guard);

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(Some(what_to_select), ReloadCause::Mutation),
        ]);

        Ok(())
    }
}
