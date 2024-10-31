use anyhow::{anyhow, bail};
use anyhow::{Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::rebase::cherry_rebase_group;
use gitbutler_repo::{LogUntil, RepositoryExt};
use gitbutler_stack::StackId;

use crate::branch_trees::{checkout_branch_trees, compute_updated_branch_head, BranchHeadAndTree};
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
    let repo = ctx.repository();

    let applied_branches = vb_state
        .list_branches_in_workspace()
        .context("failed to read virtual branches")?;

    if !applied_branches.iter().any(|b| b.id == target_stack_id) {
        bail!("branch {target_stack_id} is not among applied branches")
    }

    let default_target = vb_state.get_default_target()?;
    let default_target_commit = repo.find_commit(default_target.sha)?;

    let source_stack = vb_state
        .try_branch(source_stack_id)?
        .ok_or(anyhow!("Source stack not found"))?;

    let destination_stack = vb_state
        .try_branch(target_stack_id)?
        .ok_or(anyhow!("Destination branch not found"))?;

    let subject_commit = repo
        .find_commit(subject_commit_oid)
        .with_context(|| format!("commit {subject_commit_oid} to be moved could not be found"))?;

    take_commit_from_source_stack(
        ctx,
        repo,
        default_target_commit,
        source_stack,
        subject_commit,
    )?;

    move_commit_to_destination_stack(ctx, repo, destination_stack, subject_commit_oid)?;

    checkout_branch_trees(ctx, perm)?;
    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

/// Remove the commit from the source stack.
///
/// Will fail if the commit is not in the source stack or if has dependent changes.
fn take_commit_from_source_stack(
    ctx: &CommandContext,
    repo: &git2::Repository,
    default_target_commit: git2::Commit<'_>,
    mut source_stack: gitbutler_stack::Stack,
    subject_commit: git2::Commit<'_>,
) -> Result<(), anyhow::Error> {
    let commit_dependencies = &source_stack.commit_dependencies.get(&subject_commit.id());

    let inverse_commit_dependencies = &source_stack
        .inverse_commit_dependencies
        .get(&subject_commit.id());

    let commit_dependent_diffs = &source_stack
        .commit_dependent_diffs
        .get(&subject_commit.id());

    if let Some(commit_dependencies) = commit_dependencies {
        if !commit_dependencies.is_empty() {
            bail!("Commit depends on other changes");
        }
    }

    if let Some(inverse_commit_dependencies) = inverse_commit_dependencies {
        if !inverse_commit_dependencies.is_empty() {
            bail!("Commit has dependent changes");
        }
    }

    if let Some(commit_dependent_diffs) = commit_dependent_diffs {
        if !commit_dependent_diffs.is_empty() {
            bail!("Commit has dependent uncommitted changes");
        }
    }

    let source_merge_base_oid = repo.merge_base(default_target_commit.id(), source_stack.head())?;
    let source_commits_without_subject =
        filter_out_commit(repo, &source_stack, source_merge_base_oid, &subject_commit)?;

    let new_source_head =
        cherry_rebase_group(repo, source_merge_base_oid, &source_commits_without_subject)?;

    let BranchHeadAndTree {
        head: new_head_oid,
        tree: new_tree_oid,
    } = compute_updated_branch_head(repo, &source_stack, new_source_head)?;

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
        cherry_rebase_group(repo, destination_head_commit_oid, &[commit_id])?;

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
