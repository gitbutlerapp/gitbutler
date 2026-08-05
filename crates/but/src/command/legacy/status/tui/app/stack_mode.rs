use std::time::SystemTime;

use anyhow::Context as _;
use bstr::ByteSlice;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use gitbutler_branch_actions::BranchListingFilter;
use gix::refs::FullName;
use nonempty::NonEmpty;
use ratatui::prelude::{Line, Span, Style, Text};

use crate::{
    CliId,
    args::atoms::BranchArg,
    command::legacy::{
        apply::{self, ApplyOperation, ApplyOutcome},
        status::{
            FilesStatusFlag, StatusOutputLine,
            output::StatusOutputLineData,
            tui::{
                App, Cursor, FuzzyPicker, Message, Modal, Mode, ReloadCause, SelectAfterReload,
                ToastKind,
                app::NormalMode,
                cursor::is_selectable_in_mode,
                fuzzy_picker::{Col, FuzzyPickerItem, SearchableToken},
                fuzzy_picker_key_binds,
                render::{
                    ModeRender, RenderSingleLineSpans, SpanExt,
                    render_move_stack_operation_target_marker, source_span,
                    stack_operation_display,
                },
            },
        },
        unapply::{self, UnapplyOperation},
    },
    id::{BranchId, CommitId, CommittedFileId},
    theme::Theme,
    utils::time::format_relative_time,
};

#[derive(Debug, Clone)]
pub struct StackMode {
    pub stack_heads: Vec<FullName>,
}

#[derive(Debug, Clone)]
pub struct MoveStackMode {
    pub source: ReorderStackSource,
}

#[derive(Debug, Clone)]
pub struct ReorderStackSource {
    pub branch: BranchId,
}

impl ModeRender for StackMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        let Some(display) = stack_operation_display(data, self) else {
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

impl ModeRender for MoveStackMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if data
            .cli_id()
            .is_some_and(|target| self.source.matches(target))
        {
            render_move_stack_operation_target_marker(app, data, self, line);
        }
    }

    fn render_operation_source_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if let Some(cli_id) = data.cli_id()
            && self.source.matches(cli_id)
        {
            line.extend([source_span(app.theme), Span::raw(" ")]);
        }
    }
}

impl ReorderStackSource {
    pub fn matches(&self, id: &CliId) -> bool {
        match id {
            CliId::Branch(branch) => self.branch == *branch,
            CliId::Stack { .. }
            | CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Commit { .. }
            | CliId::Uncommitted { .. } => false,
        }
    }
}

#[derive(Debug)]
pub enum StackMessage {
    Enter,
    ShowApplyPicker,
    Unapply,
    MoveStart,
    MoveConfirm,
}

#[derive(Debug, Clone)]
pub struct ApplyBranchItem {
    name: String,
    last_commiter: String,
    has_local: bool,
    updated_at: u128,
    updated_at_display: String,
}

impl FuzzyPickerItem for ApplyBranchItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        [
            Col {
                text: if self.has_local { "local" } else { "remote" }.into(),
                searchable: None,
            },
            Col {
                text: self.name.as_str().into(),
                searchable: Some(searchable),
            },
            Col {
                text: self.updated_at_display.as_str().into(),
                searchable: None,
            },
            Col {
                text: self.last_commiter.as_str().into(),
                searchable: None,
            },
        ]
    }

    fn style(&self, theme: &'static Theme) -> Style {
        if self.has_local {
            theme.local_branch
        } else {
            theme.remote_branch
        }
    }
}

