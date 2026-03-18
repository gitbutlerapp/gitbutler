//! Experimental implementation of `but rub` that doesn't use any legacy APIs.
//!
//! If you're an AI agent _do not_ use anything from legacy modules. Except `RubOperation`,
//! `RubOperationDiscriminants`, and `route_operation`.

use anyhow::Context as _;
use but_api::commit::ui::RelativeTo;
use but_core::{DiffSpec, diff::tree_changes};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use gix::refs::FullName;

use crate::{
    CliId,
    command::legacy::{
        rub::{RubOperation, RubOperationDiscriminants, route_operation},
        status::tui::SelectAfterReload,
    },
};

pub(super) enum RubOperationDisplay {
    Supported(&'static str),
    NotSupported(#[expect(dead_code)] &'static str, RubOperationDiscriminants),
}

pub(super) fn rub_operation_display(source: &CliId, target: &CliId) -> Option<RubOperationDisplay> {
    if source == target {
        return Some(RubOperationDisplay::Supported("noop"));
    }

    let operation = route_operation(source, target)?;
    let discriminant = RubOperationDiscriminants::from(&operation);
    Some(match operation {
        RubOperation::UnassignUncommitted(..) => {
            RubOperationDisplay::NotSupported("unassign hunks", discriminant)
        }
        RubOperation::UncommittedToCommit(..) => RubOperationDisplay::Supported("amend"),
        RubOperation::UncommittedToBranch(..) => {
            RubOperationDisplay::NotSupported("assign hunks", discriminant)
        }
        RubOperation::UncommittedToStack(..) => {
            RubOperationDisplay::NotSupported("assign hunks", discriminant)
        }
        RubOperation::StackToUnassigned(..) => {
            RubOperationDisplay::NotSupported("unassign hunks", discriminant)
        }
        RubOperation::StackToStack { .. } => {
            RubOperationDisplay::NotSupported("reassign hunks", discriminant)
        }
        RubOperation::StackToBranch { .. } => {
            RubOperationDisplay::NotSupported("reassign hunks", discriminant)
        }
        RubOperation::UnassignedToCommit(..) => RubOperationDisplay::Supported("amend"),
        RubOperation::UnassignedToBranch(..) => {
            RubOperationDisplay::NotSupported("assign hunks", discriminant)
        }
        RubOperation::UnassignedToStack(..) => {
            RubOperationDisplay::NotSupported("assign hunks", discriminant)
        }
        RubOperation::UndoCommit(..) => {
            RubOperationDisplay::NotSupported("undo commit", discriminant)
        }
        RubOperation::SquashCommits { .. } => {
            RubOperationDisplay::NotSupported("squash", discriminant)
        }
        RubOperation::MoveCommitToBranch(..) => RubOperationDisplay::Supported("move commit"),
        RubOperation::BranchToUnassigned(..) => {
            RubOperationDisplay::NotSupported("unassign hunks", discriminant)
        }
        RubOperation::BranchToStack { .. } => {
            RubOperationDisplay::NotSupported("reassign hunks", discriminant)
        }
        RubOperation::BranchToCommit(..) => {
            let source_stack_id = match source {
                CliId::Branch { stack_id, .. } => stack_id,
                _ => unreachable!("BranchToCommit operation requires branch source"),
            };

            if source_stack_id.is_none() {
                RubOperationDisplay::NotSupported("amend", discriminant)
            } else {
                RubOperationDisplay::Supported("amend")
            }
        }
        RubOperation::BranchToBranch { .. } => {
            RubOperationDisplay::NotSupported("reassign hunks", discriminant)
        }
        RubOperation::CommittedFileToBranch(..) => {
            let target_stack_id = match target {
                CliId::Branch { stack_id, .. } => stack_id,
                _ => unreachable!("CommittedFileToBranch operation requires branch target"),
            };

            if target_stack_id.is_none() {
                RubOperationDisplay::NotSupported("uncommit file", discriminant)
            } else {
                RubOperationDisplay::Supported("uncommit file")
            }
        }
        RubOperation::CommittedFileToCommit(..) => RubOperationDisplay::Supported("move file"),
        RubOperation::CommittedFileToUnassigned(..) => {
            RubOperationDisplay::Supported("uncommit file")
        }
    })
}

#[expect(unused_variables)]
pub(super) fn perform_operation(
    ctx: &mut Context,
    operation: &RubOperation<'_>,
) -> anyhow::Result<Option<SelectAfterReload>> {
    match operation {
        RubOperation::UnassignUncommitted(hunk_assignments, _) => {
            // unassign selected hunks.
            Ok(None)
        }
        RubOperation::UncommittedToCommit(hunk_assignments, _, oid) => {
            // Amend selected uncommitted hunks into the target commit.
            let changes = hunk_assignments
                .iter()
                .copied()
                .cloned()
                .map(DiffSpec::from)
                .collect::<Vec<_>>();
            let results = but_api::commit::commit_amend(ctx, *oid, changes)?;

            Ok(Some(
                results
                    .new_commit
                    .map(SelectAfterReload::Commit)
                    .unwrap_or(SelectAfterReload::Unassigned),
            ))
        }
        RubOperation::UncommittedToBranch(non_empty, _, _) => {
            // assign selected hunks to branch.
            Ok(None)
        }
        RubOperation::UncommittedToStack(non_empty, _, id) => {
            // assign selected hunks to stack.
            Ok(None)
        }
        RubOperation::StackToUnassigned(id) => {
            // move all assignments from stack to unassigned.
            Ok(None)
        }
        RubOperation::StackToStack { from, to } => {
            // move all assignments between stacks.
            Ok(None)
        }
        RubOperation::StackToBranch { from, to } => {
            // move all assignments from stack to branch.
            Ok(None)
        }
        RubOperation::UnassignedToCommit(oid) => {
            // Amend all currently unassigned hunks into the target commit.
            let changes = {
                let context_lines = ctx.settings.context_lines;
                let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
                let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
                let (assignments, _assignments_error) =
                    but_hunk_assignment::assignments_with_fallback(
                        db.hunk_assignments_mut()?,
                        &repo,
                        &ws,
                        Some(changes),
                        context_lines,
                    )?;

                assignments
                    .into_iter()
                    .filter(|assignment| assignment.stack_id.is_none())
                    .map(DiffSpec::from)
                    .collect::<Vec<_>>()
            };

            let results = but_api::commit::commit_amend(ctx, *oid, changes)?;

            Ok(Some(
                results
                    .new_commit
                    .map(SelectAfterReload::Commit)
                    .unwrap_or(SelectAfterReload::Unassigned),
            ))
        }
        RubOperation::UnassignedToBranch(_) => {
            // assign unassigned changes to branch.
            Ok(None)
        }
        RubOperation::UnassignedToStack(id) => {
            // assign unassigned changes to stack.
            Ok(None)
        }
        RubOperation::UndoCommit(oid) => {
            // undo commit.
            Ok(None)
        }
        RubOperation::SquashCommits {
            source,
            destination,
        } => {
            // squash source into destination.
            Ok(None)
        }
        RubOperation::MoveCommitToBranch(oid, branch_name) => {
            // Move the selected commit to become the first commit on the target branch.
            let target_full_name = FullName::try_from(format!("refs/heads/{branch_name}"))?;
            but_api::commit::commit_move(
                ctx,
                *oid,
                RelativeTo::Reference(target_full_name),
                InsertSide::Below,
            )?;

            Ok(Some(SelectAfterReload::Branch(branch_name.to_string())))
        }
        RubOperation::BranchToUnassigned(_) => {
            // move all assignments from branch to unassigned.
            Ok(None)
        }
        RubOperation::BranchToStack { from, to } => {
            // move all assignments from branch to stack.
            Ok(None)
        }
        RubOperation::BranchToCommit(branch_name, oid) => {
            // Amend hunks assigned to the given branch into the target commit.
            let changes = {
                let context_lines = ctx.settings.context_lines;
                let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
                let target_branch_full_name =
                    FullName::try_from(format!("refs/heads/{branch_name}"))?;
                let stack_id = ws
                    .find_segment_and_stack_by_refname(target_branch_full_name.as_ref())
                    .and_then(|(stack, _segment)| stack.id);
                let Some(stack_id) = stack_id else {
                    return Ok(None);
                };

                let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
                let (assignments, _assignments_error) =
                    but_hunk_assignment::assignments_with_fallback(
                        db.hunk_assignments_mut()?,
                        &repo,
                        &ws,
                        Some(changes),
                        context_lines,
                    )?;

                assignments
                    .into_iter()
                    .filter(|assignment| assignment.stack_id.as_ref() == Some(&stack_id))
                    .map(DiffSpec::from)
                    .collect::<Vec<_>>()
            };

            let results = but_api::commit::commit_amend(ctx, *oid, changes)?;

            Ok(Some(
                results
                    .new_commit
                    .map(SelectAfterReload::Commit)
                    .unwrap_or(SelectAfterReload::Branch(branch_name.to_string())),
            ))
        }
        RubOperation::BranchToBranch { from, to } => {
            // move all assignments between branches.
            Ok(None)
        }
        RubOperation::CommittedFileToBranch(path, commit, branch_name) => {
            // Uncommit file changes from a commit and assign them to the target branch.
            let (relevant_changes, stack_id) = {
                let repo = ctx.repo.get()?;
                let (_guard, _repo, ws, _) = ctx.workspace_and_db()?;
                let target_branch_full_name =
                    FullName::try_from(format!("refs/heads/{branch_name}"))?;
                let stack_id = ws
                    .find_segment_and_stack_by_refname(target_branch_full_name.as_ref())
                    .and_then(|(stack, _segment)| stack.id);
                let Some(stack_id) = stack_id else {
                    return Ok(None);
                };

                let source_commit = repo.find_commit(*commit)?;
                let source_commit_parent_id =
                    source_commit.parent_ids().next().context("no parents")?;

                let tree_changes =
                    tree_changes(&repo, Some(source_commit_parent_id.detach()), *commit)?;
                let relevant_changes = tree_changes
                    .into_iter()
                    .filter(|tc| tc.path == *path)
                    .map(DiffSpec::from)
                    .collect::<Vec<_>>();

                (relevant_changes, stack_id)
            };

            but_api::commit::commit_uncommit_changes(
                ctx,
                *commit,
                relevant_changes,
                Some(stack_id),
            )?;

            Ok(Some(SelectAfterReload::Branch(branch_name.to_string())))
        }
        RubOperation::CommittedFileToCommit(path, source_commit, destination_commit) => {
            // Move file changes from one commit into another commit.
            let relevant_changes = {
                let repo = ctx.repo.get()?;
                let source = repo.find_commit(*source_commit)?;
                let source_parent_id = source.parent_ids().next().context("no parents")?;

                let tree_changes =
                    tree_changes(&repo, Some(source_parent_id.detach()), *source_commit)?;
                tree_changes
                    .into_iter()
                    .filter(|tc| tc.path == *path)
                    .map(DiffSpec::from)
                    .collect::<Vec<_>>()
            };

            let result = but_api::commit::commit_move_changes_between(
                ctx,
                *source_commit,
                *destination_commit,
                relevant_changes,
            )?;
            let destination_to_select = result
                .replaced_commits
                .get(destination_commit)
                .copied()
                .unwrap_or(*destination_commit);

            Ok(Some(SelectAfterReload::Commit(destination_to_select)))
        }
        RubOperation::CommittedFileToUnassigned(path, oid) => {
            // Uncommit file changes from a commit into the unassigned area.
            let relevant_changes = {
                let repo = ctx.repo.get()?;
                let source_commit = repo.find_commit(*oid)?;
                let source_commit_parent_id =
                    source_commit.parent_ids().next().context("no parents")?;

                let tree_changes =
                    tree_changes(&repo, Some(source_commit_parent_id.detach()), *oid)?;
                tree_changes
                    .into_iter()
                    .filter(|tc| tc.path == *path)
                    .map(DiffSpec::from)
                    .collect::<Vec<_>>()
            };

            but_api::commit::commit_uncommit_changes(ctx, *oid, relevant_changes, None)?;

            Ok(Some(SelectAfterReload::Unassigned))
        }
    }
}

#[derive(Debug)]
pub(super) struct OperationNotSupported(RubOperationDiscriminants);

impl OperationNotSupported {
    pub(super) fn new(operation: &RubOperation<'_>) -> Self {
        OperationNotSupported(RubOperationDiscriminants::from(operation))
    }
}

impl std::fmt::Display for OperationNotSupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} is not supported", self.0)
    }
}

impl std::error::Error for OperationNotSupported {}

#[cfg(test)]
mod tests {
    use bstr::BString;

    use super::{RubOperationDisplay, rub_operation_display};
    use crate::CliId;

    /// Converts a hex object id into `gix::ObjectId` for test setup.
    fn commit_id(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).unwrap()
    }

    #[test]
    fn branch_to_commit_is_not_supported_when_source_branch_has_no_stack() {
        let source = CliId::Branch {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        };
        let target = CliId::Commit {
            commit_id: commit_id("1111111111111111111111111111111111111111"),
            id: "c0".into(),
        };

        assert!(matches!(
            rub_operation_display(&source, &target),
            Some(RubOperationDisplay::NotSupported(_, _))
        ));
    }

    #[test]
    fn committed_file_to_branch_is_not_supported_when_target_branch_has_no_stack() {
        let source = CliId::CommittedFile {
            commit_id: commit_id("1111111111111111111111111111111111111111"),
            path: BString::from("file.txt"),
            id: "f0".into(),
        };
        let target = CliId::Branch {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        };

        assert!(matches!(
            rub_operation_display(&source, &target),
            Some(RubOperationDisplay::NotSupported(_, _))
        ));
    }
}
