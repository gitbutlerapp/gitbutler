use anyhow::{bail, Context, Ok, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{commit_ext::CommitExt, commit_headers::HasCommitHeaders};
use gitbutler_oxidize::{git2_to_gix_object_id, gix_to_git2_oid, GixRepositoryExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{
    logging::{LogUntil, RepositoryExt},
    rebase::cherry_rebase_group,
    RepositoryExt as _,
};
use gitbutler_stack::{stack_context::CommandContextExt, StackId};
use gitbutler_workspace::{compute_updated_branch_head, BranchHeadAndTree};
use itertools::Itertools;

use crate::VirtualBranchesExt;

/// Squashes one or multiple commuits from a virtual branch into a destination commit
/// All of the commits involved have to be in the same stack
pub(crate) fn squash_commits(
    ctx: &CommandContext,
    stack_id: StackId,
    source_ids: Vec<git2::Oid>,
    desitnation_id: git2::Oid,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let vb_state = ctx.project().virtual_branches();
    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let default_target = vb_state.get_default_target()?;
    let branch_commit_oids =
        ctx.repo()
            .l(stack.head(), LogUntil::Commit(default_target.sha), false)?;

    let source_commits: Vec<git2::Commit> = source_ids
        .iter()
        .filter_map(|id| ctx.repo().find_commit(*id).ok())
        .collect();

    let destination_commit = ctx.repo().find_commit(desitnation_id)?;

    validate(
        ctx,
        &stack,
        &branch_commit_oids,
        &source_commits,
        &destination_commit,
    )?;

    // Create a new commit that that has the source trees merged into the target tree
    let base_tree = git2_to_gix_object_id(destination_commit.tree_id());
    let mut final_tree_id = git2_to_gix_object_id(destination_commit.tree_id());
    let gix_repo = ctx.gix_repository_for_merging()?;
    let (merge_options_fail_fast, conflict_kind) = gix_repo.merge_options_fail_fast()?;
    for source_commit in &source_commits {
        let source_tree = git2_to_gix_object_id(source_commit.tree_id());
        let mut merge = gix_repo.merge_trees(
            base_tree,
            final_tree_id,
            source_tree,
            gix_repo.default_merge_labels(),
            merge_options_fail_fast.clone(),
        )?;
        if merge.has_unresolved_conflicts(conflict_kind) {
            bail!("Merge failed with conflicts");
        }
        final_tree_id = merge.tree.write()?.detach();
    }
    let final_tree = ctx.repo().find_tree(gix_to_git2_oid(final_tree_id))?;
    // Squash commit messages string separated by newlines
    let source_messages = source_commits
        .iter()
        .map(|c| c.message().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    let parents: Vec<_> = destination_commit.parents().collect();

    // Create a new commit with the final tree
    let new_commit_oid = ctx
        .repo()
        .commit_with_signature(
            None,
            &destination_commit.author(),
            &destination_commit.author(),
            &format!("{}\n{}", destination_commit.message_bstr(), source_messages),
            &final_tree,
            &parents.iter().collect::<Vec<_>>(),
            destination_commit.gitbutler_headers(),
        )
        .context("Failed to create a squash commit")?;

    // ids_to_rebase is the list the original list of commit ids (branch_commit_ids) with the source commits removed and the
    // destination commit replaced with the new commit
    let ids_to_rebase = branch_commit_oids
        .iter()
        .filter(|id| !source_ids.contains(id))
        .map(|id| {
            if *id == destination_commit.id() {
                new_commit_oid
            } else {
                *id
            }
        })
        .collect::<Vec<_>>();

    let merge_base = ctx
        .repo()
        .merge_base(destination_commit.id(), default_target.sha)?;

    // Rebase the commits in the stack so that the source commits are removed and the destination commit
    // is replaces with a new commit with the final tree that is the result of the merge
    let new_stack_head = cherry_rebase_group(ctx.repo(), merge_base, &ids_to_rebase, false)?;

    let BranchHeadAndTree {
        head: new_head_oid,
        tree: new_tree_oid,
    } = compute_updated_branch_head(ctx.repo(), &stack, new_stack_head)?;

    let new_destination_commit = ctx.repo().find_commit(new_head_oid)?;

    // Update stack heads, starting with the desitnation commit
    // If the destination commit happens to be a head, update the head to the new commit
    stack.replace_head(ctx, &destination_commit, &new_destination_commit)?;
    // Go over the source commits and for each one, if it happens to be a head, update the head to be the first parent of the commit
    // as long as the parent iself is not in the list of source commits, otherwise do nothing
    for source_commit in &source_commits {
        let parent = source_commit.parent(0)?;
        if !source_commits.iter().any(|c| c.id() == parent.id()) {
            stack.replace_head(ctx, source_commit, &parent)?;
        }
    }
    // Finally, update the stack head
    stack.set_stack_head(ctx, new_head_oid, Some(new_tree_oid))?;

    Ok(())
}

fn validate(
    ctx: &CommandContext,
    stack: &gitbutler_stack::Stack,
    branch_commit_oids: &[git2::Oid],
    source_commits: &[git2::Commit<'_>],
    destination_commit: &git2::Commit<'_>,
) -> Result<()> {
    for source_commit in source_commits {
        if !branch_commit_oids.contains(&source_commit.id()) {
            bail!("commit {} not in the stack", source_commit.id());
        }
    }

    if !branch_commit_oids.contains(&destination_commit.id()) {
        bail!("commit {} not in the stack", destination_commit.id());
    }

    for c in source_commits {
        if c.is_conflicted() {
            bail!("cannot squash conflicted source commit {}", c.id());
        }
    }

    if destination_commit.is_conflicted() {
        bail!(
            "cannot squash into conflicted destination commit {}",
            destination_commit.id()
        );
    }

    let stack_ctx = ctx.to_stack_context()?;
    let remote_commits = stack
        .branches()
        .iter()
        .flat_map(|b| b.commits(&stack_ctx, stack))
        .flat_map(|c| c.remote_commits)
        .map(|c| c.id())
        .collect_vec();

    if !stack.allow_rebasing {
        for source_commit in source_commits {
            if remote_commits.contains(&source_commit.id()) {
                bail!(
                    "Force push is now allowed. Source commits with id {} has already been pushed",
                    source_commit.id()
                );
            }
        }
        if remote_commits.contains(&destination_commit.id()) {
            bail!(
                "Force push is now allowed. Destination commit with id {} has already been pushed",
                destination_commit.id()
            );
        }
    }

    Ok(())
}