fn line_uses_top_stack_for_stack_mode(line: &StatusOutputLine) -> bool {
    match &line.data {
        StatusOutputLineData::UncommittedChanges { .. } => true,
        StatusOutputLineData::UncommittedFile { cli_id }
        | StatusOutputLineData::StagedFile { cli_id }
        | StatusOutputLineData::File { cli_id } => {
            matches!(
                &**cli_id,
                CliId::UncommittedHunkOrFile(..) | CliId::PathPrefix { .. }
            )
        }
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::Branch { .. }
        | StatusOutputLineData::Commit { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::MergeBase
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => false,
    }
}

pub fn stack_ids_in_display_order(status_lines: &[StatusOutputLine]) -> Vec<StackId> {
    let mut stack_ids = Vec::new();
    for line in status_lines {
        if let StatusOutputLineData::Branch { cli_id, .. } = &line.data
            && let CliId::Branch(branch) = &**cli_id
            && let Some(stack_id) = branch.stack_id
            && !stack_ids.contains(&stack_id)
        {
            stack_ids.push(stack_id);
        }
    }
    stack_ids
}

fn stack_id_for_line(
    line: &StatusOutputLine,
    status_lines: &[StatusOutputLine],
) -> Option<StackId> {
    match &line.data {
        StatusOutputLineData::Branch { cli_id, .. }
        | StatusOutputLineData::StagedChanges { cli_id }
        | StatusOutputLineData::StagedFile { cli_id }
        | StatusOutputLineData::UncommittedFile { cli_id }
        | StatusOutputLineData::File { cli_id } => stack_id_for_cli_id(cli_id, status_lines),
        StatusOutputLineData::Commit { stack_id, .. } => *stack_id,
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::MergeBase
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    }
}

fn stack_id_for_cli_id(cli_id: &CliId, status_lines: &[StatusOutputLine]) -> Option<StackId> {
    let commit_matches = |other: &CommitId| match cli_id {
        CliId::CommittedFile {
            committed_file: CommittedFileId { commit_id, .. },
            id: _,
        } => other.commit_id == *commit_id,
        CliId::Commit { commit, id: _ } => other == commit,
        CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::Branch(..)
        | CliId::Uncommitted { .. }
        | CliId::Stack { .. } => false,
    };

    match cli_id {
        CliId::CommittedFile { .. } | CliId::Commit { .. } => {
            status_lines.iter().find_map(|line| match &line.data {
                StatusOutputLineData::Commit {
                    cli_id, stack_id, ..
                } => match &**cli_id {
                    CliId::Commit { commit, id: _ } if commit_matches(commit) => *stack_id,
                    CliId::UncommittedHunkOrFile(..)
                    | CliId::PathPrefix { .. }
                    | CliId::CommittedFile { .. }
                    | CliId::Branch(..)
                    | CliId::Commit { .. }
                    | CliId::Uncommitted { .. }
                    | CliId::Stack { .. } => None,
                },
                StatusOutputLineData::UpdateNotice
                | StatusOutputLineData::Connector
                | StatusOutputLineData::BetweenStacks
                | StatusOutputLineData::StagedChanges { .. }
                | StatusOutputLineData::StagedFile { .. }
                | StatusOutputLineData::UncommittedChanges { .. }
                | StatusOutputLineData::UncommittedFile { .. }
                | StatusOutputLineData::Branch { .. }
                | StatusOutputLineData::CommitMessage
                | StatusOutputLineData::EmptyCommitMessage
                | StatusOutputLineData::File { .. }
                | StatusOutputLineData::MergeBase
                | StatusOutputLineData::UpstreamChanges
                | StatusOutputLineData::Warning
                | StatusOutputLineData::Hint
                | StatusOutputLineData::NoAssignmentsUnstaged => None,
            })
        }
        CliId::Branch(branch) => branch.stack_id,
        CliId::Stack { stack_id, .. } => Some(*stack_id),
        CliId::UncommittedHunkOrFile(..) | CliId::PathPrefix { .. } | CliId::Uncommitted { .. } => {
            None
        }
    }
}

impl App {
    pub fn handle_stack(
        &mut self,
        message: StackMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match message {
            StackMessage::Enter => self.handle_stack_enter(ctx)?,
            StackMessage::ShowApplyPicker => self.handle_stack_show_apply_picker(ctx)?,
            StackMessage::Unapply => self.handle_stack_unapply(ctx, messages)?,
            StackMessage::MoveStart => self.handle_stack_move_start(),
            StackMessage::MoveConfirm => self.handle_stack_move_confirm(ctx, messages)?,
        }

        Ok(())
    }

    fn handle_stack_enter(&mut self, ctx: &Context) -> anyhow::Result<()> {
        match self.flags.show_files {
            FilesStatusFlag::Commit(..) => return Ok(()),
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }

        let head_info = but_api::legacy::workspace::head_info(ctx)?;

        let stack_heads = head_info
            .stacks
            .iter()
            .filter_map(|stack| stack.ref_name().cloned())
            .collect::<Vec<_>>();

        let Some(top_stack_head) = stack_heads.first().cloned() else {
            self.mode
                .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                    *mode = Mode::Stack(StackMode {
                        stack_heads: Default::default(),
                    });
                });
            return Ok(());
        };

        let selected_stack_head = if self.selected_line_uses_top_stack_for_stack_mode() {
            Some(top_stack_head)
        } else {
            self.selected_stack_id().and_then(|selected_stack_id| {
                head_info
                    .stacks
                    .iter()
                    .find(|stack| stack.id == Some(selected_stack_id))
                    .and_then(|stack| stack.ref_name().cloned())
            })
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Stack(StackMode { stack_heads });
            });

        if let Some(selected_stack_head) = selected_stack_head {
            let branch_name = selected_stack_head.shorten().to_str_lossy();
            if let Some(cursor) = Cursor::select_branch(&branch_name, &self.status_lines) {
                self.cursor = cursor;
            }
        }

        Ok(())
    }

    fn handle_stack_show_apply_picker(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let branch_listings = but_api::legacy::virtual_branches::list_branches(
            ctx,
            Some(BranchListingFilter {
                local: None,
                applied: Some(false),
            }),
        )
        .context("Failed to list branches available to apply")?
        .into_iter();

        let now = SystemTime::now();
        let mut branches = branch_listings
            .map(|listing| ApplyBranchItem {
                name: listing.name.0.to_string(),
                has_local: listing.has_local,
                updated_at: listing.updated_at,
                updated_at_display: format_relative_time(now, (listing.updated_at / 1000) as i64),
                last_commiter: listing
                    .last_commiter
                    .name
                    .map(|name| name.to_string())
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();

        branches.sort_by(|a, b| {
            b.has_local
                .cmp(&a.has_local)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
                .then_with(|| a.name.cmp(&b.name))
        });

        let Some(items) = NonEmpty::from_vec(branches) else {
            return Ok(());
        };
        let picker = FuzzyPicker::new(items, self.theme, |item, ctx, messages| {
            let branch = {
                let repo = ctx.repo.get()?;
                BranchArg(item.name.clone()).resolve_legacy_top_level_apply_branch_name(&repo)?
            };

            let mut guard = ctx.exclusive_worktree_access();
            match apply::run(ctx, guard.write_permission(), ApplyOperation { branch })? {
                ApplyOutcome::ConflictAborted(outcome) => {
                    anyhow::bail!("Failed to apply branch: {outcome}")
                }
                ApplyOutcome::AlreadyApplied { .. } | ApplyOutcome::Applied { .. } => {}
            }

            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(
                    Some(SelectAfterReload::Branch(item.name)),
                    ReloadCause::Mutation,
                ),
            ]);

            Ok(())
        });
        self.modal = Some(Modal::ApplyStackPicker {
            picker: Box::new(picker),
            key_binds: fuzzy_picker_key_binds(),
        });

        Ok(())
    }

    fn handle_stack_unapply(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(selection) = selection.data.cli_id() else {
            return Ok(());
        };

        let (stack_id, name) = match &**selection {
            CliId::Branch(branch) => {
                let Some(stack_id) = branch.stack_id else {
                    return Ok(());
                };
                (stack_id, &branch.name)
            }
            CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Commit { .. }
            | CliId::Uncommitted { .. }
            | CliId::Stack { .. } => return Ok(()),
        };

        let next_stack_id = stack_ids_in_display_order(&self.status_lines)
            .into_iter()
            .skip_while(|candidate| *candidate != stack_id)
            .nth(1);
        let select_after_reload = next_stack_id
            .and_then(|next_stack_id| {
                self.status_lines
                    .iter()
                    .filter_map(|line| match line.data.cli_id().map(|id| &**id) {
                        Some(CliId::Branch(branch)) if branch.stack_id == Some(next_stack_id) => {
                            Some(branch.name.to_owned())
                        }
                        _ => None,
                    })
                    .next_back()
            })
            .map(SelectAfterReload::Branch)
            .unwrap_or(SelectAfterReload::Uncommitted);

        {
            let mut guard = ctx.exclusive_worktree_access();
            let head_info = but_api::legacy::workspace::head_info(ctx)?;
            unapply::run(
                ctx,
                guard.write_permission(),
                &head_info,
                UnapplyOperation { stack_id },
            )?;
        }

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(Some(select_after_reload), ReloadCause::Mutation),
            Message::ShowToast {
                kind: ToastKind::Info,
                text: Text::from(Line::from_iter([
                    Span::raw("Unapplied "),
                    Span::styled(format!("'{name}'"), self.theme.local_branch),
                ])),
            },
        ]);

        Ok(())
    }

    fn handle_stack_move_start(&mut self) {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        let Some(CliId::Branch(branch)) = selection.data.cli_id().map(|id| &**id) else {
            return;
        };
        if branch.stack_id.is_none() {
            return;
        }
        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                let source = ReorderStackSource {
                    branch: branch.clone(),
                };
                *mode = Mode::MoveStack(MoveStackMode { source });
            });
    }

    fn handle_stack_move_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::MoveStack(MoveStackMode { source }) = &*self.mode else {
            return Ok(());
        };

        let selection_index = self.cursor.index();
        let Some(selection) = self.status_lines.get(selection_index) else {
            return Ok(());
        };

        if selection
            .data
            .cli_id()
            .is_some_and(|target| source.matches(target))
        {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        if !matches!(selection.data, StatusOutputLineData::BetweenStacks) {
            return Ok(());
        }

        let Some(source_stack_id) = source.branch.stack_id else {
            return Ok(());
        };
        let current_stack_order = stack_ids_in_display_order(&self.status_lines);
        let Some(source_index) = current_stack_order
            .iter()
            .position(|stack| *stack == source_stack_id)
        else {
            return Ok(());
        };

        let target_index = stack_ids_in_display_order(&self.status_lines[..selection_index]).len();
        let mut new_stack_order = current_stack_order.clone();
        let source_stack = new_stack_order.remove(source_index);
        let insert_index = if target_index > source_index {
            target_index - 1
        } else {
            target_index
        };
        new_stack_order.insert(insert_index.min(new_stack_order.len()), source_stack);

        if new_stack_order == current_stack_order {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        let updates = new_stack_order
            .into_iter()
            .enumerate()
            .map(|(order, stack)| gitbutler_branch::BranchUpdateRequest {
                id: Some(stack),
                order: Some(order),
            })
            .collect();

        but_api::legacy::virtual_branches::update_stack_order(ctx, updates)?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(source.branch.name.clone())),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }

    fn selected_stack_id(&self) -> Option<StackId> {
        let selected_line = self.cursor.selected_line(&self.status_lines)?;
        stack_id_for_line(selected_line, &self.status_lines)
    }

    fn selected_line_uses_top_stack_for_stack_mode(&self) -> bool {
        self.cursor
            .selected_line(&self.status_lines)
            .is_some_and(line_uses_top_stack_for_stack_mode)
    }

    pub fn restore_cursor_before_move_stack(&mut self, messages: &mut Vec<Message>) -> bool {
        if matches!(&*self.mode, Mode::MoveStack(_)) {
            self.mode.update(&mut self.backstack, |backstack, mode| {
                let _ = backstack;
                *mode = Mode::Normal(NormalMode::default());
            });
            if let Some(line) = self.cursor.selected_line(&self.status_lines)
                && !is_selectable_in_mode(line, self.mode.as_ref(), self.flags.show_files)
            {
                messages.push(Message::MoveCursorDown(1));
            }
            return true;
        }

        false
    }
}
