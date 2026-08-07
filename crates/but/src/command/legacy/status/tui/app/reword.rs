use but_ctx::Context;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use gix::refs::{Category, FullName};
use ratatui::backend::Backend;
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    CliId, CliResultExt,
    command::legacy::{
        reword2::{self, BranchNameSource, CommitMessageSource, RewordOperation, RewordOutcome},
        status::tui::{
            App, Message, Mode, ReloadCause, SelectAfterReload, operations, render::ModeRender,
        },
    },
    id::CommitId,
    tui::TerminalGuard,
};

#[derive(Debug, Clone)]
pub enum InlineRewordMode {
    Commit {
        commit_id: CommitId,
        textarea: Box<TextArea<'static>>,
    },
    Branch {
        name: FullName,
        textarea: Box<TextArea<'static>>,
    },
}

impl ModeRender for InlineRewordMode {}

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

#[derive(Debug)]
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
        let Some(target) = self.selected_commit_id() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;

        let mut guard = ctx.exclusive_worktree_access();
        let mut meta = ctx.meta()?;
        let (outcome, _ws) = reword2::run(
            ctx,
            &mut meta,
            guard.write_permission(),
            RewordOperation::Commit {
                target: target.clone(),
                new_message: CommitMessageSource::Editor { initial: None },
            },
        )
        .into_internal_error()?;

        let Some(what_to_select) = reword_outcome_to_selection(outcome) else {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        };

        messages.push(Message::Reload(Some(what_to_select), ReloadCause::Mutation));

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
            CliId::Branch(branch) => {
                let mut textarea = TextArea::from([branch.name.as_str()]);
                textarea.set_cursor_line_style(self.theme.local_branch);
                textarea.move_cursor(CursorMove::End);

                InlineRewordMode::Branch {
                    name: Category::LocalBranch.to_full_name(&*branch.name)?,
                    textarea: Box::new(textarea),
                }
            }
            CliId::Commit { commit, .. } => {
                let current_message = operations::current_commit_message(ctx, commit.commit_id)?;

                if operations::commit_message_has_multiple_lines_legacy(&current_message) {
                    messages.push(Message::Reword(RewordMessage::WithEditor));
                    return Ok(());
                }

                let first_line = current_message.lines().next().unwrap_or("").to_string();
                let mut textarea = TextArea::from([first_line]);
                textarea.set_cursor_line_style(self.theme.default);
                textarea.move_cursor(CursorMove::End);

                InlineRewordMode::Commit {
                    commit_id: commit.clone(),
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
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
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

        let mut guard = ctx.exclusive_worktree_access();
        let mut meta = ctx.meta()?;

        match inline_reword_mode {
            InlineRewordMode::Commit {
                commit_id: target, ..
            } => {
                let (outcome, _ws) = reword2::run(
                    ctx,
                    &mut meta,
                    guard.write_permission(),
                    RewordOperation::Commit {
                        target: target.clone(),
                        new_message: CommitMessageSource::Provided(first_line.to_owned()),
                    },
                )
                .into_internal_error()?;

                let Some(what_to_select) = reword_outcome_to_selection(outcome) else {
                    messages.push(Message::EnterNormalModeAfterConfirmingOperation);
                    return Ok(());
                };

                messages.extend([
                    Message::EnterNormalModeAfterConfirmingOperation,
                    Message::Reload(Some(what_to_select), ReloadCause::Mutation),
                ]);
            }
            InlineRewordMode::Branch { name: target, .. } => {
                let (outcome, _ws) = reword2::run(
                    ctx,
                    &mut meta,
                    guard.write_permission(),
                    RewordOperation::Branch {
                        target: target.clone(),
                        new_name: BranchNameSource::Provided(first_line.to_owned()),
                    },
                )
                .into_internal_error()?;

                let Some(what_to_select) = reword_outcome_to_selection(outcome) else {
                    messages.push(Message::EnterNormalModeAfterConfirmingOperation);
                    return Ok(());
                };

                messages.extend([
                    Message::EnterNormalModeAfterConfirmingOperation,
                    Message::Reload(Some(what_to_select), ReloadCause::Mutation),
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

        let mut guard = ctx.exclusive_worktree_access();
        let mut meta = ctx.meta()?;

        let _suspend_guard = terminal_guard.suspend()?;
        let (outcome, _ws) = match inline_reword_mode {
            InlineRewordMode::Commit {
                commit_id: target, ..
            } => reword2::run(
                ctx,
                &mut meta,
                guard.write_permission(),
                RewordOperation::Commit {
                    target: target.clone(),
                    new_message: CommitMessageSource::Editor {
                        initial: Some(line.to_owned()),
                    },
                },
            )
            .into_internal_error()?,
            InlineRewordMode::Branch { name: target, .. } => reword2::run(
                ctx,
                &mut meta,
                guard.write_permission(),
                RewordOperation::Branch {
                    target: target.clone(),
                    new_name: BranchNameSource::Editor {
                        initial: Some(line.to_owned()),
                    },
                },
            )
            .into_internal_error()?,
        };
        drop(_suspend_guard);

        let Some(what_to_select) = reword_outcome_to_selection(outcome) else {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        };

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(Some(what_to_select), ReloadCause::Mutation),
        ]);

        Ok(())
    }
}

fn reword_outcome_to_selection(outcome: RewordOutcome) -> Option<SelectAfterReload> {
    match outcome {
        RewordOutcome::CommitUpdated {
            new_commit: commit, ..
        } => Some(SelectAfterReload::Commit(commit.commit_id)),
        RewordOutcome::BranchRenamed { new_name, .. } => Some(SelectAfterReload::Branch(new_name)),
        RewordOutcome::CommitUnchanged { .. } | RewordOutcome::BranchUnchanged { .. } => None,
    }
}
