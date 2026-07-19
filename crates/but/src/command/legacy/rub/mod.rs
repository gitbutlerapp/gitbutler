use std::collections::HashMap;
use std::fmt;

use crate::theme::{self, Paint};
use anyhow::bail;
use bstr::BStr;
use but_api::commit::types::{
    CommitCreateResult, CommitMoveResult, CommitSquashResult, MoveChangesResult,
    UncommitChangesSource, UncommitResult,
};
use but_core::{DiffSpec, DryRun, ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use but_hunk_assignment::{HunkAssignment, HunkAssignmentRequest, HunkAssignmentTarget};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
mod amend;
mod assign;
pub(crate) mod squash;
mod undo;
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gix::refs::FullName;
use nonempty::NonEmpty;

use crate::{
    CliError, CliId, IdMap, bad_input,
    command::legacy::rub::assign::stack_id_to_branch_name,
    id::parser::{
        IdResolutionError, parse_sources_with_disambiguation,
        parse_uncommitted_sources_with_disambiguation, prompt_for_disambiguation,
    },
    utils::{OutputChannel, diff_specs::DiffSpecBuilder, shorten_object_id},
};

/// A description of a set of hunks.
type Description = String;

/// Represents moving selected uncommitted hunks to uncommitted.
#[derive(Debug)]
pub(crate) struct UnassignUncommittedOperation<'a> {
    /// The uncommitted hunk assignments to unassign.
    pub(crate) hunk_assignments: NonEmpty<&'a HunkAssignment>,
    /// A human-readable description of the selected hunks.
    pub(crate) description: Description,
}

/// Represents amending selected uncommitted hunks into a commit.
#[derive(Debug)]
pub(crate) struct UncommittedToCommitOperation<'a> {
    /// The uncommitted hunk assignments to amend.
    pub(crate) hunk_assignments: NonEmpty<&'a HunkAssignment>,
    /// A human-readable description of the selected hunks.
    pub(crate) description: Description,
    /// The destination commit id.
    pub(crate) oid: gix::ObjectId,
}

/// Represents assigning selected uncommitted hunks to a branch.
#[derive(Debug)]
pub(crate) struct UncommittedToBranchOperation<'a> {
    /// The uncommitted hunk assignments to assign.
    pub(crate) hunk_assignments: NonEmpty<&'a HunkAssignment>,
    /// A human-readable description of the selected hunks.
    pub(crate) description: Description,
    /// The destination branch name.
    pub(crate) name: &'a str,
}

/// Represents assigning selected uncommitted hunks to a stack.
#[derive(Debug)]
pub(crate) struct UncommittedToStackOperation<'a> {
    /// The uncommitted hunk assignments to assign.
    pub(crate) hunk_assignments: NonEmpty<&'a HunkAssignment>,
    /// A human-readable description of the selected hunks.
    pub(crate) description: Description,
    /// The destination stack id.
    pub(crate) stack_id: StackId,
}

/// Represents moving all assignments from a stack to uncommitted.
#[derive(Debug)]
pub(crate) struct StackToUncommittedAreaOperation {
    /// The source stack id.
    pub(crate) stack_id: StackId,
}

/// Represents moving all assignments from one stack to another.
#[derive(Debug)]
pub(crate) struct StackToStackOperation {
    /// The source stack id.
    pub(crate) from: StackId,
    /// The destination stack id.
    pub(crate) to: StackId,
}

/// Represents moving all assignments from a stack to a branch.
#[derive(Debug)]
pub(crate) struct StackToBranchOperation<'a> {
    /// The source stack id.
    pub(crate) from: StackId,
    /// The destination branch name.
    pub(crate) to: &'a str,
}

/// Represents squashing all assignments from a stack to a commit.
#[derive(Debug)]
pub(crate) struct StackToCommitOperation {
    /// The source stack id.
    pub(crate) from: StackId,
    pub(crate) to: gix::ObjectId,
}

/// Represents amending all uncommitted hunks into a commit.
#[derive(Debug)]
pub(crate) struct UncommittedAreaToCommitOperation {
    /// The destination commit id.
    pub(crate) oid: gix::ObjectId,
}

/// Represents assigning all uncommitted hunks to a branch.
#[derive(Debug)]
pub(crate) struct UncommittedAreaToBranchOperation<'a> {
    /// The destination branch name.
    pub(crate) to: &'a str,
}

/// Represents assigning all uncommitted hunks to a stack.
#[derive(Debug)]
pub(crate) struct UncommittedAreaToStackOperation {
    /// The destination stack id.
    pub(crate) to: StackId,
}

/// Represents undoing a commit.
#[derive(Debug)]
pub(crate) struct CommitToUncommittedAreaOperation {
    /// The commits to undo.
    pub(crate) commits: NonEmpty<gix::ObjectId>,
}

/// Represents undoing a commit to a stack.
#[derive(Debug)]
pub(crate) struct CommitToStackOperation {
    /// The commit id to undo.
    pub(crate) oid: gix::ObjectId,
    /// The stack to assign the changes to.
    pub(crate) stack: StackId,
}

/// Represents squashing one commit into another.
#[derive(Debug)]
pub(crate) struct SquashCommitsOperation {
    /// The source commit ids.
    pub(crate) sources: NonEmpty<gix::ObjectId>,
    /// The destination commit id.
    pub(crate) destination: gix::ObjectId,
    pub(crate) how_to_combine_messages: MessageCombinationStrategy,
}

/// Represents moving a commit to a branch.
#[derive(Debug)]
pub(crate) struct MoveCommitToBranchOperation<'a> {
    /// The commit id to move.
    pub(crate) oid: gix::ObjectId,
    /// The destination branch name.
    pub(crate) name: &'a str,
}

/// Represents moving all assignments from a branch to uncommitted.
#[derive(Debug)]
pub(crate) struct BranchToUncommittedAreaOperation<'a> {
    /// The source branch name.
    pub(crate) from: &'a str,
}

/// Represents moving all assignments from a branch to a stack.
#[derive(Debug)]
pub(crate) struct BranchToStackOperation<'a> {
    /// The source branch name.
    pub(crate) from: &'a str,
    /// The destination stack id.
    pub(crate) to: StackId,
}

/// Represents amending branch-assigned hunks into a commit.
#[derive(Debug)]
pub(crate) struct BranchToCommitOperation<'a> {
    /// The source branch name.
    pub(crate) name: &'a str,
    /// The destination commit id.
    pub(crate) oid: gix::ObjectId,
}

/// Represents moving all assignments from one branch to another.
#[derive(Debug)]
pub(crate) struct BranchToBranchOperation<'a> {
    /// The source branch name.
    pub(crate) from: &'a str,
    /// The destination branch name.
    pub(crate) to: &'a str,
}

/// Represents uncommitting file changes from a commit into a branch.
#[derive(Debug)]
pub(crate) struct CommittedFileToBranchOperation<'a> {
    /// The file path.
    pub(crate) path: &'a BStr,
    /// The source commit id.
    pub(crate) commit_oid: gix::ObjectId,
    /// The destination branch name.
    pub(crate) name: &'a str,
}

/// Represents moving file changes from one commit into another.
#[derive(Debug)]
pub(crate) struct CommittedFileToCommitOperation<'a> {
    /// The file path.
    pub(crate) path: &'a BStr,
    /// The source commit id.
    pub(crate) commit_oid: gix::ObjectId,
    /// The destination commit id.
    pub(crate) oid: gix::ObjectId,
}

/// Represents uncommitting file changes from a commit into uncommitted.
#[derive(Debug)]
pub(crate) struct CommittedFileToUncommittedAreaOperation<'a> {
    /// The file path.
    pub(crate) path: &'a BStr,
    /// The source commit id.
    pub(crate) commit_oid: gix::ObjectId,
}

