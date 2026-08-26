use std::time::SystemTime;

use anyhow::Context as _;
use bstr::ByteSlice as _;
use but_ctx::Context;
use gix::refs::Category;
use nonempty::NonEmpty;
use ratatui::{style::Style, text::Span};

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
                    App, Modal,
                    mark::{Marks, MarksRef},
                },
                fuzzy_picker::{Col, FuzzyPicker, FuzzyPickerItem, SearchableToken},
                key_bind::fuzzy_picker_key_binds,
                mode::Mode,
                render::{
                    ModeRender, RenderSingleLineSpans, SpanExt as _, branch_operation_display,
                },
            },
        },
        r#switch::{self, SwitchOperation},
    },
    theme::Theme,
    utils::{targeting::Side, time::format_relative_time},
};

use super::MoveCursorDiration;

#[derive(Debug, Clone, Default)]
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
    PickAndSwitch,
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
            BranchMessage::PickAndSwitch => self.handle_pick_and_switch(ctx)?,
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

        let mut guard = ctx.exclusive_worktree_access();
        let _outcome = r#switch::run(
            ctx,
            guard.write_permission(),
            SwitchOperation::Branch { branch },
        )?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(branch_id.name.clone())),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }

    fn handle_pick_and_switch(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let current_branch = {
            let repo = ctx.repo.get()?;
            repo.head_ref()?
                .map(|head_ref| head_ref.name().shorten().to_owned())
        };
        let branch_listings = but_api::branch::branch_list(ctx)
            .context("Failed to list branches available to switch to")?
            .into_iter()
            .flat_map(|stack| stack.branches)
            .map(|listed_branch| listed_branch.branch)
            .filter(|branch| branch.has_local)
            .filter(|branch| {
                current_branch
                    .as_ref()
                    .is_none_or(|current_branch| current_branch.as_bstr() != *branch.display_name)
            });

        let now = SystemTime::now();
        let mut branches = branch_listings
            .map(|listing| SwitchBranchItem {
                name: listing.display_name.to_str_lossy().into_owned(),
                updated_at: listing.updated_at_ms,
                updated_at_display: listing
                    .updated_at_ms
                    .map(|updated_at| format_relative_time(now, updated_at / 1000))
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        branches.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.name.cmp(&b.name))
        });

        let Some(items) = NonEmpty::from_vec(branches) else {
            return Ok(());
        };

        let picker = FuzzyPicker::new(items, self.theme, |item, ctx, messages| {
            let branch = Category::LocalBranch.to_full_name(&*item.name)?;

            // TODO(david): we should rewrite `but switch` to use the new command architecture and
            // share the "switch" code path with this
            let mut guard = ctx.exclusive_worktree_access();
            but_api::branch::branch_checkout_with_perm(ctx, branch, guard.write_permission())?;

            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(
                    Some(SelectAfterReload::Branch(item.name.clone())),
                    ReloadCause::Mutation,
                ),
            ]);

            Ok(())
        });

        self.modal = Some(Modal::SwitchBranchPicker {
            picker: Box::new(picker),
            key_binds: fuzzy_picker_key_binds(),
        });

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
            | StatusOutputLineData::WorktreeUncommittedChanges { .. }
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

#[derive(Debug, Clone)]
pub struct SwitchBranchItem {
    name: String,
    updated_at: Option<i64>,
    updated_at_display: String,
}

impl FuzzyPickerItem for SwitchBranchItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        [
            Col {
                text: self.name.as_str().into(),
                searchable: Some(searchable),
            },
            Col {
                text: self.updated_at_display.as_str().into(),
                searchable: None,
            },
        ]
    }

    fn style(&self, theme: &'static Theme) -> Style {
        theme.local_branch
    }
}
