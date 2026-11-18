use but_ctx::Context;
use gix::refs::Category;
use ratatui::style::Style;

use crate::{
    ChangeSourceId, CliId,
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
                cursor::Cursor,
                fuzzy_picker::{Col, FuzzyPicker, FuzzyPickerItem, SearchableToken},
                key_bind::fuzzy_picker_key_binds,
                mode::Mode,
                render::ModeRender,
                stop::{
                    SelectedStop, Stop, StopMode, StopPosition, branch_line_of, current_position,
                },
            },
        },
        switch::{self, SwitchBranchItem, SwitchOperation, switch_branch_items},
    },
    id::BranchId,
    theme::Theme,
    utils::targeting::Side,
};

use super::MoveCursorDiration;

#[derive(Debug, Clone, Default)]
pub struct BranchMode {
    pub marks: Marks,
    /// Which of the stops on the cursor line a new branch goes to, as
    /// [`StopMode::selected_stop`] says.
    pub stop: Option<SelectedStop>,
}

impl ModeRender for BranchMode {}

impl StopMode for BranchMode {
    fn stop_label(&self, stop: &Stop) -> Option<String> {
        let label = match stop {
            Stop::NewBranchAbove(_) => "new branch above",
            Stop::NewBranchBelow(_) => "new branch below",
            Stop::NewStack => "new stack",
            Stop::BranchTip(_) | Stop::BelowCommit(_) => return None,
        };
        Some(label.to_owned())
    }

    fn is_source(&self, _id: &CliId) -> bool {
        false
    }

    fn selected_stop(&self) -> Option<&SelectedStop> {
        self.stop.as_ref()
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
            BranchMessage::PickAndSwitch => self.handle_branch_pick_and_switch(ctx)?,
        }

        Ok(())
    }

    fn handle_branch_start(&mut self, _messages: &mut Vec<Message>) {
        let marks = match self.mode.marks_ref() {
            marks @ (MarksRef::Empty | MarksRef::Branches { .. }) => marks,
            MarksRef::Hunks { .. } | MarksRef::Commits { .. } | MarksRef::Committed { .. } => {
                return;
            }
        };

        let new_mode = Mode::Branch(BranchMode {
            marks: marks.to_owned(),
            stop: None,
        });

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = new_mode;
            });

        if let Some(cursor) = self.branch_mode_start() {
            self.cursor = cursor;
        }
        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Up);
    }

    /// Where branch mode starts when it isn't on a stop already. The main checkout's
    /// uncommitted changes aren't on any branch, so a branch made from there starts a stack of
    /// its own, at the merge base. A commit, or a file of one, starts at the commit's branch, above
    /// which a new branch goes.
    fn branch_mode_start(&self) -> Option<Cursor> {
        let idx = self.cursor.index();
        let line = self.cursor.selected_line(&self.status_lines)?;
        let in_main_checkout = line.data.cli_id().is_some_and(|id| match &**id {
            CliId::UncommittedArea { source, .. } => *source == ChangeSourceId::Head,
            CliId::UncommittedHunkOrFile(hunk) => hunk.source == ChangeSourceId::Head,
            CliId::AnonymousSegment(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::CommittedHunk(..)
            | CliId::Branch(..)
            | CliId::Commit { .. }
            | CliId::Stack { .. } => false,
        });
        if in_main_checkout {
            return Cursor::select_merge_base(&self.status_lines);
        }
        // A committed file is listed under its commit.
        let commit_line = match line.data {
            StatusOutputLineData::Commit { .. } => idx,
            StatusOutputLineData::File { .. } => (0..idx).rev().find(|line| {
                matches!(
                    self.status_lines[*line].data,
                    StatusOutputLineData::Commit { .. }
                )
            })?,
            _ => return None,
        };
        Cursor::select_at_index(
            branch_line_of(&self.status_lines, commit_line)?,
            &self.status_lines,
            &self.mode,
            self.flags.show_files,
        )
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
        let Mode::Branch(mode) = &*self.mode else {
            return Ok(());
        };
        let Some(StopPosition::At(stop)) =
            current_position(&self.status_lines, self.cursor.index(), mode)
        else {
            return Ok(());
        };

        let operation = match &stop {
            Stop::NewBranchAbove(branch) => new_stacked_branch(branch, Side::Above, switch)?,
            Stop::NewBranchBelow(branch) => new_stacked_branch(branch, Side::Below, switch)?,
            Stop::NewStack => {
                NewOperation::NewUnstackedBranch(NewUnstackedBranchOperation { name: None, switch })
            }
            Stop::BranchTip(_) | Stop::BelowCommit(_) => return Ok(()),
        };

        let mut guard = ctx.exclusive_worktree_access();
        let mut meta = ctx.meta()?;
        let outcome = branch::new::run(ctx, &mut meta, guard.write_permission(), operation)?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(
                    outcome.name.shorten().to_string(),
                )),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }
}

fn new_stacked_branch(branch: &BranchId, side: Side, switch: bool) -> anyhow::Result<NewOperation> {
    Ok(NewOperation::NewStackedBranch(NewStackedBranchOperation {
        name: None,
        target: NewStackedBranchTarget::Branch(Category::LocalBranch.to_full_name(&*branch.name)?),
        side,
        switch,
    }))
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