/// Represents the operation to perform for a given source and target combination.
/// This enum serves as the single source of truth for valid rub operations.
// NOTE: Remember to update crates/but/tests/but/command/undo/undo_rub.rs with an undo test when
// adding new operations
#[derive(Debug, strum::EnumDiscriminants)]
pub(crate) enum RubOperation<'a> {
    UnassignUncommitted(UnassignUncommittedOperation<'a>),
    UncommittedToCommit(UncommittedToCommitOperation<'a>),
    UncommittedToBranch(UncommittedToBranchOperation<'a>),
    UncommittedToStack(UncommittedToStackOperation<'a>),
    StackToUncommittedArea(StackToUncommittedAreaOperation),
    StackToStack(StackToStackOperation),
    StackToBranch(StackToBranchOperation<'a>),
    StackToCommit(StackToCommitOperation),
    UncommittedAreaToCommit(UncommittedAreaToCommitOperation),
    UncommittedAreaToBranch(UncommittedAreaToBranchOperation<'a>),
    UncommittedAreaToStack(UncommittedAreaToStackOperation),
    CommitToUncommittedArea(CommitToUncommittedAreaOperation),
    CommitToStack(CommitToStackOperation),
    SquashCommits(SquashCommitsOperation),
    MoveCommitToBranch(MoveCommitToBranchOperation<'a>),
    BranchToUncommittedArea(BranchToUncommittedAreaOperation<'a>),
    BranchToStack(BranchToStackOperation<'a>),
    BranchToCommit(BranchToCommitOperation<'a>),
    BranchToBranch(BranchToBranchOperation<'a>),
    CommittedFileToBranch(CommittedFileToBranchOperation<'a>),
    CommittedFileToCommit(CommittedFileToCommitOperation<'a>),
    CommittedFileToUncommittedArea(CommittedFileToUncommittedAreaOperation<'a>),
}

impl<'a> UnassignUncommittedOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            writeln!(out, "Unstaged {}", self.description)?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes this operation without writing any output.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let requests =
            assignment_requests_for_selected_hunks(self.hunk_assignments.iter().copied(), None);
        but_api::diff::assign_hunk(ctx, requests)
    }
}

impl<'a> UncommittedToCommitOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        let result = self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let new_commit = theme::new_commit_ref(ctx, result.new_commit)?;
            writeln!(out, "Amended {} → {new_commit}", self.description)?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "ok": true,
                "new_commit_id": result.new_commit.map(|c| c.to_string()),
            }))?;
        }
        Ok(())
    }

    /// Executes this operation without writing any output.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<CommitCreateResult> {
        let changes = {
            let context_lines = ctx.settings.context_lines;
            let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
            builder.push_hunk_assignments(self.hunk_assignments.iter().copied().cloned())?;
            builder.reconcile_worktree_diff_specs()?;
            builder.into_diff_specs()
        };
        but_api::commit::amend::commit_amend(ctx, self.oid, changes, DryRun::No)
    }
}

impl<'a> UncommittedToBranchOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged {} → {}.",
                self.description,
                t.local_branch.paint(format!("[{}]", self.name))
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `UncommittedToBranch` by assigning selected hunks to the target branch stack.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let stack_id = stack_id_for_branch_name(ctx, self.name)?;
        let requests =
            assignment_requests_for_selected_hunks(self.hunk_assignments.iter().copied(), stack_id);
        but_api::diff::assign_hunk(ctx, requests)
    }
}

impl<'a> UncommittedToStackOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged {} → stack {}.",
                self.description,
                t.local_branch.paint(format!("[{}]", self.stack_id))
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `UncommittedToStack` by assigning selected hunks to the target stack.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let requests = assignment_requests_for_selected_hunks(
            self.hunk_assignments.iter().copied(),
            Some(self.stack_id),
        );
        but_api::diff::assign_hunk(ctx, requests)
    }
}

impl StackToUncommittedAreaOperation {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Unstaged all {} changes.",
                stack_id_to_branch_name(ctx, self.stack_id)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack"))
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `StackToUncommittedArea` by reassigning all hunks from the source stack into uncommitted.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        reassign_all_from_stack_to_stack(ctx, Some(self.stack_id), None)
    }
}

impl StackToStackOperation {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged all {} changes to {}.",
                stack_id_to_branch_name(ctx, self.from)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack")),
                stack_id_to_branch_name(ctx, self.to)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack"))
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `StackToStack` by reassigning all hunks from one stack to another.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        reassign_all_from_stack_to_stack(ctx, Some(self.from), Some(self.to))
    }
}

impl<'a> StackToBranchOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged all {} changes to {}.",
                stack_id_to_branch_name(ctx, self.from)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack")),
                t.local_branch.paint(format!("[{}]", self.to)),
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }

        Ok(())
    }

    /// Executes `StackToBranch` by reassigning all hunks from the source stack to the target branch stack.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let target_stack_id = stack_id_for_branch_name(ctx, self.to)?;
        reassign_all_from_stack_to_stack(ctx, Some(self.from), target_stack_id)
    }
}

impl StackToCommitOperation {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        let result = self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            let new_commit = theme::new_commit_ref(ctx, result.new_commit)?;
            writeln!(
                out,
                "Amended files assigned to {} → {}",
                stack_id_to_branch_name(ctx, self.from)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack")),
                new_commit,
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "ok": true,
                "new_commit_id": result.new_commit.map(|c| c.to_string()),
            }))?;
        }
        Ok(())
    }

    /// Executes `StackToCommit` by squashing all hunks from the source stack to the target commit.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<CommitCreateResult> {
        let changes = changes_for_stack_assignment(ctx, Some(self.from))?;
        but_api::commit::amend::commit_amend(ctx, self.to, changes, DryRun::No)
    }
}

impl UncommittedAreaToCommitOperation {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        let result = self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let new_commit = theme::new_commit_ref(ctx, result.new_commit)?;
            writeln!(out, "Amended uncommitted files → {new_commit}")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "ok": true,
                "new_commit_id": result.new_commit.map(|c| c.to_string()),
            }))?;
        }
        Ok(())
    }

    /// Executes `UncommittedAreaToCommit` and returns the exact commit-amend API result.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<CommitCreateResult> {
        let changes = changes_for_stack_assignment(ctx, None)?;
        but_api::commit::amend::commit_amend(ctx, self.oid, changes, DryRun::No)
    }
}

impl<'a> UncommittedAreaToBranchOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged all {} changes to {}.",
                t.important.paint("unstaged"),
                t.local_branch.paint(format!("[{}]", self.to)),
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `UncommittedAreaToBranch` by assigning uncommitted hunks to the target branch stack.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let target_stack_id = stack_id_for_branch_name(ctx, self.to)?;
        reassign_all_from_stack_to_stack(ctx, None, target_stack_id)
    }
}

impl UncommittedAreaToStackOperation {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged all {} changes to {}.",
                t.important.paint("unstaged"),
                stack_id_to_branch_name(ctx, self.to)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack"))
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `UncommittedAreaToStack` by assigning uncommitted hunks to the target stack.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        reassign_all_from_stack_to_stack(ctx, None, Some(self.to))
    }
}

impl CommitToUncommittedAreaOperation {
    /// Executes this operation.
    pub(crate) fn execute(
        self,
        ctx: &mut Context,
        id_map: &IdMap,
        out: &mut OutputChannel,
    ) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let repo = ctx.repo.get()?;
            if self.commits.len() == 1 {
                writeln!(
                    out,
                    "Uncommitted {}",
                    theme::CommitRef(id_map, &repo, *self.commits.first())
                )?;
            } else {
                writeln!(out, "Uncommitted {} commits", self.commits.len())?;
            }
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `UndoCommit` by uncommitting all changes from the selected commit.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<UncommitResult> {
        but_api::commit::uncommit::commit_uncommit(
            ctx,
            self.commits.iter().copied().collect(),
            None,
            DryRun::No,
        )
    }
}

impl CommitToStackOperation {
    /// Executes this operation.
    pub(crate) fn execute(
        self,
        ctx: &mut Context,
        id_map: &IdMap,
        out: &mut OutputChannel,
    ) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            let repo = ctx.repo.get()?;
            writeln!(
                out,
                "Uncommitted {} to {}",
                theme::CommitRef(id_map, &repo, self.oid),
                stack_id_to_branch_name(ctx, self.stack)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack")),
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Uncommits all changes from the selected commit to the given stack.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<UncommitResult> {
        but_api::commit::uncommit::commit_uncommit(
            ctx,
            vec![self.oid],
            Some(self.stack),
            DryRun::No,
        )
    }
}

impl SquashCommitsOperation {
    /// Executes this operation.
    pub(crate) fn execute(
        self,
        ctx: &mut Context,
        id_map: &IdMap,
        out: &mut OutputChannel,
    ) -> anyhow::Result<()> {
        let result = self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let repo = ctx.repo.get()?;
            let new_commit = theme::new_commit_ref(ctx, result.new_commit)?;
            if self.sources.len() == 1 {
                writeln!(
                    out,
                    "Squashed {} → {}",
                    theme::CommitRef(id_map, &repo, *self.sources.first()),
                    new_commit,
                )?;
            } else {
                writeln!(
                    out,
                    "Squashed {} commits → {}",
                    self.sources.len(),
                    new_commit,
                )?;
            }
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "ok": true,
                "new_commit_id": result.new_commit.to_string(),
                "squashed_count": self.sources.len(),
            }))?;
        }
        Ok(())
    }

    /// Executes `SquashCommits` by squashing source into target.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<CommitSquashResult> {
        but_api::commit::squash::commit_squash(
            ctx,
            self.sources.iter().copied().collect(),
            self.destination,
            self.how_to_combine_messages,
            DryRun::No,
        )
    }
}

