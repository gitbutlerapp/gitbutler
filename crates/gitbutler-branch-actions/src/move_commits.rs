use std::collections::HashMap;

use anyhow::{anyhow, bail};
use anyhow::{Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_hunk_dependency::locks::HunkDependencyResult;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gitbutler_repo::rebase::cherry_rebase_group;
use gitbutler_stack::StackId;
use gitbutler_workspace::{checkout_branch_trees, compute_updated_branch_head, BranchHeadAndTree};

use crate::dependencies::commit_dependencies_from_workspace;
use crate::{compute_workspace_dependencies, BranchStatus};
use crate::{conflicts::RepoConflictsExt, VirtualBranchesExt};

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
    ctx.assure_resolved()?;
    let vb_state = ctx.project().virtual_branches();
    let repo = ctx.repo();

    let applied_stacks = vb_state
        .list_stacks_in_workspace()
        .context("failed to read virtual branches")?;

    if !applied_stacks.iter().any(|b| b.id == target_stack_id) {
        bail!("Destination branch not found");
    }

    let default_target = vb_state.get_default_target()?;
    let default_target_commit = repo.find_commit(default_target.sha)?;

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
        default_target_commit,
        &mut source_stack,
        subject_commit,
        &workspace_dependencies,
    )?;

    move_commit_to_destination_stack(ctx, repo, destination_stack, subject_commit_oid)?;

    checkout_branch_trees(ctx, perm)?;
    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

fn get_source_branch_diffs(
    ctx: &CommandContext,
    source_stack: &gitbutler_stack::Stack,
) -> Result<BranchStatus> {
    let repo = ctx.repo();
    let source_stack_head = repo.find_commit(source_stack.head())?;
    let source_stack_head_tree = source_stack_head.tree()?;
    let uncommitted_changes_tree = repo.find_tree(source_stack.tree)?;

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
    default_target_commit: git2::Commit<'_>,
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

    let source_merge_base_oid = repo.merge_base(default_target_commit.id(), source_stack.head())?;
    let source_commits_without_subject =
        filter_out_commit(repo, source_stack, source_merge_base_oid, &subject_commit)?;

    let new_source_head = cherry_rebase_group(
        repo,
        source_merge_base_oid,
        &source_commits_without_subject,
        false,
    )?;

    let BranchHeadAndTree {
        head: new_head_oid,
        tree: new_tree_oid,
    } = compute_updated_branch_head(repo, source_stack, new_source_head)?;

    let subject_parent = subject_commit.parent(0)?;
    source_stack.replace_head(ctx, &subject_commit, &subject_parent)?;
    source_stack.set_stack_head(ctx, new_head_oid, Some(new_tree_oid))?;
    Ok(())
}

/// Move the commit to the destination stack.
fn move_commit_to_destination_stack(
    ctx: &CommandContext,
    repo: &git2::Repository,
    mut destination_stack: gitbutler_stack::Stack,
    commit_id: git2::Oid,
) -> Result<(), anyhow::Error> {
    let destination_head_commit_oid = destination_stack.head();
    let new_destination_head_oid =
        cherry_rebase_group(repo, destination_head_commit_oid, &[commit_id], false)?;

    let BranchHeadAndTree {
        head: new_destination_head_oid,
        tree: new_destination_tree_oid,
    } = compute_updated_branch_head(repo, &destination_stack, new_destination_head_oid)?;

    destination_stack.set_stack_head(
        ctx,
        new_destination_head_oid,
        Some(new_destination_tree_oid),
    )?;
    Ok(())
}

struct FilterOutCommitResult {
    found: bool,
    source_commits_without_subject: Vec<git2::Oid>,
}

/// Filter out the commit from the source stack.
///
/// Will fail if the commit is not in the source stack.
fn filter_out_commit(
    repo: &git2::Repository,
    source_stack: &gitbutler_stack::Stack,
    source_merge_base_oid: git2::Oid,
    subject_commit: &git2::Commit<'_>,
) -> Result<Vec<git2::Oid>, anyhow::Error> {
    let FilterOutCommitResult {
        found,
        source_commits_without_subject,
    } = repo
        .log(
            source_stack.head(),
            LogUntil::Commit(source_merge_base_oid),
            false,
        )?
        .iter()
        .fold(
            FilterOutCommitResult {
                found: false,
                source_commits_without_subject: vec![],
            },
            |result, c| {
                if c.id() == subject_commit.id() {
                    FilterOutCommitResult {
                        found: true,
                        ..result
                    }
                } else {
                    let mut source_commits_without_subject = result.source_commits_without_subject;
                    source_commits_without_subject.push(c.id());
                    FilterOutCommitResult {
                        source_commits_without_subject,
                        ..result
                    }
                }
            },
        );

    if !found {
        return Err(anyhow!("Commit not found in source stack"));
    }
    Ok(source_commits_without_subject)
}
