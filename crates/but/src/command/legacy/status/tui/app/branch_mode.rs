use but_ctx::Context;
use gix::refs::Category;
use ratatui::text::Span;

use crate::{
    CliId,
    command::legacy::{
        branch::{
            self,
            new::{
                NewOperation, NewStackedBranchOperation, NewStackedBranchTarget,
                NewUnstackedBranchOperation,
            },
        },
        status::{
            output::StatusOutputLineData,
            tui::{
                Message, ReloadCause, SelectAfterReload,
                app::{
                    App,
                    mark::{Marks, MarksRef},
                },
                mode::Mode,
                render::{
                    ModeRender, RenderSingleLineSpans, SpanExt as _, branch_operation_display,
                },
            },
        },
    },
    utils::targeting::Side,
};

use super::MoveCursorDiration;

#[derive(Debug, Clone)]
pub struct BranchMode {
    pub marks: Marks,
}

impl ModeRender for BranchMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        let Some(display) = branch_operation_display(data, self) else {
            return;
        };
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(display).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    }
}

#[derive(Debug)]
pub enum BranchMessage {
    Start,
    Switch,
    New { switch: bool },
}

impl App {
    pub fn handle_branch(
        &mut self,
        branch_message: BranchMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match branch_message {
            BranchMessage::Start => self.handle_branch_start(messages),
            BranchMessage::Switch => self.handle_branch_switch(ctx, messages)?,
            BranchMessage::New { switch } => self.handle_branch_new(ctx, messages, switch)?,
        }

        Ok(())
    }

    fn handle_branch_start(&mut self, _messages: &mut Vec<Message>) {
        if !matches!(
            self.mode.marks_ref(),
            MarksRef::Branches { .. } | MarksRef::Empty
        ) {
            return;
        }

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Branch(BranchMode {
                    marks: mode.marks_ref().to_owned(),
                });
            });

        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Up);
    }

    fn handle_branch_switch(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return Ok(());
        };

        let CliId::Branch(branch_id) = &**selection else {
            return Ok(());
        };

        let branch = Category::LocalBranch.to_full_name(&*branch_id.name)?;

        // TODO(david): we should rewrite `but switch` to use the new command architecture and
        // share the "switch" code path with this
        let mut guard = ctx.exclusive_worktree_access();
        but_api::branch::branch_checkout_with_perm(ctx, branch, guard.write_permission())?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(branch_id.name.clone())),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }

    fn handle_branch_new(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
        switch: bool,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let new_name = match &selection.data {
            StatusOutputLineData::Branch { cli_id, .. } => {
                let CliId::Branch(branch) = &**cli_id else {
                    return Ok(());
                };

                let mut guard = ctx.exclusive_worktree_access();
                let mut meta = ctx.meta()?;

                let outcome = branch::new::run(
                    ctx,
                    &mut meta,
                    guard.write_permission(),
                    NewOperation::NewStackedBranch(NewStackedBranchOperation {
                        name: None,
                        target: NewStackedBranchTarget::Branch(
                            Category::LocalBranch.to_full_name(&*branch.name)?,
                        ),
                        side: Side::Above,
                        switch,
                    }),
                )?;

                outcome.name.shorten().to_string()
            }
            StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UncommittedFile { .. } => {
                let mut guard = ctx.exclusive_worktree_access();
                let mut meta = ctx.meta()?;

                let outcome = branch::new::run(
                    ctx,
                    &mut meta,
                    guard.write_permission(),
                    NewOperation::NewUnstackedBranch(NewUnstackedBranchOperation {
                        name: None,
                        switch,
                    }),
                )?;

                outcome.name.shorten().to_string()
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(new_name)),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }
}