impl<'a> MoveCommitToBranchOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(
        self,
        ctx: &mut Context,
        id_map: &IdMap,
        out: &mut OutputChannel,
    ) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            let repo = ctx.repo.get()?;
            writeln!(
                out,
                "Moved {} → {}",
                theme::CommitRef(id_map, &repo, self.oid),
                t.local_branch.paint(format!("[{}]", self.name))
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `MoveCommitToBranch` and returns the exact commit-move API result.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<CommitMoveResult> {
        let target_full_name = FullName::try_from(format!("refs/heads/{}", self.name))?;
        but_api::commit::move_commit::commit_move(
            ctx,
            vec![self.oid],
            RelativeTo::Reference(target_full_name),
            InsertSide::Below,
            DryRun::No,
        )
    }
}

impl<'a> BranchToUncommittedAreaOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Unstaged all {} changes.",
                t.local_branch.paint(format!("[{}]", self.from)),
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `BranchToUncommittedArea` by moving all branch-assigned hunks into uncommitted.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let source_stack_id = stack_id_for_branch_name(ctx, self.from)?;
        reassign_all_from_stack_to_stack(ctx, source_stack_id, None)
    }
}

impl<'a> BranchToStackOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged all {} changes to {}.",
                t.local_branch.paint(format!("[{}]", self.from)),
                stack_id_to_branch_name(ctx, self.to)
                    .map(|b| t.local_branch.paint(format!("[{b}]")))
                    .unwrap_or_else(|| t.important.paint("stack")),
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `BranchToStack` by moving all branch-assigned hunks into the target stack.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let source_stack_id = stack_id_for_branch_name(ctx, self.from)?;
        reassign_all_from_stack_to_stack(ctx, source_stack_id, Some(self.to))
    }
}

impl<'a> BranchToCommitOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        let result = self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            let new_commit = theme::new_commit_ref(ctx, result.new_commit)?;
            writeln!(
                out,
                "Amended assigned files {} → {}",
                t.local_branch.paint(format!("[{}]", self.name)),
                new_commit,
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "ok": true,
                "new_commit_id": result.new_commit.map(|c| c.to_string()),
            }))?;
        }
        Ok(())
    }

    /// Executes `BranchToCommit` and returns the exact commit-amend API result.
    ///
    /// When the source branch is not associated with a stack, this amends currently
    /// uncommitted hunks to match legacy `but rub` behavior.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<CommitCreateResult> {
        let stack_id = stack_id_for_branch_name(ctx, self.name)?;
        let changes = changes_for_stack_assignment(ctx, stack_id)?;
        but_api::commit::amend::commit_amend(ctx, self.oid, changes, DryRun::No)
    }
}

impl<'a> BranchToBranchOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(
                out,
                "Staged all {} changes to {}.",
                t.local_branch.paint(format!("[{}]", self.from)),
                t.local_branch.paint(format!("[{}]", self.to)),
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `BranchToBranch` by reassigning all hunks from one branch stack to another.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let source_stack_id = stack_id_for_branch_name(ctx, self.from)?;
        let target_stack_id = stack_id_for_branch_name(ctx, self.to)?;
        reassign_all_from_stack_to_stack(ctx, source_stack_id, target_stack_id)
    }
}

impl<'a> CommittedFileToBranchOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            writeln!(out, "Uncommitted changes")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `CommittedFileToBranch` and returns the exact uncommit API result.
    ///
    /// When the target branch is not associated with a stack, this uncommits file
    /// changes into uncommitted to match legacy `but rub` behavior.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<MoveChangesResult> {
        let stack_id = stack_id_for_branch_name(ctx, self.name)?;
        let relevant_changes = file_changes_from_commit(ctx, self.commit_oid, self.path)?;
        but_api::commit::uncommit::commit_uncommit_changes(
            ctx,
            self.commit_oid,
            relevant_changes,
            stack_id,
            DryRun::No,
        )
    }
}

impl<'a> CommittedFileToCommitOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            writeln!(out, "Moved files between commits!")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `CommittedFileToCommit` and returns the exact move-changes API result.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<MoveChangesResult> {
        let relevant_changes = file_changes_from_commit(ctx, self.commit_oid, self.path)?;
        but_api::commit::move_changes::commit_move_changes_between(
            ctx,
            self.commit_oid,
            self.oid,
            relevant_changes,
            DryRun::No,
        )
    }
}

impl<'a> CommittedFileToUncommittedAreaOperation<'a> {
    /// Executes this operation.
    pub(crate) fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        self.execute_inner(ctx)?;
        if let Some(out) = out.for_human() {
            writeln!(out, "Uncommitted changes")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        Ok(())
    }

    /// Executes `CommittedFileToUncommittedArea` and returns the exact uncommit API result.
    pub(crate) fn execute_inner(&self, ctx: &mut Context) -> anyhow::Result<MoveChangesResult> {
        let relevant_changes = file_changes_from_commit(ctx, self.commit_oid, self.path)?;
        but_api::commit::uncommit::commit_uncommit_changes(
            ctx,
            self.commit_oid,
            relevant_changes,
            None,
            DryRun::No,
        )
    }
}

impl<'a> RubOperation<'a> {
    /// Executes this operation, delegating to the wrapped operation payload.
    pub(crate) fn execute(
        self,
        ctx: &mut Context,
        id_map: &IdMap,
        out: &mut OutputChannel,
    ) -> anyhow::Result<()> {
        match self {
            RubOperation::UnassignUncommitted(operation) => operation.execute(ctx, out),
            RubOperation::UncommittedToCommit(operation) => operation.execute(ctx, out),
            RubOperation::UncommittedToBranch(operation) => operation.execute(ctx, out),
            RubOperation::UncommittedToStack(operation) => operation.execute(ctx, out),
            RubOperation::StackToUncommittedArea(operation) => operation.execute(ctx, out),
            RubOperation::StackToStack(operation) => operation.execute(ctx, out),
            RubOperation::StackToBranch(operation) => operation.execute(ctx, out),
            RubOperation::UncommittedAreaToCommit(operation) => operation.execute(ctx, out),
            RubOperation::UncommittedAreaToBranch(operation) => operation.execute(ctx, out),
            RubOperation::UncommittedAreaToStack(operation) => operation.execute(ctx, out),
            RubOperation::CommitToUncommittedArea(operation) => operation.execute(ctx, id_map, out),
            RubOperation::CommitToStack(operation) => operation.execute(ctx, id_map, out),
            RubOperation::SquashCommits(operation) => operation.execute(ctx, id_map, out),
            RubOperation::MoveCommitToBranch(operation) => operation.execute(ctx, id_map, out),
            RubOperation::BranchToUncommittedArea(operation) => operation.execute(ctx, out),
            RubOperation::BranchToStack(operation) => operation.execute(ctx, out),
            RubOperation::BranchToCommit(operation) => operation.execute(ctx, out),
            RubOperation::BranchToBranch(operation) => operation.execute(ctx, out),
            RubOperation::CommittedFileToBranch(operation) => operation.execute(ctx, out),
            RubOperation::CommittedFileToCommit(operation) => operation.execute(ctx, out),
            RubOperation::CommittedFileToUncommittedArea(operation) => operation.execute(ctx, out),
            RubOperation::StackToCommit(operation) => operation.execute(ctx, out),
        }
    }
}

fn hunk_assignments_from_uncommitted_sources<'a>(
    sources: &NonEmpty<&'a CliId>,
) -> Option<NonEmpty<&'a HunkAssignment>> {
    let mut hunk_assignments = Vec::new();
    for source in sources {
        let CliId::UncommittedHunkOrFile(uncommitted) = source else {
            return None;
        };
        hunk_assignments.extend(uncommitted.hunk_assignments.iter());
    }
    NonEmpty::from_vec(hunk_assignments)
}

