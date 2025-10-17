use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use but_rebase::RebaseStep;
use but_workspace::stack_ext::StackExt;
use gitbutler_command_context::CommandContext;
use gitbutler_hunk_dependency::locks::HunkDependencyResult;
use gitbutler_oxidize::{ObjectIdExt, OidExt, RepoExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommited_changes};
use serde::Serialize;

use crate::{
    BranchStatus, VirtualBranchesExt, compute_workspace_dependencies,
    dependencies::commit_dependencies_from_workspace,
};

/// move a commit from one stack to another
///
/// commit will end up at the top of the destination stack
pub(crate) fn move_commit(
    ctx: &CommandContext,
    target_stack_id: StackId,
    subject_commit_oid: git2::Oid,
    perm: &mut WorktreeWritePermission,
    source_stack_id: StackId,
) -> Result<Option<MoveCommitIllegalAction>> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let vb_state = ctx.project().virtual_branches();
    let repo = ctx.repo();

    let applied_stacks = vb_state
        .list_stacks_in_workspace()
        .context("failed to read virtual branches")?;

    if !applied_stacks.iter().any(|b| b.id == target_stack_id) {
        bail!("Destination branch not found");
    }

    let default_target = vb_state.get_default_target()?;

    let mut source_stack = vb_state
        .try_stack(source_stack_id)?
        .ok_or(anyhow!("Source stack not found"))?;

    let destination_stack = vb_state
        .try_stack(target_stack_id)?
        .ok_or(anyhow!("Destination branch not found"))?;

    let subject_commit = repo
        .find_commit(subject_commit_oid)
        .with_context(|| format!("commit {subject_commit_oid} to be moved could not be found"))?;

    let source_branch_diffs = get_source_branch_diffs(ctx, &source_stack)?;

    let workspace_dependencies = compute_workspace_dependencies(
        ctx,
        &default_target.sha,
        &source_branch_diffs,
        &applied_stacks,
    )?;

    if let Some(illegal_action) = take_commit_from_source_stack(
        ctx,
        &mut source_stack,
        subject_commit,
        &workspace_dependencies,
    )? {
        return Ok(Some(illegal_action));
    }

    move_commit_to_destination_stack(&vb_state, ctx, destination_stack, subject_commit_oid)?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    // Even if this fails, it's not actionable
    let _ = update_uncommited_changes(ctx, old_workspace, new_workspace, perm);
    crate::integration::update_workspace_commit(&vb_state, ctx, false)
        .context("failed to update gitbutler workspace")?;

    Ok(None)
}

fn get_source_branch_diffs(
    ctx: &CommandContext,
    source_stack: &gitbutler_stack::Stack,
) -> Result<BranchStatus> {
    let repo = ctx.repo();
    let source_stack_head = repo.find_commit(source_stack.head_oid(&repo.to_gix()?)?.to_git2())?;
    let source_stack_head_tree = source_stack_head.tree()?;
    let uncommitted_changes_tree = repo.find_tree(source_stack.tree(ctx)?)?;

    let uncommitted_changes_diff = gitbutler_diff::trees(
        repo,
        &source_stack_head_tree,
        &uncommitted_changes_tree,
        true,
    )
    .map(|diff| gitbutler_diff::diff_files_into_hunks(&diff).collect::<HashMap<_, _>>())?;

    Ok(uncommitted_changes_diff)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum MoveCommitIllegalAction {
    /// The commit being moved has dependencies on some of its parent commits.
    DependsOnCommits(Vec<String>),
    /// The commit being moves has dependent child commits.
    HasDependentChanges(Vec<String>),
    /// The commit being moved has dependent uncommitted changes. (This should not matter in the v3 worlds)
    HasDependentUncommittedChanges,
}

/// Remove the commit from the source stack.
///
/// Will fail if the commit is not in the source stack or if has dependent changes.
fn take_commit_from_source_stack(
    ctx: &CommandContext,
    source_stack: &mut gitbutler_stack::Stack,
    subject_commit: git2::Commit<'_>,
    workspace_dependencies: &HunkDependencyResult,
) -> Result<Option<MoveCommitIllegalAction>, anyhow::Error> {
    let commit_dependencies = commit_dependencies_from_workspace(
        workspace_dependencies,
        source_stack.id,
        subject_commit.id(),
    );

    if !commit_dependencies.dependencies.is_empty() {
        return Ok(Some(MoveCommitIllegalAction::DependsOnCommits(
            commit_dependencies
                .dependencies
                .iter()
                .map(|d| d.to_string())
                .collect(),
        )));
    }

    if !commit_dependencies.reverse_dependencies.is_empty() {
        return Ok(Some(MoveCommitIllegalAction::HasDependentChanges(
            commit_dependencies
                .reverse_dependencies
                .iter()
                .map(|d| d.to_string())
                .collect(),
        )));
    }

    if !commit_dependencies.dependent_diffs.is_empty() {
        return Ok(Some(
            MoveCommitIllegalAction::HasDependentUncommittedChanges,
        ));
    }

    let merge_base = source_stack.merge_base(ctx)?;
    let gix_repo = ctx.gix_repo()?;
    let steps = source_stack
        .as_rebase_steps(ctx, &gix_repo)?
        .into_iter()
        .filter(|s| match s {
            RebaseStep::Pick {
                commit_id,
                new_message: _,
            } => commit_id != &subject_commit.id().to_gix(),
            _ => true,
        })
        .collect::<Vec<_>>();
    let mut rebase = but_rebase::Rebase::new(&gix_repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;
    let new_source_head = output.top_commit.to_git2();

    source_stack.set_heads_from_rebase_output(ctx, output.references)?;
    let vb_state = ctx.project().virtual_branches();
    source_stack.set_stack_head(&vb_state, &gix_repo, new_source_head, None)?;
    Ok(None)
}

/// Move the commit to the destination stack.
fn move_commit_to_destination_stack(
    vb_state: &VirtualBranchesHandle,
    ctx: &CommandContext,
    mut destination_stack: gitbutler_stack::Stack,
    commit_id: git2::Oid,
) -> Result<(), anyhow::Error> {
    let gix_repo = ctx.gix_repo()?;
    let merge_base = destination_stack.merge_base(ctx)?;
    let mut steps = destination_stack.as_rebase_steps(ctx, &gix_repo)?;
    // TODO: In the future we can make the API provide additional info for exacly where to place the commit on the destination stack
    steps.insert(
        steps.len() - 1,
        RebaseStep::Pick {
            commit_id: commit_id.to_gix(),
            new_message: None,
        },
    );
    let mut rebase = but_rebase::Rebase::new(&gix_repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;
    let new_destination_head_oid = output.top_commit.to_git2();

    destination_stack.set_heads_from_rebase_output(ctx, output.references)?;
    destination_stack.set_stack_head(vb_state, &gix_repo, new_destination_head_oid, None)?;
    Ok(())
}
