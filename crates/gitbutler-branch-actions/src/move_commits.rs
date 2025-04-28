use std::collections::HashMap;

use anyhow::{anyhow, bail};
use anyhow::{Context, Result};
use but_rebase::RebaseStep;
use but_workspace::stack_ext::StackExt;
use gitbutler_command_context::CommandContext;
use gitbutler_hunk_dependency::locks::HunkDependencyResult;
use gitbutler_oxidize::{ObjectIdExt, OidExt, RepoExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{update_uncommited_changes, WorkspaceState};
#[allow(deprecated)]
use gitbutler_workspace::{checkout_branch_trees, compute_updated_branch_head};

use crate::dependencies::commit_dependencies_from_workspace;
use crate::VirtualBranchesExt;
use crate::{compute_workspace_dependencies, BranchStatus};

/// move a commit from one stack to another
///
/// commit will end up at the top of the destination stack
pub(crate) fn move_commit(
    ctx: &CommandContext,
    target_stack_id: StackId,
    subject_commit_oid: git2::Oid,
    perm: &mut WorktreeWritePermission,
    source_stack_id: StackId,
) -> Result<()> {
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

    take_commit_from_source_stack(
        ctx,
        repo,
        &mut source_stack,
        subject_commit,
        &workspace_dependencies,
    )?;

    move_commit_to_destination_stack(&vb_state, ctx, repo, destination_stack, subject_commit_oid)?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    if ctx.app_settings().feature_flags.v3 {
        update_uncommited_changes(ctx, old_workspace, new_workspace, perm)?;
    } else {
        #[allow(deprecated)]
        checkout_branch_trees(ctx, perm)?;
    }
    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

fn get_source_branch_diffs(
    ctx: &CommandContext,
    source_stack: &gitbutler_stack::Stack,
) -> Result<BranchStatus> {
    let repo = ctx.repo();
    let source_stack_head = repo.find_commit(source_stack.head(&repo.to_gix()?)?.to_git2())?;
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

/// Remove the commit from the source stack.
///
/// Will fail if the commit is not in the source stack or if has dependent changes.
fn take_commit_from_source_stack(
    ctx: &CommandContext,
    repo: &git2::Repository,
    source_stack: &mut gitbutler_stack::Stack,
    subject_commit: git2::Commit<'_>,
    workspace_dependencies: &HunkDependencyResult,
) -> Result<(), anyhow::Error> {
    let commit_dependencies = commit_dependencies_from_workspace(
        workspace_dependencies,
        source_stack.id,
        subject_commit.id(),
    );

    if !commit_dependencies.dependencies.is_empty() {
        bail!("Commit depends on other changes");
    }

    if !commit_dependencies.reverse_dependencies.is_empty() {
        bail!("Commit has dependent changes");
    }

    if !commit_dependencies.dependent_diffs.is_empty() {
        bail!("Commit has dependent uncommitted changes");
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

    let (new_head_oid, new_tree_oid) = if ctx.app_settings().feature_flags.v3 {
        (new_source_head, None)
    } else {
        #[allow(deprecated)]
        let res = compute_updated_branch_head(repo, &gix_repo, source_stack, new_source_head, ctx)?;
        (res.head, Some(res.tree))
    };

    source_stack.set_heads_from_rebase_output(ctx, output.references)?;
    let vb_state = ctx.project().virtual_branches();
    source_stack.set_stack_head(&vb_state, &gix_repo, new_head_oid, new_tree_oid)?;
    Ok(())
}

/// Move the commit to the destination stack.
fn move_commit_to_destination_stack(
    vb_state: &VirtualBranchesHandle,
    ctx: &CommandContext,
    repo: &git2::Repository,
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

    let (new_destination_head_oid, new_destination_tree_oid) =
        if ctx.app_settings().feature_flags.v3 {
            (new_destination_head_oid, None)
        } else {
            #[allow(deprecated)]
            let res = compute_updated_branch_head(
                repo,
                &gix_repo,
                &destination_stack,
                new_destination_head_oid,
                ctx,
            )?;
            (res.head, Some(res.tree))
        };

    destination_stack.set_heads_from_rebase_output(ctx, output.references)?;
    destination_stack.set_stack_head(
        vb_state,
        &gix_repo,
        new_destination_head_oid,
        new_destination_tree_oid,
    )?;
    Ok(())
}
