use std::sync::Arc;

use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;

use crate::{
    CliId,
    command::legacy::status::{
        output::StatusOutputLineData,
        tui::{
            App, Message, Mode, ReloadCause, SelectAfterReload,
            marking::{MarkClasses, Markable, Marks},
            operations,
        },
    },
    id::ShortId,
};

#[derive(Debug, Clone)]
pub struct MoveMode {
    pub source: Arc<MoveSource>,
    pub insert_side: InsertSide,
}

/// A subset of [`CliId`] that supports being moved
#[derive(Debug)]
pub enum MoveSource {
    Marks(Marks),
    Commit {
        commit_id: gix::ObjectId,
        id: ShortId,
    },
    Branch {
        name: String,
        id: ShortId,
        stack_id: Option<StackId>,
    },
}

enum MoveTarget<'a> {
    Branch { name: &'a str },
    Commit { commit_id: gix::ObjectId },
    MergeBase,
}

impl MoveSource {
    pub fn contains(&self, other: &CliId) -> bool {
        match self {
            MoveSource::Marks(marks) => {
                Markable::try_from_cli_id(other).is_some_and(|markable| marks.contains(&markable))
            }
            MoveSource::Commit {
                commit_id: commit_id_lhs,
                id: id_lhs,
            } => {
                if let CliId::Commit {
                    commit_id: commit_id_rhs,
                    id: id_rhs,
                } = other
                {
                    commit_id_lhs == commit_id_rhs && id_lhs == id_rhs
                } else {
                    false
                }
            }
            MoveSource::Branch {
                name: name_lhs,
                id: id_lhs,
                stack_id: stack_id_lhs,
            } => {
                if let CliId::Branch {
                    name: name_rhs,
                    id: id_rhs,
                    stack_id: stack_id_rhs,
                } = other
                {
                    name_lhs == name_rhs && id_lhs == id_rhs && stack_id_lhs == stack_id_rhs
                } else {
                    false
                }
            }
        }
    }
}

impl TryFrom<CliId> for MoveSource {
    type Error = anyhow::Error;

    fn try_from(id: CliId) -> Result<Self, Self::Error> {
        match id {
            CliId::Branch { name, id, stack_id } => Ok(Self::Branch { name, id, stack_id }),
            CliId::Commit { commit_id, id } => Ok(Self::Commit { commit_id, id }),
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

#[derive(Debug, Clone)]
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
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };

        let move_mode = if let Some(marks) = self.marks()
            && !marks.is_empty()
        {
            let MarkClasses {
                marked_commits,
                marked_uncommitted,
            } = marks.classify();
            if !marked_commits || marked_uncommitted {
                return;
            }
            MoveMode {
                source: Arc::new(MoveSource::Marks(marks.clone())),
                insert_side: InsertSide::Above,
            }
        } else {
            match &selection.data {
                StatusOutputLineData::Branch { cli_id }
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
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
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
            StatusOutputLineData::Branch { cli_id } => {
                if let CliId::Branch { name, .. } = &**cli_id {
                    MoveTarget::Branch { name }
                } else {
                    return Ok(());
                }
            }
            StatusOutputLineData::Commit { cli_id, .. } => {
                if let CliId::Commit { commit_id, .. } = &**cli_id {
                    MoveTarget::Commit {
                        commit_id: *commit_id,
                    }
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

        let selection_after_reload = match &**source {
            MoveSource::Commit {
                commit_id: source_commit_id,
                ..
            } => {
                let commit_move_result = match target {
                    MoveTarget::Branch { name } => operations::move_commit_to_branch(
                        ctx,
                        Vec::from([*source_commit_id]),
                        name,
                    )?,
                    MoveTarget::Commit {
                        commit_id: target_commit_id,
                    } => operations::move_commit_to_commit(
                        ctx,
                        Vec::from([*source_commit_id]),
                        target_commit_id,
                        *insert_side,
                    )?,
                    MoveTarget::MergeBase => return Ok(()),
                };

                commit_move_result
                    .workspace
                    .replaced_commits
                    .get(source_commit_id)
                    .copied()
                    .map(SelectAfterReload::Commit)
            }
            MoveSource::Marks(marks) => {
                let Some(sources) = marks
                    .iter()
                    .map(|mark| match mark {
                        Markable::Commit { commit_id, .. } => Some(*commit_id),
                        Markable::Uncommitted(..) => None,
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return Ok(());
                };

                let commit_move_result = match target {
                    MoveTarget::Branch { name } => {
                        operations::move_commit_to_branch(ctx, sources.clone(), name)?
                    }
                    MoveTarget::Commit {
                        commit_id: target_commit_id,
                    } => operations::move_commit_to_commit(
                        ctx,
                        sources.clone(),
                        target_commit_id,
                        *insert_side,
                    )?,
                    MoveTarget::MergeBase => return Ok(()),
                };

                sources
                    .iter()
                    .find_map(|source| {
                        commit_move_result
                            .workspace
                            .replaced_commits
                            .get(source)
                            .copied()
                    })
                    .map(SelectAfterReload::Commit)
            }
            MoveSource::Branch {
                name: source_branch_name,
                ..
            } => match target {
                MoveTarget::Branch {
                    name: target_branch_name,
                } => {
                    operations::move_branch_onto_branch(
                        ctx,
                        source_branch_name,
                        target_branch_name,
                    )?;
                    Some(SelectAfterReload::Branch(source_branch_name.to_owned()))
                }
                MoveTarget::MergeBase => {
                    operations::tear_off_branch(ctx, source_branch_name)?;
                    Some(SelectAfterReload::Branch(source_branch_name.to_owned()))
                }
                MoveTarget::Commit { .. } => return Ok(()),
            },
        };

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(selection_after_reload, ReloadCause::Mutation),
        ]);

        Ok(())
    }
}