fn commits_from_sources(sources: &NonEmpty<&CliId>) -> Option<NonEmpty<gix::ObjectId>> {
    let commits = sources
        .iter()
        .map(|source| {
            if let CliId::Commit { commit_id, .. } = source {
                Some(*commit_id)
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>()?;
    NonEmpty::from_vec(commits)
}

/// Determines the operation to perform for a given source and target combination.
/// Returns `Some(operation)` if the combination is valid, `None` otherwise.
///
/// This function is the single source of truth for what operations are valid.
/// Both `handle()` and disambiguation logic use this function.
pub(crate) fn route_operation<'a>(
    sources: NonEmpty<&'a CliId>,
    target: &'a CliId,
    how_to_combine_messages: MessageCombinationStrategy,
) -> Option<RubOperation<'a>> {
    use CliId::*;

    if sources.len() == 1 {
        let source = sources.first();
        match (source, target) {
            // Uncommitted -> *
            (UncommittedHunkOrFile(uncommitted), Uncommitted { .. }) => {
                let hunk_assignments = uncommitted.hunk_assignments.as_ref();
                let description = uncommitted.describe();
                Some(RubOperation::UnassignUncommitted(
                    UnassignUncommittedOperation {
                        hunk_assignments,
                        description,
                    },
                ))
            }
            (UncommittedHunkOrFile(uncommitted), Commit { commit_id, .. }) => {
                let hunk_assignments = uncommitted.hunk_assignments.as_ref();
                let description = uncommitted.describe();
                Some(RubOperation::UncommittedToCommit(
                    UncommittedToCommitOperation {
                        hunk_assignments,
                        description,
                        oid: *commit_id,
                    },
                ))
            }
            (UncommittedHunkOrFile(uncommitted), Branch { name, .. }) => {
                let hunk_assignments = uncommitted.hunk_assignments.as_ref();
                let description = uncommitted.describe();
                Some(RubOperation::UncommittedToBranch(
                    UncommittedToBranchOperation {
                        hunk_assignments,
                        description,
                        name,
                    },
                ))
            }
            (UncommittedHunkOrFile(uncommitted), Stack { stack_id, .. }) => {
                let hunk_assignments = uncommitted.hunk_assignments.as_ref();
                let description = uncommitted.describe();
                Some(RubOperation::UncommittedToStack(
                    UncommittedToStackOperation {
                        hunk_assignments,
                        description,
                        stack_id: *stack_id,
                    },
                ))
            }
            // Uncommitted path prefix -> *
            (
                PathPrefix {
                    hunk_assignments, ..
                },
                Uncommitted { .. },
            ) => {
                let hunk_assignments = hunk_assignments
                    .as_ref()
                    .map(|(_, hunk_assignment)| hunk_assignment);
                Some(RubOperation::UnassignUncommitted(
                    UnassignUncommittedOperation {
                        hunk_assignments,
                        description: "hunk(s)".to_string(),
                    },
                ))
            }
            (
                PathPrefix {
                    hunk_assignments, ..
                },
                Commit { commit_id, .. },
            ) => {
                let hunk_assignments = hunk_assignments
                    .as_ref()
                    .map(|(_, hunk_assignment)| hunk_assignment);
                Some(RubOperation::UncommittedToCommit(
                    UncommittedToCommitOperation {
                        hunk_assignments,
                        description: "hunk(s)".to_string(),
                        oid: *commit_id,
                    },
                ))
            }
            (
                PathPrefix {
                    hunk_assignments, ..
                },
                Branch { name, .. },
            ) => {
                let hunk_assignments = hunk_assignments
                    .as_ref()
                    .map(|(_, hunk_assignment)| hunk_assignment);
                Some(RubOperation::UncommittedToBranch(
                    UncommittedToBranchOperation {
                        hunk_assignments,
                        description: "hunk(s)".to_string(),
                        name,
                    },
                ))
            }
            (
                PathPrefix {
                    hunk_assignments, ..
                },
                Stack { stack_id, .. },
            ) => {
                let hunk_assignments = hunk_assignments
                    .as_ref()
                    .map(|(_, hunk_assignment)| hunk_assignment);
                Some(RubOperation::UncommittedToStack(
                    UncommittedToStackOperation {
                        hunk_assignments,
                        description: "hunk(s)".to_string(),
                        stack_id: *stack_id,
                    },
                ))
            }
            // Stack -> *
            (Stack { stack_id, .. }, Uncommitted { .. }) => Some(
                RubOperation::StackToUncommittedArea(StackToUncommittedAreaOperation {
                    stack_id: *stack_id,
                }),
            ),
            (Stack { stack_id: from, .. }, Stack { stack_id: to, .. }) => {
                Some(RubOperation::StackToStack(StackToStackOperation {
                    from: *from,
                    to: *to,
                }))
            }
            (Stack { stack_id: from, .. }, Branch { name: to, .. }) => {
                Some(RubOperation::StackToBranch(StackToBranchOperation {
                    from: *from,
                    to,
                }))
            }
            (Stack { stack_id, .. }, Commit { commit_id, .. }) => {
                Some(RubOperation::StackToCommit(StackToCommitOperation {
                    from: *stack_id,
                    to: *commit_id,
                }))
            }
            // Uncommitted -> *
            (Uncommitted { .. }, Commit { commit_id, .. }) => {
                Some(RubOperation::UncommittedAreaToCommit(
                    UncommittedAreaToCommitOperation { oid: *commit_id },
                ))
            }
            (Uncommitted { .. }, Branch { name, .. }) => {
                Some(RubOperation::UncommittedAreaToBranch(
                    UncommittedAreaToBranchOperation { to: name },
                ))
            }
            (Uncommitted { .. }, Stack { stack_id, .. }) => {
                Some(RubOperation::UncommittedAreaToStack(
                    UncommittedAreaToStackOperation { to: *stack_id },
                ))
            }
            // Commit -> *
            (Commit { commit_id, .. }, Uncommitted { .. }) => Some(
                RubOperation::CommitToUncommittedArea(CommitToUncommittedAreaOperation {
                    commits: NonEmpty::new(*commit_id),
                }),
            ),
            (
                Commit {
                    commit_id: source, ..
                },
                Commit {
                    commit_id: destination,
                    ..
                },
            ) => Some(RubOperation::SquashCommits(SquashCommitsOperation {
                sources: NonEmpty::new(*source),
                destination: *destination,
                how_to_combine_messages,
            })),
            (Commit { commit_id, .. }, Branch { name, .. }) => Some(
                RubOperation::MoveCommitToBranch(MoveCommitToBranchOperation {
                    oid: *commit_id,
                    name,
                }),
            ),
            (Commit { commit_id, .. }, Stack { stack_id, .. }) => {
                Some(RubOperation::CommitToStack(CommitToStackOperation {
                    oid: *commit_id,
                    stack: *stack_id,
                }))
            }
            // Branch -> *
            (Branch { name, .. }, Uncommitted { .. }) => {
                Some(RubOperation::BranchToUncommittedArea(
                    BranchToUncommittedAreaOperation { from: name },
                ))
            }
            (Branch { name: from, .. }, Stack { stack_id, .. }) => {
                Some(RubOperation::BranchToStack(BranchToStackOperation {
                    from,
                    to: *stack_id,
                }))
            }
            (Branch { name, .. }, Commit { commit_id, .. }) => {
                Some(RubOperation::BranchToCommit(BranchToCommitOperation {
                    name,
                    oid: *commit_id,
                }))
            }
            (Branch { name: from, .. }, Branch { name: to, .. }) => {
                Some(RubOperation::BranchToBranch(BranchToBranchOperation {
                    from,
                    to,
                }))
            }
            // CommittedFile -> *
            (
                CommittedFile {
                    path, commit_id, ..
                },
                Branch { name, .. },
            ) => Some(RubOperation::CommittedFileToBranch(
                CommittedFileToBranchOperation {
                    path: path.as_ref(),
                    commit_oid: *commit_id,
                    name,
                },
            )),
            (
                CommittedFile {
                    path,
                    commit_id: source,
                    ..
                },
                Commit {
                    commit_id: target, ..
                },
            ) => Some(RubOperation::CommittedFileToCommit(
                CommittedFileToCommitOperation {
                    path: path.as_ref(),
                    commit_oid: *source,
                    oid: *target,
                },
            )),
            (
                CommittedFile {
                    path, commit_id, ..
                },
                Uncommitted { .. },
            ) => Some(RubOperation::CommittedFileToUncommittedArea(
                CommittedFileToUncommittedAreaOperation {
                    path: path.as_ref(),
                    commit_oid: *commit_id,
                },
            )),
            // All other combinations are invalid
            _ => None,
        }
    } else {
        match target {
            Commit {
                commit_id: target_commit_id,
                id: _,
                change_id: _,
            } => {
                if let Some(commits) = sources
                    .iter()
                    .map(|source| match source {
                        Commit {
                            commit_id,
                            id: _,
                            change_id: _,
                        } => Some(*commit_id),
                        UncommittedHunkOrFile(..)
                        | PathPrefix { .. }
                        | CommittedFile { .. }
                        | Branch { .. }
                        | Uncommitted { .. }
                        | Stack { .. } => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .and_then(NonEmpty::from_vec)
                {
                    Some(RubOperation::SquashCommits(SquashCommitsOperation {
                        sources: commits,
                        destination: *target_commit_id,
                        how_to_combine_messages,
                    }))
                } else {
                    hunk_assignments_from_uncommitted_sources(&sources).map(|hunk_assignments| {
                        RubOperation::UncommittedToCommit(UncommittedToCommitOperation {
                            hunk_assignments,
                            description: "hunk(s)".to_string(),
                            oid: *target_commit_id,
                        })
                    })
                }
            }
            Uncommitted { .. } => hunk_assignments_from_uncommitted_sources(&sources)
                .map(|hunk_assignments| {
                    RubOperation::UnassignUncommitted(UnassignUncommittedOperation {
                        hunk_assignments,
                        description: "hunk(s)".to_string(),
                    })
                })
                .or_else(|| {
                    commits_from_sources(&sources).map(|commits| {
                        RubOperation::CommitToUncommittedArea(CommitToUncommittedAreaOperation {
                            commits,
                        })
                    })
                }),
            Stack { stack_id, .. } => {
                hunk_assignments_from_uncommitted_sources(&sources).map(|hunk_assignments| {
                    RubOperation::UncommittedToStack(UncommittedToStackOperation {
                        hunk_assignments,
                        description: "hunk(s)".to_string(),
                        stack_id: *stack_id,
                    })
                })
            }
            UncommittedHunkOrFile(..)
            | PathPrefix { .. }
            | CommittedFile { .. }
            | Branch { .. } => None,
        }
    }
}

pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
    target_str: &str,
    how_to_combine_messages: MessageCombinationStrategy,
) -> anyhow::Result<()> {
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;
    let (sources, target) = ids(ctx, &id_map, source_str, target_str, out)?;
    handle_resolved(ctx, &id_map, out, sources, target, how_to_combine_messages)
}

fn handle_resolved(
    ctx: &mut Context,
    id_map: &IdMap,
    out: &mut OutputChannel,
    sources: Vec<CliId>,
    target: CliId,
    how_to_combine_messages: MessageCombinationStrategy,
) -> anyhow::Result<()> {
    for source in sources {
        let Some(operation) =
            route_operation(NonEmpty::new(&source), &target, how_to_combine_messages)
        else {
            bail!(makes_no_sense_error(&source, &target))
        };

        operation.execute(ctx, id_map, out)?;
    }
    Ok(())
}

fn makes_no_sense_error(source: &CliId, target: &CliId) -> String {
    let t = theme::get();
    format!(
        "Operation doesn't make sense. Source {} is {} and target {} is {}.",
        t.cli_id.paint(source.to_short_string()),
        t.attention.paint(source.kind_for_humans()),
        t.cli_id.paint(target.to_short_string()),
        t.attention.paint(target.kind_for_humans())
    )
}

fn ids(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
    target: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<(Vec<CliId>, CliId)> {
    let sources = parse_sources_with_disambiguation(ctx, id_map, source, out)?;
    let target_result = id_map.parse_using_context(target, ctx)?;

    if target_result.is_empty() {
        return Err(anyhow::anyhow!(
            "Target '{target}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
        ));
    }

    if target_result.len() == 1 {
        return Ok((sources, target_result[0].clone()));
    }

    // Target is ambiguous - filter by checking validity with ALL sources
    // A target is only valid if it works with every source in the list
    let valid_targets: Vec<CliId> = target_result
        .into_iter()
        .filter(|target_candidate| {
            sources.iter().all(|src| {
                route_operation(
                    NonEmpty::new(src),
                    target_candidate,
                    MessageCombinationStrategy::KeepBoth,
                )
                .is_some()
            })
        })
        .collect();

    if valid_targets.is_empty() {
        // No valid operations found - this means all possible interpretations of the target
        // would result in invalid operations with at least one source.
        let source_summary = if sources.len() == 1 {
            format!("source {}", sources[0].to_short_string())
        } else {
            format!(
                "sources ({})",
                sources
                    .iter()
                    .map(|s| s.to_short_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        return Err(anyhow::anyhow!(
            "Target '{target}' matches multiple objects, but none would result in valid operations with all {source_summary}. Try using more characters or a different identifier."
        ));
    }

    if valid_targets.len() == 1 {
        // Disambiguation successful through validity filtering!
        return Ok((sources, valid_targets[0].clone()));
    }

    // Still ambiguous even after filtering by validity - prompt the user
    let selected_target = prompt_for_disambiguation(target, valid_targets, "the target", out)?;
    Ok((sources, selected_target))
}
fn create_snapshot(ctx: &mut Context, operation: OperationKind) {
    let mut guard = ctx.exclusive_worktree_access();
    create_snapshot_with_perm(ctx, operation, guard.write_permission());
}

fn create_snapshot_with_perm(
    ctx: &mut Context,
    operation: OperationKind,
    perm: &mut RepoExclusive,
) {
    let _snapshot = ctx
        .create_snapshot(SnapshotDetails::new(operation), perm)
        .ok(); // Ignore errors for snapshot creation
}

/// Resolves a single entity string to a CliId with disambiguation support.
///
/// If the entity matches multiple IDs, this will prompt the user to disambiguate
/// in interactive mode, or error in non-interactive mode.
///
/// # Arguments
/// * `id_map` - The ID map to resolve against
/// * `entity_str` - The string to resolve (e.g., "ab", "main")
/// * `context` - Description for error messages (e.g., "commit", "branch")
/// * `out` - Output channel for interactive prompts
///
/// # Returns
/// The resolved CliId
fn resolve_single_id(
    ctx: &mut Context,
    id_map: &IdMap,
    entity_str: &str,
    context: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<CliId> {
    let matches = id_map.parse_using_context(entity_str, ctx)?;

    if matches.is_empty() {
        return Err(IdResolutionError::new(format!(
            "{context} '{entity_str}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
        ))
        .into());
    }

    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }

    // Multiple matches - use disambiguation
    prompt_for_disambiguation(entity_str, matches, context, out)
}

/// Handler for `but uncommit <source>` - runs `but rub <source> zz`
/// Validates that source is a commit or file-in-commit.
pub(crate) fn handle_uncommit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
    discard: bool,
) -> anyhow::Result<()> {
    let t = theme::get();
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;
    let sources = parse_sources_with_disambiguation(ctx, &id_map, source_str, out)?;

    // Validate that all sources are commits or committed files
    for source in &sources {
        match source {
            CliId::Commit { .. } | CliId::CommittedFile { .. } => {
                // Valid types for uncommit
            }
            _ => {
                bail!(
                    "Cannot uncommit {} - it is {}. Only commits and files-in-commits can be uncommitted.",
                    t.cli_id.paint(source_str),
                    t.attention.paint(source.kind_for_humans())
                );
            }
        }
    }

    if discard {
        let json_mode = out.for_json().is_some();

        for source in sources {
            match source {
                CliId::Commit { commit_id, .. } => {
                    but_api::commit::discard_commit::commit_discard(ctx, commit_id, DryRun::No)?;

                    if !json_mode && let Some(out) = out.for_human() {
                        let repo = ctx.repo.get()?;
                        writeln!(
                            out,
                            "Discarded {}",
                            theme::CommitRef(&id_map, &repo, commit_id)
                        )?;
                    }
                }
                CliId::CommittedFile {
                    path, commit_id, ..
                } => {
                    crate::command::commit::file::uncommit_file_and_discard(
                        ctx,
                        path.as_ref(),
                        commit_id,
                        out,
                        !json_mode,
                    )?;
                }
                _ => {
                    unreachable!("uncommit sources were validated before execution");
                }
            }
        }

        if json_mode && let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }

        return Ok(());
    }

    // When every source is a committed file, uncommit them in a single batched
    // operation. This lets the backend group the changes by commit and apply them
    // in child-to-parent order, so callers can pass files from several commits
    // (and in any order) without hitting stale commit IDs from intermediate
    // rebases. Whole-commit sources keep the sequential `rub <source> zz` path.
    if sources
        .iter()
        .all(|source| matches!(source, CliId::CommittedFile { .. }))
    {
        return uncommit_committed_files(ctx, &id_map, out, &sources);
    }

    // Call the main rub handler with "zz" as target
    handle(
        ctx,
        out,
        source_str,
        "zz",
        MessageCombinationStrategy::KeepBoth,
    )
}

/// Uncommit one or more committed files as a single multi-source operation.
///
/// Committed-file ids are grouped by commit so each source commit yields a
/// single [`UncommitChangesSource`] with all of its changes combined. This
/// computes the diff specs for a commit in one pass and keeps the payload to one
/// entry per commit; the backend then applies them child-to-parent in one editor
/// session. Sources that could not be applied are reported best-effort without
/// failing the whole operation.
fn uncommit_committed_files(
    ctx: &mut Context,
    id_map: &IdMap,
    out: &mut OutputChannel,
    sources: &[CliId],
) -> anyhow::Result<()> {
    // Group the requested paths by commit, preserving first-seen commit order.
    let mut commit_order = Vec::new();
    let mut paths_by_commit: HashMap<gix::ObjectId, Vec<&BStr>> = HashMap::new();
    for source in sources {
        let CliId::CommittedFile {
            commit_id, path, ..
        } = source
        else {
            unreachable!("uncommit_committed_files only handles committed files");
        };
        match paths_by_commit.get_mut(commit_id) {
            Some(paths) => paths.push(path.as_ref()),
            None => {
                commit_order.push(*commit_id);
                paths_by_commit.insert(*commit_id, vec![path.as_ref()]);
            }
        }
    }

    // One source per commit, with the changes for all of its paths combined.
    let mut uncommit_sources = Vec::with_capacity(commit_order.len());
    for commit_id in commit_order {
        let paths = paths_by_commit
            .remove(&commit_id)
            .expect("commit id was just inserted");
        uncommit_sources.push(UncommitChangesSource {
            commit_id,
            changes: file_changes_from_commit_paths(ctx, commit_id, &paths)?,
        });
    }

    // One source per commit, so this is the number of commits we attempted.
    let source_count = uncommit_sources.len();
    let result = but_api::commit::uncommit::commit_uncommit_changes_from_commits(
        ctx,
        uncommit_sources,
        None,
        DryRun::No,
    )?;

    // The multi-source API is best-effort and returns `Ok` even when sources
    // fail. If every commit failed, nothing was uncommitted, so fail the command
    // (matching the old single-source behavior) rather than reporting success.
    if result.failures.len() == source_count {
        let repo = ctx.repo.get()?;
        let details = result
            .failures
            .iter()
            .map(|failure| {
                format!(
                    "{}: {}",
                    id_map
                        .change_id_ref(failure.commit_id)
                        .map(|change_id| change_id.padded_short_id())
                        .unwrap_or_else(|| shorten_object_id(&repo, failure.commit_id)),
                    failure.error
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        bail!("Failed to uncommit changes:\n{details}");
    }

    // Partial success: warn about the sources that could not be applied.
    if !result.failures.is_empty()
        && let Some(out) = out.for_human()
    {
        let repo = ctx.repo.get()?;
        for failure in &result.failures {
            writeln!(
                out,
                "Warning: could not uncommit changes from {}: {}",
                theme::CommitRef(id_map, &repo, failure.commit_id),
                failure.error
            )?;
        }
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "Uncommitted changes")?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }

    Ok(())
}

/// Handler for `but amend <file>... <commit>` - runs `but rub <file> <commit>`
/// semantics for one or more files/hunks.
///
/// Validates that sources are uncommitted files/hunks and target is a commit.
pub(crate) fn handle_amend(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_strs: &[String],
    commit_str: &str,
) -> anyhow::Result<()> {
    let t = theme::get();
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
    let mut files = Vec::new();
    for file_str in file_strs {
        files.extend(parse_uncommitted_sources_with_disambiguation(
            ctx, &id_map, file_str, out,
        )?);
    }
    let commit = resolve_single_id(ctx, &id_map, commit_str, "Commit", out)?;

    // Validate that all files are uncommitted
    for file in &files {
        match file {
            CliId::UncommittedHunkOrFile(_) => {
                // Valid type for amend
            }
            _ => {
                bail!(
                    "Cannot amend {} - it is {}. Only uncommitted files and hunks can be amended.",
                    t.cli_id.paint(file.to_short_string()),
                    t.attention.paint(file.kind_for_humans())
                );
            }
        }
    }

    // Validate that commit is a commit
    match &commit {
        CliId::Commit { .. } => {
            let Some(source_refs) = NonEmpty::from_vec(files.iter().collect()) else {
                bail!("At least one file or hunk must be provided.");
            };
            let Some(RubOperation::UncommittedToCommit(operation)) =
                route_operation(source_refs, &commit, MessageCombinationStrategy::KeepBoth)
            else {
                unreachable!("amend source and target were validated before execution");
            };

            create_snapshot_with_perm(ctx, OperationKind::AmendCommit, guard.write_permission());
            amend::uncommitted_to_commit_with_perm(
                ctx,
                operation.hunk_assignments,
                operation.description,
                operation.oid,
                out,
                guard.write_permission(),
            )?;
        }
        other => {
            bail!(
                "Cannot amend into {} - it is {}. Target must be a commit.",
                t.cli_id.paint(other.to_short_string()),
                t.attention.paint(other.kind_for_humans())
            );
        }
    }
    Ok(())
}

pub(crate) fn stage_cli_error(err: anyhow::Error) -> CliError {
    if let Some(stage_bad_input) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<StageBadInput>())
    {
        stage_bad_input.cli_error()
    } else {
        err.into()
    }
}

const STAGE_FILE_OR_HUNK_HINT: &str = "Run `but status --format json -f` to refresh CLI IDs, then retry with a file or hunk cliId from the output";
const STAGE_BRANCH_HINT: &str =
    "Use a branch name or branch cliId from `but status --format json -f`";

#[derive(Debug)]
struct StageBadInput {
    message: String,
    arg_name: &'static str,
    arg_value: String,
    hint: &'static str,
}

impl StageBadInput {
    fn file_or_hunk(value: &str, message: impl fmt::Display) -> Self {
        Self {
            message: message.to_string(),
            arg_name: "<FILE_OR_HUNK>",
            arg_value: value.to_owned(),
            hint: STAGE_FILE_OR_HUNK_HINT,
        }
    }

    fn branch(value: &str, message: impl fmt::Display) -> Self {
        Self {
            message: message.to_string(),
            arg_name: "<BRANCH>",
            arg_value: value.to_owned(),
            hint: STAGE_BRANCH_HINT,
        }
    }

    fn cli_error(&self) -> CliError {
        bad_input(&self.message)
            .arg_name(self.arg_name)
            .arg_value(&self.arg_value)
            .hint(self.hint)
            .into()
    }
}

impl fmt::Display for StageBadInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for StageBadInput {}

fn stage_resolution_error(
    err: anyhow::Error,
    wrap: impl FnOnce(anyhow::Error) -> StageBadInput,
) -> anyhow::Error {
    if err
        .chain()
        .any(|cause| cause.downcast_ref::<IdResolutionError>().is_some())
    {
        wrap(err).into()
    } else {
        err
    }
}

/// Handler for `but stage <file_or_hunk> <branch>` - runs `but rub <file_or_hunk> <branch>`
/// Validates that file_or_hunk is uncommitted or a path prefix, and that branch is a branch.
pub(crate) fn handle_stage(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_or_hunk_str: &str,
    branch_str: &str,
) -> anyhow::Result<()> {
    let t = theme::get();
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;
    let files = parse_uncommitted_sources_with_disambiguation(ctx, &id_map, file_or_hunk_str, out)
        .map_err(|err| {
            stage_resolution_error(err, |err| {
                StageBadInput::file_or_hunk(file_or_hunk_str, err)
            })
        })?;
    let branch = resolve_single_id(ctx, &id_map, branch_str, "Branch", out)
        .map_err(|err| stage_resolution_error(err, |err| StageBadInput::branch(branch_str, err)))?;

    // Validate that all files are uncommitted or a path prefix
    for file in &files {
        match file {
            CliId::UncommittedHunkOrFile(_) | CliId::PathPrefix { .. } => {
                // Valid type for stage
            }
            _ => {
                return Err(StageBadInput::file_or_hunk(
                    file_or_hunk_str,
                    format!(
                        "Cannot stage {} - it is {}. Only uncommitted files and hunks can be staged.",
                        t.cli_id.paint(file.to_short_string()),
                        t.attention.paint(file.kind_for_humans())
                    ),
                )
                .into());
            }
        }
    }

    // Validate that branch is a branch
    match &branch {
        CliId::Branch { .. } => {
            // Valid type for target
        }
        other => {
            return Err(StageBadInput::branch(
                branch_str,
                format!(
                    "Cannot stage to {} - it is {}. Target must be a branch.",
                    t.cli_id.paint(other.to_short_string()),
                    t.attention.paint(other.kind_for_humans())
                ),
            )
            .into());
        }
    }

    handle_resolved(
        ctx,
        &id_map,
        out,
        files,
        branch,
        MessageCombinationStrategy::KeepBoth,
    )
}

/// Handler for `but stage --tui` - interactive hunk selection TUI.
/// If `branch_str` is None, prompts the user to select a branch.
pub(crate) fn handle_stage_tui(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_str: Option<&str>,
) -> anyhow::Result<()> {
    let t = theme::get();
    use crate::tui::stage_viewer::{StageFileEntry, StageResult};

    let id_map = IdMap::legacy_new_from_context(ctx, None)?;

    // Resolve branch: from flag, or interactive selection
    let branch_name = if let Some(branch_str) = branch_str {
        let branch = resolve_single_id(ctx, &id_map, branch_str, "Branch", out).map_err(|err| {
            stage_resolution_error(err, |err| StageBadInput::branch(branch_str, err))
        })?;
        match &branch {
            CliId::Branch { name, .. } => name.clone(),
            other => {
                return Err(StageBadInput::branch(
                    branch_str,
                    format!(
                        "Cannot stage to {} - it is {}. Target must be a branch.",
                        t.cli_id.paint(other.to_short_string()),
                        t.attention.paint(other.kind_for_humans())
                    ),
                )
                .into());
            }
        }
    } else {
        // Get available stacks, use top branch of each as the staging target
        let stacks = crate::legacy::workspace::applied_stacks(ctx)?;
        let stack_top_branches: Vec<String> = stacks
            .iter()
            .filter_map(|s| s.top_branch_name().map(ToOwned::to_owned))
            .collect();

        if stack_top_branches.is_empty() {
            // Auto-create a branch with a generated name
            let branch_name = but_api::legacy::workspace::canned_branch_name(ctx)?;
            but_api::legacy::stack::create_reference(
                ctx,
                but_api::legacy::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor: None,
                },
            )?;
            if let Some(out) = out.for_human() {
                writeln!(out, "Created new branch '{branch_name}'")?;
            }
            branch_name
        } else if stack_top_branches.len() == 1 {
            stack_top_branches.into_iter().next().unwrap()
        } else {
            match crate::tui::stage_viewer::run_branch_selector(&stack_top_branches)? {
                Some(name) => name,
                None => {
                    if let Some(out) = out.for_human() {
                        writeln!(out, "Stage cancelled.")?;
                    }
                    return Ok(());
                }
            }
        }
    };

    let files = StageFileEntry::from_worktree(&id_map);

    if files.is_empty() {
        bail!("No uncommitted changes to stage.");
    }

    let result = crate::tui::stage_viewer::run_stage_viewer(files, &branch_name)?;

    match result {
        StageResult::Stage {
            selected,
            unselected,
        } => {
            if selected.is_empty() {
                if let Some(out) = out.for_human() {
                    writeln!(out, "No hunks selected. Nothing staged.")?;
                }
                return Ok(());
            }
            create_snapshot(ctx, OperationKind::MoveHunk);
            // Stage selected hunks to the target branch
            let mut reqs =
                assign::to_assignment_request(ctx, selected.into_iter(), Some(&branch_name))?;
            // Unassign deselected hunks (set stack_id to None)
            reqs.extend(assign::to_assignment_request(
                ctx,
                unselected.into_iter(),
                None,
            )?);
            assign::do_assignments(ctx, reqs)?;
            if let Some(out) = out.for_human() {
                let t = theme::get();
                writeln!(
                    out,
                    "Staged selected hunks → {}.",
                    t.local_branch.paint(format!("[{branch_name}]"))
                )?;
            }
            Ok(())
        }
        StageResult::Cancelled => {
            if let Some(out) = out.for_human() {
                writeln!(out, "Stage cancelled.")?;
            }
            Ok(())
        }
    }
}

/// Handler for `but unstage <file_or_hunk> [branch]` - runs `but rub <file_or_hunk> zz`
/// Validates that file_or_hunk is uncommitted or a path prefix. Optionally
/// validates it's assigned to the specified branch.
pub(crate) fn handle_unstage(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_or_hunk_str: &str,
    branch_str: Option<&str>,
) -> anyhow::Result<()> {
    let t = theme::get();
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;
    let files = parse_uncommitted_sources_with_disambiguation(ctx, &id_map, file_or_hunk_str, out)?;

    // Validate that all files are uncommitted or a path prefix
    for file in &files {
        match file {
            CliId::UncommittedHunkOrFile(_) | CliId::PathPrefix { .. } => {
                // Valid type for unstage
            }
            _ => {
                bail!(
                    "Cannot unstage {} - it is {}. Only uncommitted files and hunks can be unstaged.",
                    t.cli_id.paint(file.to_short_string()),
                    t.attention.paint(file.kind_for_humans())
                );
            }
        }
    }

    // If a branch is specified, validate it exists (but we don't strictly require the file to be assigned to it)
    if let Some(branch_name) = branch_str {
        let branch = resolve_single_id(ctx, &id_map, branch_name, "Branch", out)?;
        match &branch {
            CliId::Branch { .. } => {
                // Valid - branch exists
            }
            other => {
                bail!(
                    "Cannot unstage from {} - it is {}. Target must be a branch.",
                    t.cli_id.paint(other.to_short_string()),
                    t.attention.paint(other.kind_for_humans())
                );
            }
        }
    }

    // Unassign the already-resolved files directly; re-parsing the raw string
    // through the full namespace could hit the very shadowing the scoped
    // resolution above just avoided.
    let target = id_map.uncommitted().clone();
    handle_resolved(
        ctx,
        &id_map,
        out,
        files,
        target,
        MessageCombinationStrategy::KeepBoth,
    )
}

/// Builds assignment requests for selected hunks and assigns them to `target_stack_id`.
fn assignment_requests_for_selected_hunks<'a>(
    hunks: impl Iterator<Item = &'a HunkAssignment>,
    target_stack_id: Option<StackId>,
) -> Vec<HunkAssignmentRequest> {
    hunks
        .map(|assignment| HunkAssignmentRequest {
            hunk_header: assignment.hunk_header,
            path_bytes: assignment.path_bytes.to_owned(),
            target: target_stack_id.map(|stack_id| HunkAssignmentTarget::Stack { stack_id }),
        })
        .collect()
}

/// Resolves a branch name into its workspace stack id, if any.
fn stack_id_for_branch_name(
    ctx: &mut Context,
    branch_name: &str,
) -> anyhow::Result<Option<StackId>> {
    let target_branch_full_name = FullName::try_from(format!("refs/heads/{branch_name}"))?;
    let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
    Ok(ws
        .find_segment_and_stack_by_refname(target_branch_full_name.as_ref())
        .and_then(|(stack, _segment)| stack.id))
}

/// Reassigns all current worktree assignments from `source_stack_id` to `target_stack_id`.
fn reassign_all_from_stack_to_stack(
    ctx: &mut Context,
    source_stack_id: Option<StackId>,
    target_stack_id: Option<StackId>,
) -> anyhow::Result<()> {
    let requests = but_api::diff::changes_in_worktree(ctx, true)?
        .assignments
        .into_iter()
        .filter(|assignment| assignment.stack_id == source_stack_id)
        .map(|assignment| HunkAssignmentRequest {
            hunk_header: assignment.hunk_header,
            path_bytes: assignment.path_bytes,
            target: target_stack_id.map(|stack_id| HunkAssignmentTarget::Stack { stack_id }),
        })
        .collect::<Vec<_>>();

    but_api::diff::assign_hunk(ctx, requests)
}

/// Collects worktree diff specs that are currently assigned to `stack_id`.
fn changes_for_stack_assignment(
    ctx: &mut Context,
    stack_id: Option<StackId>,
) -> anyhow::Result<Vec<DiffSpec>> {
    let assignments = but_api::diff::changes_in_worktree(ctx, true)?
        .assignments
        .into_iter()
        .filter(|assignment| assignment.stack_id == stack_id);
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
    builder.push_hunk_assignments(assignments)?;
    Ok(builder.into_diff_specs())
}

/// Computes diff specs for changes to `path` in `commit_oid` relative to its first parent.
fn file_changes_from_commit(
    ctx: &Context,
    commit_oid: gix::ObjectId,
    path: &BStr,
) -> anyhow::Result<Vec<DiffSpec>> {
    file_changes_from_commit_paths(ctx, commit_oid, std::slice::from_ref(&path))
}

/// Compute the combined diff specs for several `paths` in a single commit, using
/// one workspace/db and [`DiffSpecBuilder`] setup for all of them.
fn file_changes_from_commit_paths(
    ctx: &Context,
    commit_oid: gix::ObjectId,
    paths: &[&BStr],
) -> anyhow::Result<Vec<DiffSpec>> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
    for path in paths {
        builder.push_changes_from_path_in_commit(path, commit_oid, "no parents")?;
    }
    Ok(builder.into_diff_specs())
}

#[cfg(test)]
mod tests {
    use bstr::BString;
    use nonempty::NonEmpty;

    use crate::id::UNCOMMITTED;

    use super::*;

    // Helper to create test CliIds
    fn uncommitted_id() -> CliId {
        CliId::UncommittedHunkOrFile(crate::id::UncommittedHunkOrFile {
            id: "ab".to_string(),
            hunk_assignments: NonEmpty::new(but_hunk_assignment::HunkAssignment {
                id: None,
                hunk_header: None,
                path: "test.txt".to_string(),
                path_bytes: BString::from("test.txt"),
                stack_id: None,
                branch_ref_bytes: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            }),
            is_entire_file: true,
        })
    }

    fn committed_file_id() -> CliId {
        CliId::CommittedFile {
            commit_id: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            path: BString::from("test.txt"),
            id: "cd".to_string(),
        }
    }

    fn branch_id() -> CliId {
        CliId::Branch {
            name: "main".to_string(),
            id: "ef".to_string(),
            stack_id: None,
        }
    }

    fn commit_id() -> CliId {
        CliId::Commit {
            commit_id: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            id: "gh".to_string(),
            change_id: None,
        }
    }

    fn uncommitted_area_id() -> CliId {
        CliId::Uncommitted {
            id: "zz".to_string(),
        }
    }

    fn stack_id() -> CliId {
        CliId::Stack {
            id: "ij".to_string(),
            stack_id: StackId::generate(),
        }
    }

    #[test]
    fn test_route_operation_uncommitted_hunk_to_targets() {
        let uncommitted = uncommitted_id();

        // Valid: Uncommitted -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &uncommitted_area_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Uncommitted -> Commit
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &commit_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Uncommitted -> Branch
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &branch_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Uncommitted -> Stack
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &stack_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Invalid: Uncommitted -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &uncommitted_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: Uncommitted -> CommittedFile
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &committed_file_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );
    }

    #[test]
    fn test_route_operation_commit_to_targets() {
        let commit = commit_id();

        // Valid: Commit -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&commit),
                &uncommitted_area_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Commit -> Commit
        assert!(
            route_operation(
                NonEmpty::new(&commit),
                &commit_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Commit -> Branch
        assert!(
            route_operation(
                NonEmpty::new(&commit),
                &branch_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Commit -> Stack
        assert!(
            route_operation(
                NonEmpty::new(&commit),
                &stack_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Invalid: Commit -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&commit),
                &uncommitted_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: Commit -> CommittedFile
        assert!(
            route_operation(
                NonEmpty::new(&commit),
                &committed_file_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );
    }

    #[test]
    fn test_route_operation_branch_to_targets() {
        let branch = branch_id();

        // Valid: Branch -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&branch),
                &uncommitted_area_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Branch -> Stack
        assert!(
            route_operation(
                NonEmpty::new(&branch),
                &stack_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Branch -> Commit
        assert!(
            route_operation(
                NonEmpty::new(&branch),
                &commit_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Branch -> Branch
        assert!(
            route_operation(
                NonEmpty::new(&branch),
                &branch_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Invalid: Branch -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&branch),
                &uncommitted_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: Branch -> CommittedFile
        assert!(
            route_operation(
                NonEmpty::new(&branch),
                &committed_file_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );
    }

    #[test]
    fn test_route_operation_stack_to_targets() {
        let stack = stack_id();

        // Valid: Stack -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&stack),
                &uncommitted_area_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Stack -> Stack
        assert!(
            route_operation(
                NonEmpty::new(&stack),
                &stack_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Stack -> Branch
        assert!(
            route_operation(
                NonEmpty::new(&stack),
                &branch_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Stack -> Commit
        assert!(
            route_operation(
                NonEmpty::new(&stack),
                &commit_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Invalid: Stack -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&stack),
                &uncommitted_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: Stack -> CommittedFile
        assert!(
            route_operation(
                NonEmpty::new(&stack),
                &committed_file_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );
    }

    #[test]
    fn test_route_operation_uncommitted_area_to_targets() {
        let uncommitted = uncommitted_area_id();

        // Valid: Uncommitted -> Commit
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &commit_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Uncommitted -> Branch
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &branch_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: Uncommitted -> Stack
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &stack_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Invalid: Uncommitted -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &uncommitted_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: Uncommitted -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &uncommitted_area_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: Uncommitted -> CommittedFile
        assert!(
            route_operation(
                NonEmpty::new(&uncommitted),
                &committed_file_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );
    }

    #[test]
    fn test_route_operation_committed_file_to_targets() {
        let committed_file = committed_file_id();

        // Valid: CommittedFile -> Branch
        assert!(
            route_operation(
                NonEmpty::new(&committed_file),
                &branch_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: CommittedFile -> Commit
        assert!(
            route_operation(
                NonEmpty::new(&committed_file),
                &commit_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Valid: CommittedFile -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&committed_file),
                &uncommitted_area_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_some()
        );

        // Invalid: CommittedFile -> Uncommitted
        assert!(
            route_operation(
                NonEmpty::new(&committed_file),
                &uncommitted_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: CommittedFile -> Stack
        assert!(
            route_operation(
                NonEmpty::new(&committed_file),
                &stack_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );

        // Invalid: CommittedFile -> CommittedFile
        assert!(
            route_operation(
                NonEmpty::new(&committed_file),
                &committed_file_id(),
                MessageCombinationStrategy::KeepBoth
            )
            .is_none()
        );
    }

    /// Verifies that route_operation returns the correct variant (not just Some/None).
    /// This test ensures the routing logic maps to the right operation types.
    #[test]
    fn test_route_operation_returns_correct_variants() {
        let uncommitted_hunk = uncommitted_id();
        let committed_file = committed_file_id();
        let branch = branch_id();
        let commit = commit_id();
        let uncommitted_area = uncommitted_area_id();
        let stack = stack_id();

        // Test a representative sample of operations to verify correct variant matching
        // We use match with wildcard to verify the variant type without destructuring all fields

        // Uncommitted -> Uncommitted should be UnassignUncommitted
        match route_operation(
            NonEmpty::new(&uncommitted_hunk),
            &uncommitted_area,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::UnassignUncommitted(..)) => {}
            _ => panic!("Expected UnassignUncommitted variant"),
        }

        // Uncommitted -> Commit should be UncommittedToCommit
        match route_operation(
            NonEmpty::new(&uncommitted_hunk),
            &commit,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::UncommittedToCommit(..)) => {}
            _ => panic!("Expected UncommittedToCommit variant"),
        }

        // Commit -> Commit should be SquashCommits
        match route_operation(
            NonEmpty::new(&commit),
            &commit_id(),
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::SquashCommits(..)) => {}
            _ => panic!("Expected SquashCommits variant"),
        }

        // Commit -> Uncommitted should be CommitToUncommittedArea
        match route_operation(
            NonEmpty::new(&commit),
            &uncommitted_area,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::CommitToUncommittedArea(..)) => {}
            _ => panic!("Expected CommitToUncommittedArea variant"),
        }

        // Commit -> Stack should be CommitToStack
        match route_operation(
            NonEmpty::new(&commit),
            &stack,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::CommitToStack(..)) => {}
            _ => panic!("Expected CommitToStack variant"),
        }

        // Branch -> Stack should be BranchToStack
        match route_operation(
            NonEmpty::new(&branch),
            &stack,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::BranchToStack(..)) => {}
            _ => panic!("Expected BranchToStack variant"),
        }

        // Stack -> Branch should be StackToBranch
        match route_operation(
            NonEmpty::new(&stack),
            &branch,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::StackToBranch(..)) => {}
            _ => panic!("Expected StackToBranch variant"),
        }

        // Stack -> Commit should be StackToCommit
        match route_operation(
            NonEmpty::new(&stack),
            &commit,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::StackToCommit(..)) => {}
            _ => panic!("Expected StackToCommit variant"),
        }

        // CommittedFile -> Commit should be CommittedFileToCommit
        match route_operation(
            NonEmpty::new(&committed_file),
            &commit,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::CommittedFileToCommit(..)) => {}
            _ => panic!("Expected CommittedFileToCommit variant"),
        }

        // Uncommitted -> Stack should be UncommittedAreaToStack
        match route_operation(
            NonEmpty::new(&uncommitted_area),
            &stack,
            MessageCombinationStrategy::KeepBoth,
        ) {
            Some(RubOperation::UncommittedAreaToStack(..)) => {}
            _ => panic!("Expected UncommittedAreaToStack variant"),
        }
    }

    #[test]
    fn uncommit_multiple_commits() {
        let commit = commit_id();
        let commits = NonEmpty::from_vec(Vec::from([&commit, &commit, &commit])).unwrap();
        let uncommitted = CliId::Uncommitted {
            id: UNCOMMITTED.to_owned(),
        };
        let op =
            route_operation(commits, &uncommitted, MessageCombinationStrategy::KeepBoth).unwrap();
        let RubOperation::CommitToUncommittedArea(CommitToUncommittedAreaOperation {
            commits: routed_commits,
        }) = op
        else {
            panic!("unexpected op: {op:?}");
        };
        assert_eq!(routed_commits.len(), 3);
    }
}
