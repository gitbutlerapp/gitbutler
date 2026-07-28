use std::sync::Arc;

use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use gix::refs::Category;
use nonempty::NonEmpty;
use ratatui::prelude::Span;

use crate::{
    CliId,
    command::legacy::{
        r#move::{
            self, MoveCommitsRelativeToOperation, MoveOperation,
            MoveOutcome as MoveOperationOutcome, StackBranchOnOperation, UnstackBranchOperation,
        },
        status::{
            output::StatusOutputLineData,
            tui::{
                App, Message, Mode, ReloadCause, SelectAfterReload,
                render::{
                    ModeRender, RenderSingleLineSpans, render_move_operation_target_marker,
                    source_span,
                },
            },
        },
    },
    id::{BranchId, CommitId},
    utils::targeting,
};

use super::mark::MarksRef;

#[derive(Debug, Clone)]
pub struct MoveMode {
    pub source: Arc<MoveSource>,
    pub insert_side: InsertSide,
}

/// A subset of [`CliId`] that supports being moved
#[derive(Debug)]
pub enum MoveSource {
    Marks(NonEmpty<CommitId>),
    Commit(CommitId),
    Branch(BranchId),
}

enum MoveTarget<'a> {
    Branch { name: &'a str },
    Commit(CommitId),
    MergeBase,
}

impl ModeRender for MoveMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if data
            .cli_id()
            .is_some_and(|target| self.source.contains(target))
            || matches!(data, StatusOutputLineData::MergeBase)
        {
            render_move_operation_target_marker(app, data, self, line);
        }
    }

    fn render_operation_source_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if let Some(cli_id) = data.cli_id()
            && self.source.contains(cli_id)
        {
            line.extend([source_span(app.theme), Span::raw(" ")]);
        }
    }
}

impl MoveSource {
    pub fn contains(&self, other: &CliId) -> bool {
        match self {
            MoveSource::Marks(commits) => {
                if let CliId::Commit { commit: rhs, id: _ } = other {
                    commits.iter().any(|commit| commit == rhs)
                } else {
                    false
                }
            }
            MoveSource::Commit(lhs) => {
                matches!(other, CliId::Commit{ commit: rhs, .. } if lhs == rhs)
            }
            MoveSource::Branch(lhs) => {
                matches!(other, CliId::Branch(rhs) if lhs == rhs)
            }
        }
    }
}

impl TryFrom<CliId> for MoveSource {
    type Error = anyhow::Error;

    fn try_from(id: CliId) -> Result<Self, Self::Error> {
        match id {
            CliId::Branch(branch) => Ok(Self::Branch(branch)),
            CliId::Commit { commit, .. } => Ok(Self::Commit(commit)),
            CliId::UncommittedHunkOrFile(uncommitted_cli_id) => {
                anyhow::bail!("cannot move: {:?}", uncommitted_cli_id.id)
            }
            CliId::PathPrefix { id, .. }
            | CliId::CommittedFile { id, .. }
            | CliId::Uncommitted { id }
            | CliId::Stack { id, .. } => {
                anyhow::bail!("cannot move: {id:?}")
            }
        }
    }
}

#[derive(Debug)]
pub enum MoveMessage {
    Start,
    ToggleInsertSide,
    Confirm,
}

impl App {
    pub fn handle_move(
        &mut self,
        move_message: MoveMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match move_message {
            MoveMessage::Start => self.handle_move_start(),
            MoveMessage::ToggleInsertSide => self.handle_move_toggle_insert_side(),
            MoveMessage::Confirm => self.handle_move_confirm(ctx, messages)?,
        }

        Ok(())
    }

    fn handle_move_start(&mut self) {
        match self.mode.marks_ref() {
            MarksRef::Branches { .. } => return,
            MarksRef::Empty
            | MarksRef::Hunks { .. }
            | MarksRef::Commits { .. }
            | MarksRef::CommittedFiles { .. } => {}
        }

        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };

        let move_mode = if let Mode::Normal(normal_mode) = &*self.mode
            && let Some(commits) = normal_mode.marks.as_commits().cloned()
        {
            MoveMode {
                source: Arc::new(MoveSource::Marks(commits)),
                insert_side: InsertSide::Above,
            }
        } else {
            match &selection.data {
                StatusOutputLineData::Branch { cli_id, .. }
                | StatusOutputLineData::Commit { cli_id, .. } => {
                    let Ok(source) = MoveSource::try_from(Arc::unwrap_or_clone(Arc::clone(cli_id)))
                    else {
                        return;
                    };
                    MoveMode {
                        source: Arc::new(source),
                        insert_side: InsertSide::Above,
                    }
                }
                StatusOutputLineData::UpdateNotice
                | StatusOutputLineData::Connector
                | StatusOutputLineData::BetweenStacks
                | StatusOutputLineData::StagedChanges { .. }
                | StatusOutputLineData::StagedFile { .. }
                | StatusOutputLineData::UncommittedChanges { .. }
                | StatusOutputLineData::UncommittedFile { .. }
                | StatusOutputLineData::CommitMessage
                | StatusOutputLineData::EmptyCommitMessage
                | StatusOutputLineData::File { .. }
                | StatusOutputLineData::MergeBase
                | StatusOutputLineData::UpstreamChanges
                | StatusOutputLineData::Warning
                | StatusOutputLineData::Hint
                | StatusOutputLineData::NoAssignmentsUnstaged => return,
            }
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Move(move_mode);
            });
    }

    fn handle_move_toggle_insert_side(&mut self) {
        let Mode::Move(move_mode) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        else {
            return;
        };
        move_mode.insert_side = match move_mode.insert_side {
            InsertSide::Above => InsertSide::Below,
            InsertSide::Below => InsertSide::Above,
        };
    }

    fn handle_move_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Move(MoveMode {
            source,
            insert_side,
        }) = &*self.mode
        else {
            return Ok(());
        };

        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        if selection
            .data
            .cli_id()
            .is_some_and(|target| source.contains(target))
        {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        let target = match &selection.data {
            StatusOutputLineData::Branch { cli_id, .. } => {
                if let CliId::Branch(branch) = &**cli_id {
                    MoveTarget::Branch { name: &branch.name }
                } else {
                    return Ok(());
                }
            }
            StatusOutputLineData::Commit { cli_id, .. } => {
                if let CliId::Commit { commit, id: _ } = &**cli_id {
                    MoveTarget::Commit(commit.clone())
                } else {
                    return Ok(());
                }
            }
            StatusOutputLineData::MergeBase => MoveTarget::MergeBase,
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => {
                return Ok(());
            }
        };

        let move_op = match &**source {
            MoveSource::Commit(commit) => {
                MoveOperation::CommitsRelativeTo(MoveCommitsRelativeToOperation {
                    sources: NonEmpty::new(commit.clone()),
                    target: move_target(target, *insert_side)?,
                })
            }
            MoveSource::Marks(commits) => {
                MoveOperation::CommitsRelativeTo(MoveCommitsRelativeToOperation {
                    sources: commits.clone(),
                    target: move_target(target, *insert_side)?,
                })
            }
            MoveSource::Branch(source) => {
                let source_branch = Category::LocalBranch.to_full_name(source.name.as_str())?;
                match target {
                    MoveTarget::Branch {
                        name: target_branch_name,
                    } => MoveOperation::StackBranch(StackBranchOnOperation {
                        source_branch,
                        target_branch: Category::LocalBranch.to_full_name(target_branch_name)?,
                    }),
                    MoveTarget::MergeBase => {
                        MoveOperation::UnstackBranch(UnstackBranchOperation { source_branch })
                    }
                    MoveTarget::Commit { .. } => return Ok(()),
                }
            }
        };

        let selection_after_reload = move_with(ctx, move_op)?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(selection_after_reload, ReloadCause::Mutation),
        ]);

        Ok(())
    }
}

fn move_target(
    target: MoveTarget<'_>,
    insert_side: InsertSide,
) -> anyhow::Result<r#move::MoveTarget> {
    Ok(match target {
        MoveTarget::Branch { name } => r#move::MoveTarget::BranchTip {
            name: Category::LocalBranch.to_full_name(name)?,
        },
        MoveTarget::Commit(commit) => r#move::MoveTarget::Commit {
            commit,
            side: targeting::Side::from(insert_side),
        },
        MoveTarget::MergeBase => anyhow::bail!("commits cannot be moved to the merge base"),
    })
}

fn move_with(
    ctx: &mut Context,
    move_op: MoveOperation,
) -> anyhow::Result<Option<SelectAfterReload>> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let outcome = r#move::run(ctx, &mut meta, guard.write_permission(), move_op)?;

    Ok(match outcome {
        MoveOperationOutcome::Commits { moved_commits, .. } => {
            Some(SelectAfterReload::Commit(moved_commits.head.commit_id))
        }
        MoveOperationOutcome::Changes { new_commit, .. } => {
            Some(SelectAfterReload::Commit(new_commit.commit_id))
        }
        MoveOperationOutcome::StackBranch { source_branch, .. }
        | MoveOperationOutcome::UnstackBranch { source_branch } => Some(SelectAfterReload::Branch(
            source_branch.shorten().to_string(),
        )),
    })
}
