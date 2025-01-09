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
use gitbutler_workspace::{checkout_branch_trees, compute_updated_branch_head, BranchHeadAndTree};
use itertools::Itertools;

use crate::{commit_ops::get_exclusive_tree, VirtualBranchesExt};

/// Squashes one or multiple commuits from a virtual branch into a destination commit
/// All of the commits involved have to be in the same stack
pub(crate) fn squash_commits(
    ctx: &CommandContext,
    stack_id: StackId,
    source_ids: Vec<git2::Oid>,
    desitnation_id: git2::Oid,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let vb_state = ctx.project().virtual_branches();
    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let default_target = vb_state.get_default_target()?;
    let merge_base = ctx.repo().merge_base(stack.head(), default_target.sha)?;

    let branch_commit_oids = ctx
        .repo()
        .l(stack.head(), LogUntil::Commit(merge_base), false)?;

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

    let final_tree = squash_tree(ctx, &source_commits, &destination_commit, merge_base)?;
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

    // Rebase the commits in the stack so that the source commits are removed and the destination commit
    // is replaces with a new commit with the final tree that is the result of the merge
    let new_stack_head = cherry_rebase_group(ctx.repo(), merge_base, &ids_to_rebase, false)?;

    let BranchHeadAndTree {
        head: new_head_oid,
        tree: new_tree_oid,
    } = compute_updated_branch_head(ctx.repo(), &stack, new_stack_head)?;

    stack.set_stack_head(ctx, new_head_oid, Some(new_tree_oid))?;

    checkout_branch_trees(ctx, perm)?;
    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    // Finally, update branch heads in the stack if present
    for source_commit in &source_commits {
        // Find the next eligible ancestor commit that is not in the source commits
        let mut ancestor = source_commit.parent(0)?;
        while source_commits.iter().any(|c| c.id() == ancestor.id()) {
            if ancestor.id() == merge_base {
                break; // Don's search past the merge base
            }
            ancestor = ancestor.parent(0)?;
        }
        stack.replace_head(ctx, source_commit, &ancestor)?;
    }
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

// Create a new tree that that has the source trees merged into the target tree
fn squash_tree<'a>(
    ctx: &'a CommandContext,
    source_commits: &[git2::Commit<'_>],
    destination_commit: &git2::Commit<'_>,
    merge_base: git2::Oid,
) -> Result<git2::Tree<'a>> {
    let base_tree = git2_to_gix_object_id(destination_commit.tree_id());
    let mut final_tree_id = git2_to_gix_object_id(destination_commit.tree_id());
    let gix_repo = ctx.gix_repository_for_merging()?;
    let (merge_options_fail_fast, conflict_kind) = gix_repo.merge_options_fail_fast()?;
    for source_commit in source_commits {
        let source_tree = get_exclusive_tree(
            &gix_repo,
            git2_to_gix_object_id(source_commit.id()),
            git2_to_gix_object_id(merge_base),
        )?;
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
    Ok(final_tree)
}
