use but_ctx::Context;
use gix::prelude::ObjectIdExt as _;
use ratatui::text::Span;

use crate::{
    CliId,
    command::{
        legacy::status::{
            output::StatusOutputLineData,
            tui::{
                Message, ReloadCause, SelectAfterReload,
                app::App,
                mode::Mode,
                render::{
                    ModeRender, RenderSingleLineSpans, SpanExt as _, worktree_operation_display,
                },
                toast::ToastKind,
            },
        },
        worktree::{self, new::NewOperation},
    },
};

use super::MoveCursorDiration;

#[derive(Debug, Clone)]
pub struct WorktreeMode {}

impl ModeRender for WorktreeMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if let Some(display) = worktree_operation_display(data, self) {
            line.extend([
                Span::raw("<< ").mode_colors(&*app.mode, app.theme),
                Span::raw(display).mode_colors(&*app.mode, app.theme),
                Span::raw(" >>").mode_colors(&*app.mode, app.theme),
                Span::raw(" "),
            ]);
        }
    }
}

impl WorktreeMode {}

#[derive(Debug)]
pub enum WorktreeMessage {
    Start,
    New,
}

impl App {
    #[allow(warnings, clippy::ptr_arg)]
    pub fn handle_worktree(
        &mut self,
        worktree_message: WorktreeMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match worktree_message {
            WorktreeMessage::Start => self.handle_worktree_start(),
            WorktreeMessage::New => self.handle_worktree_new(ctx, messages)?,
        }

        Ok(())
    }

    fn handle_worktree_start(&mut self) {
        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Worktree(WorktreeMode {});
            });
        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Down);
    }

    fn handle_worktree_new(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let operation = match &selection.data {
            StatusOutputLineData::MergeBase => NewOperation {
                ref_name: None,
                base: None,
            },
            StatusOutputLineData::Commit { cli_id, .. } => {
                let CliId::Commit { commit, .. } = &**cli_id else {
                    return Ok(());
                };

                let repo = ctx.repo.get()?;
                let selected_commit = but_core::Commit::from_id(commit.commit_id.attach(&repo))?;
                if selected_commit.is_conflicted() {
                    self.toasts.insert(
                        ToastKind::Info,
                        "Cannot create a worktree from a conflicted commit",
                    );
                    return Ok(());
                }

                NewOperation {
                    ref_name: None,
                    base: Some(commit.commit_id),
                }
            }

            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::Worktree { .. }
            | StatusOutputLineData::WorktreeUncommitted { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::Branch { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        let mut guard = ctx.exclusive_worktree_access();
        let outcome = worktree::new::run(ctx, guard.write_permission(), operation)?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Worktree(outcome.created.name)),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }
}
