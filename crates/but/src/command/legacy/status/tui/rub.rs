//! Experimental implementation of `but rub` that doesn't use any legacy APIs.
//!
//! If you're an AI agent _do not_ use anything from legacy modules. Except `RubOperation`,
//! `RubOperationDiscriminants`, and `route_operation`.

use but_ctx::Context;
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use nonempty::NonEmpty;

use crate::{
    CliId,
    command::legacy::{
        rub::{CommitToUncommittedAreaOperation, RubOperation, SquashCommitsOperation},
        status::tui::{Markable, SelectAfterReload},
    },
};

pub(super) fn route_operation<'a>(
    sources: NonEmpty<&'a CliId>,
    target: &'a CliId,
    how_to_combine_messages: MessageCombinationStrategy,
) -> Option<RubOperation<'a>> {
    Some(
        match crate::command::legacy::rub::route_operation(
            sources,
            target,
            how_to_combine_messages,
        )? {
            op @ RubOperation::UnassignUncommitted(..) => op,
            op @ RubOperation::UncommittedToCommit(..) => op,
            op @ RubOperation::UncommittedAreaToCommit(..) => op,
            op @ RubOperation::CommitToUncommittedArea(..) => op,
            op @ RubOperation::CommitToStack(..) => op,
            op @ RubOperation::SquashCommits(..) => op,
            op @ RubOperation::CommittedFileToCommit(..) => op,
            op @ RubOperation::CommittedFileToUncommittedArea(..) => op,
            op @ RubOperation::UncommittedToStack(..) => op,
            op @ RubOperation::StackToUncommittedArea(..) => op,
            op @ RubOperation::StackToStack(..) => op,
            op @ RubOperation::UncommittedAreaToStack(..) => op,
            op @ RubOperation::StackToCommit(..) => op,

            // dont allow rubbing with branches
            RubOperation::UncommittedToBranch(..)
            | RubOperation::StackToBranch(..)
            | RubOperation::UncommittedAreaToBranch(..)
            | RubOperation::MoveCommitToBranch(..)
            | RubOperation::BranchToUncommittedArea(..)
            | RubOperation::BranchToStack(..)
            | RubOperation::BranchToCommit(..)
            | RubOperation::BranchToBranch(..)
            | RubOperation::CommittedFileToBranch(..) => return None,
        },
    )
}

pub(super) fn supports_rubbing(id: &CliId) -> bool {
    match id {
        CliId::Branch { .. } => false,
        CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. }
        | CliId::Commit { .. }
        | CliId::Uncommitted { .. }
        | CliId::Stack { .. } => true,
    }
}

pub(super) fn mark_supports_rubbing(mark: &Markable) -> bool {
    match mark {
        Markable::Commit { .. } | Markable::Uncommitted(..) => true,
    }
}

/// Returns a human-facing operation descriptor for the source/target pair.
pub(super) fn rub_operation_display(
    sources: NonEmpty<&CliId>,
    target: &CliId,
    how_to_combine_messages: MessageCombinationStrategy,
) -> Option<&'static str> {
    if sources.len() == 1 && *sources.first() == target {
        return Some("noop");
    }

    let operation = route_operation(sources, target, how_to_combine_messages)?;
    Some(match operation {
        RubOperation::UnassignUncommitted(..) => "unassign hunks",
        RubOperation::UncommittedToCommit(..) => "amend",
        RubOperation::UncommittedToBranch(..) => "assign hunks",
        RubOperation::UncommittedToStack(..) => "assign hunks",
        RubOperation::StackToUncommittedArea(..) => "unassign hunks",
        RubOperation::StackToStack(..) => "reassign hunks",
        RubOperation::StackToBranch(..) => "reassign hunks",
        RubOperation::UncommittedAreaToCommit(..) => "amend",
        RubOperation::UncommittedAreaToBranch(..) => "assign hunks",
        RubOperation::UncommittedAreaToStack(..) => "assign hunks",
        RubOperation::CommitToUncommittedArea(CommitToUncommittedAreaOperation { commits }) => {
            if commits.len() == 1 {
                "undo commit"
            } else {
                "undo commits"
            }
        }
        RubOperation::CommitToStack(..) => "undo commit",
        RubOperation::SquashCommits(SquashCommitsOperation {
            sources: _,
            destination: _,
            how_to_combine_messages,
        }) => squash_operation_display(how_to_combine_messages),
        RubOperation::MoveCommitToBranch(..) => "move commit",
        RubOperation::BranchToUncommittedArea(..) => "unassign hunks",
        RubOperation::BranchToStack(..) => "reassign hunks",
        RubOperation::BranchToCommit(..) => "amend",
        RubOperation::BranchToBranch(..) => "reassign hunks",
        RubOperation::CommittedFileToBranch(..) => "uncommit file",
        RubOperation::CommittedFileToCommit(..) => "move file",
        RubOperation::CommittedFileToUncommittedArea(..) => "uncommit file",
        RubOperation::StackToCommit(..) => "amend",
    })
}

