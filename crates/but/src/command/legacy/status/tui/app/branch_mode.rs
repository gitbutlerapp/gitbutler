use but_ctx::Context;
use gix::refs::Category;
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
                render::{ModeRender, RenderSingleLineSpans, SpanExt as _},
            },
        },
        switch::{self, SwitchBranchItem, SwitchOperation, switch_branch_items},
    },
    theme::Theme,
    utils::targeting::Side,
};

use super::MoveCursorDiration;

#[derive(Debug, Clone)]
pub struct BranchMode {
    pub marks: Marks,
    pub side: Side,
}

impl Default for BranchMode {
    fn default() -> Self {
        Self {
            marks: Default::default(),
            side: Side::Above,
        }
    }
}

impl ModeRender for BranchMode {}

impl BranchMode {
    pub fn render_insert_branch_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        is_selected: bool,
        status_line_idx: usize,
        lines_part_of_current_branch: Option<&[bool]>,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        let Some(lines_part_of_current_branch) = lines_part_of_current_branch else {
            return;
        };

        match data {
            StatusOutputLineData::UncommittedChanges { .. } | StatusOutputLineData::MergeBase => {
                if is_selected {
                    line.extend([
                        Span::raw("<< branch >>").mode_colors(&*app.mode, app.theme),
                        Span::raw(" "),
                    ]);
                }
            }
            _ => match self.side {
                Side::Above => {
                    if is_selected {
                        line.extend([
                            Span::raw("<< branch >>").mode_colors(&*app.mode, app.theme),
                            Span::raw(" "),
                        ]);
                    }
                }
                Side::Below => {
                    if line_part_of_branch(status_line_idx, lines_part_of_current_branch)
                        && !line_part_of_branch(status_line_idx + 1, lines_part_of_current_branch)
                    {
                        line.extend([
                            Span::raw("<< branch below >>").mode_colors(&*app.mode, app.theme),
                            Span::raw(" "),
                        ]);
                    }
                }
            },
        }
    }
}

fn line_part_of_branch(
    line_idx: impl Into<Option<usize>>,
    lines_part_of_current_branch: &[bool],
) -> bool {
    let Some(line_idx) = line_idx.into() else {
        return false;
    };
    lines_part_of_current_branch
        .get(line_idx)
        .copied()
        .unwrap_or(false)
}

#[derive(Debug)]
pub enum BranchMessage {
    Start,
    Switch,
    New { switch: bool },
    PickAndSwitch,
    ToggleInsertSide,
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
            BranchMessage::PickAndSwitch => self.handle_branch_pick_and_switch(ctx)?,
            BranchMessage::ToggleInsertSide => self.handle_branch_toggle_insert_side(),
        }

        Ok(())
    }

    fn handle_branch_start(&mut self, _messages: &mut Vec<Message>) {
        let marks = match self.mode.marks_ref() {
            marks @ (MarksRef::Empty | MarksRef::Branches { .. }) => marks,
            MarksRef::Hunks { .. } | MarksRef::Commits { .. } | MarksRef::CommittedFiles { .. } => {
                return;
            }
        };

        let new_mode = Mode::Branch(BranchMode {
            marks: marks.to_owned(),
            side: Side::Above,
        });

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = new_mode;
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
        let _outcome = switch::run(
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

    fn handle_branch_toggle_insert_side(&mut self) {
        let Mode::Branch(branch_mode) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        else {
            return;
        };
        branch_mode.side = branch_mode.side.toggle();
    }

    fn handle_branch_pick_and_switch(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let selected_branch = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
            .and_then(|id| {
                if let CliId::Branch(branch) = &**id {
                    Some(branch.name.as_str())
                } else {
                    None
                }
            });

        let items = {
            let guard = ctx.shared_worktree_access();
            switch_branch_items(ctx, guard.read_permission(), selected_branch, true)?
        };
        let Some(items) = items else {
            return Ok(());
        };

        let picker = FuzzyPicker::new(items, self.theme, |item, ctx, messages| {
            let what_to_select = match item {
                SwitchBranchItem::Branch { name, .. } => {
                    let branch = Category::LocalBranch.to_full_name(&*name)?;

                    let mut guard = ctx.exclusive_worktree_access();
                    _ = switch::run(
                        ctx,
                        guard.write_permission(),
                        SwitchOperation::Branch { branch },
                    )?;

                    SelectAfterReload::Branch(name)
                }
                SwitchBranchItem::Workspace => {
                    let mut guard = ctx.exclusive_worktree_access();
                    _ = switch::run(ctx, guard.write_permission(), SwitchOperation::Workspace)?;

                    SelectAfterReload::Uncommitted
                }
            };

            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(Some(what_to_select), ReloadCause::Mutation),
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

        let side = if let Mode::Branch(branch_mode) = &*self.mode {
            branch_mode.side
        } else {
            Side::Above
        };

        let new_name = match &selection.data {
            StatusOutputLineData::Branch { cli_id, .. } => {
                let CliId::Branch(branch) = &**cli_id else {
                    return Ok(());
                };

                let mut guard = ctx.exclusive_worktree_access();

                let outcome = branch::new::run(
                    ctx,
                    guard.write_permission(),
                    NewOperation::NewStackedBranch(NewStackedBranchOperation {
                        name: None,
                        target: NewStackedBranchTarget::Branch(
                            Category::LocalBranch.to_full_name(&*branch.name)?,
                        ),
                        side,
                        switch,
                    }),
                )?;

                outcome.name.shorten().to_string()
            }
            StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::WorktreeUncommitted { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UncommittedFile { .. } => {
                let mut guard = ctx.exclusive_worktree_access();

                let outcome = branch::new::run(
                    ctx,
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

impl FuzzyPickerItem for SwitchBranchItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        match self {
            SwitchBranchItem::Branch {
                name,
                updated_at_display,
                updated_at: _,
            } => [
                Col {
                    text: name.as_str().into(),
                    searchable: Some(searchable),
                },
                Col {
                    text: updated_at_display.as_str().into(),
                    searchable: None,
                },
            ],
            SwitchBranchItem::Workspace => [
                Col {
                    text: "workspace".into(),
                    searchable: Some(searchable),
                },
                Col {
                    text: "".into(),
                    searchable: None,
                },
            ],
        }
    }

    fn style(&self, theme: &Theme) -> Style {
        match self {
            SwitchBranchItem::Branch { .. } => theme.local_branch,
            SwitchBranchItem::Workspace => theme.info,
        }
    }

    fn secondary_style(&self, theme: &Theme) -> Style {
        theme.hint
    }
}