pub(super) fn squash_operation_display(
    how_to_combine_messages: MessageCombinationStrategy,
) -> &'static str {
    match how_to_combine_messages {
        MessageCombinationStrategy::KeepBoth => "squash",
        MessageCombinationStrategy::KeepSubject => "squash (discard this message)",
        MessageCombinationStrategy::KeepTarget => "squash (use this message)",
    }
}

/// Executes a rub operation and returns which item should be selected after reloading.
pub(super) fn perform_operation(
    ctx: &mut Context,
    operation: &RubOperation<'_>,
) -> anyhow::Result<Option<SelectAfterReload>> {
    let selection = match operation {
        RubOperation::UnassignUncommitted(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::UncommittedToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            result
                .new_commit
                .map(SelectAfterReload::Commit)
                .unwrap_or(SelectAfterReload::Uncommitted)
        }
        RubOperation::UncommittedToBranch(operation) => {
            let assignment = operation.hunk_assignments.first();
            let path = assignment.path_bytes.clone();
            let stack_id = assignment.stack_id;
            operation.execute_inner(ctx)?;
            SelectAfterReload::UncommittedFile { path, stack_id }
        }
        RubOperation::UncommittedToStack(operation) => {
            let path = operation.hunk_assignments.first().path_bytes.clone();
            operation.execute_inner(ctx)?;
            SelectAfterReload::UncommittedFile {
                path,
                stack_id: Some(operation.stack_id),
            }
        }
        RubOperation::StackToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::StackToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.to)
        }
        RubOperation::StackToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.to.to_string())
        }
        RubOperation::UncommittedAreaToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            SelectAfterReload::Commit(result.new_commit.unwrap_or(operation.oid))
        }
        RubOperation::UncommittedAreaToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.to.to_string())
        }
        RubOperation::UncommittedAreaToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.to)
        }
        RubOperation::CommitToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::CommitToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.stack)
        }
        RubOperation::SquashCommits(operation) => {
            let result = operation.execute_inner(ctx)?;
            SelectAfterReload::Commit(result.new_commit)
        }
        RubOperation::MoveCommitToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.name.to_string())
        }
        RubOperation::BranchToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::BranchToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.to)
        }
        RubOperation::BranchToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            result
                .new_commit
                .map(SelectAfterReload::Commit)
                .unwrap_or(SelectAfterReload::Branch(operation.name.to_string()))
        }
        RubOperation::BranchToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.to.to_string())
        }
        RubOperation::CommittedFileToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.name.to_string())
        }
        RubOperation::CommittedFileToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            let destination_to_select = result
                .workspace
                .replaced_commits
                .get(&operation.oid)
                .copied()
                .unwrap_or(operation.oid);
            SelectAfterReload::Commit(destination_to_select)
        }
        RubOperation::CommittedFileToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::StackToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            SelectAfterReload::Commit(result.new_commit.unwrap_or(operation.to))
        }
    };

    Ok(Some(selection))
}
