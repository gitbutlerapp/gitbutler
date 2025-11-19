use anyhow::{Context as _, Ok, Result, bail};
use but_ctx::Context;
use but_ctx::access::WorktreeWritePermission;
use but_oxidize::{ObjectIdExt, OidExt};
use but_rebase::RebaseStep;
use gitbutler_commit::{commit_ext::CommitExt, commit_headers::HasCommitHeaders};
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_repo::{
    RepositoryExt as _,
    logging::{LogUntil, RepositoryExt},
};
use gitbutler_stack::StackId;
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommited_changes};
use itertools::Itertools;

use crate::{
    VirtualBranchesExt,
    reorder::{commits_order, reorder_stack},
};

/// Squashes one or multiple commuits from a virtual branch into a destination commit
/// All of the commits involved have to be in the same stack
pub(crate) fn squash_commits(
    ctx: &Context,
    stack_id: StackId,
    source_ids: Vec<git2::Oid>,
    desitnation_id: git2::Oid,
    perm: &mut WorktreeWritePermission,
) -> Result<git2::Oid> {
    // create a snapshot
    let snap = ctx.create_snapshot(SnapshotDetails::new(OperationKind::SquashCommit), perm)?;
    let result = do_squash_commits(ctx, stack_id, source_ids, desitnation_id, perm);
    // if result is error, restore from snapshot
    if result.is_err() {
        ctx.restore_snapshot(snap, perm)?;
    }
    result
}

/// Squashes one or multiple commits from a virtual branch into a destination commit.
///
/// The steps to accomplish this are:
/// 1. Reorder the commits so that the source commits and the destination commits are consecutively together but keeping the parentage.
/// - All source commits that come before the destination commit, stay before it. All source commits that come after the destination commit, stay after it.
///
/// 2. Once you have a consecutive list of commits to squash (source commits and destination commit), validate their state.
/// - If there were any conflicts as a result of the reordering, this will fail.
///
/// 3. Take the tree of the child most source commit (the most recent change) and use that for the new commit.
/// - By definition, the tree of the top commit includes all changes from the previous commits.
///
/// 4. Take the parent most commit from the source commits and destination commit, and use that as the squash target.
/// - This ensures that squashing parents into children works as expected.
fn do_squash_commits(
    ctx: &Context,
    stack_id: StackId,
    mut source_ids: Vec<git2::Oid>,
    destination_id: git2::Oid,
    perm: &mut WorktreeWritePermission,
) -> Result<git2::Oid> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let vb_state = ctx.legacy_project.virtual_branches();
    let stack = vb_state.get_stack_in_workspace(stack_id)?;
    let gix_repo = ctx.repo.get()?;

    let default_target = vb_state.get_default_target()?;
    let repo = ctx.git2_repo.get()?;
    let merge_base = repo.merge_base(stack.head_oid(&gix_repo)?.to_git2(), default_target.sha)?;

    // =========== Step 1: Reorder

    let order = commits_order(ctx, &stack)?;
    let mut updated_order = commits_order(ctx, &stack)?;

    // Source commits incude the destination commit
    let mut source_ids_in_order = Vec::new();
    // Remove source ids
    for branch in updated_order.series.iter_mut() {
        branch.commit_ids.retain(|id| {
            match id {
                id if source_ids.contains(id) => {
                    // Add the source ids in the order they appear in the branch
                    source_ids_in_order.push(*id);
                    false
                }
                id if *id == destination_id => {
                    // Add the destination id to the source ids in order
                    source_ids_in_order.push(*id);
                    true
                }
                _ => true,
            }
        });
    }

    // Keep the actual order of the source ids
    source_ids = source_ids_in_order;

    // Replace the destination commit with the ordered, consecutive list of source commits (including the destination commit)
    for branch in updated_order.series.iter_mut() {
        if let Some(pos) = branch
            .commit_ids
            .iter()
            .position(|&id| id == destination_id)
        {
            branch.commit_ids.splice(pos..=pos, source_ids.clone());
        }
    }

    let mapping = if order != updated_order {
        Some(reorder_stack(ctx, stack_id, updated_order, perm)?.commit_mapping)
    } else {
        None
    };

    let mut destination_id = destination_id;

    // update source ids from the mapping if present
    if let Some(mapping) = mapping {
        for (_, old, new) in mapping.iter() {
            // if source_ids contains old, replace it with new
            if source_ids.contains(&old.to_git2()) {
                let index = source_ids
                    .iter()
                    .position(|id| id == &old.to_git2())
                    .unwrap();
                source_ids[index] = new.to_git2();
            }

            // if destination_id is old, replace it with new
            if destination_id == old.to_git2() {
                destination_id = new.to_git2();
            }
        }
    };

    // =========== Step 2: Squash

    // stack was updated by reorder_stack, therefore it is reloaded
    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let branch_commit_oids = repo.l(
        stack.head_oid(&gix_repo)?.to_git2(),
        LogUntil::Commit(merge_base),
        false,
    )?;

    let branch_commits = branch_commit_oids
        .iter()
        .filter_map(|id| repo.find_commit(*id).ok())
        .collect_vec();

    // Find the new destination commit using the change id, error if not found
    let destination_commit = branch_commits
        .iter()
        .find(|c| c.id() == destination_id)
        .context("Destination commit not found in the stack")?;

    // Find the new source commits using the change ids, error if not found
    let source_commits = source_ids
        .iter()
        .filter_map(|id| repo.find_commit(*id).ok())
        .collect::<Vec<_>>();

    validate(
        ctx,
        &stack,
        &branch_commit_oids,
        &source_commits,
        destination_commit,
    )?;

    // Having all the source commits in in the right order, sitting directly on top of the destination commit
    // means that we just need to take the tree of the child most source commit (most recent change) and
    // use that for the new commit.
    // By definition, the tree of the top commit includes all changes from the previous commits.
    let child_most_source_commit = source_commits
        .first()
        .context("No source commits provided")?;
    let final_tree = child_most_source_commit
        .tree()
        .context("Failed to get tree of the child most source commit")?;

    // The parent most commit from the source commits is used as the squash target.
    let parent_most_source_commit = source_commits
        .last()
        .context("No source commits provided")?;

    let source_commits_without_destination = source_commits
        .iter()
        .filter(|&commit| commit.id() != destination_commit.id());
    let gerrit_mode = but_core::RepositoryExt::git_settings(&*ctx.repo.get()?)?
        .gitbutler_gerrit_mode
        .unwrap_or(false);

    // Squash commit messages string separating with newlines
    let new_message = Some(destination_commit)
        .into_iter()
        .chain(source_commits_without_destination)
        .filter_map(|c| {
            let msg = c.message().unwrap_or_default();
            let msg = if gerrit_mode {
                // Remove lines containing Change-Id: <hash> only if gerrit mode is enabled
                msg.lines()
                    .filter(|line| !line.trim_start().starts_with("Change-Id: I"))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                msg.to_string()
            };
            (!msg.trim().is_empty()).then_some(msg)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let parents: Vec<_> = parent_most_source_commit.parents().collect();

    // Create a new commit with the final tree
    let new_commit_oid = repo
        .commit_with_signature(
            None,
            &destination_commit.author(),
            &destination_commit.author(),
            &new_message,
            &final_tree,
            &parents.iter().collect::<Vec<_>>(),
            destination_commit.gitbutler_headers(),
        )
        .context("Failed to create a squash commit")?;

    let mut steps: Vec<RebaseStep> = Vec::new();

    for head in stack.heads_by_commit(repo.find_commit(merge_base)?, &gix_repo) {
        steps.push(RebaseStep::Reference(but_core::Reference::Virtual(head)));
    }
    for oid in branch_commit_oids.iter().rev() {
        let commit = repo.find_commit(*oid)?;
        if parent_most_source_commit.id() == *oid {
            steps.push(RebaseStep::Pick {
                commit_id: new_commit_oid.to_gix(),
                new_message: None,
            });
        } else if source_ids.contains(oid) {
            // noop - skipping this
        } else {
            steps.push(RebaseStep::Pick {
                commit_id: oid.to_gix(),
                new_message: None,
            });
        }
        for head in stack.heads_by_commit(commit, &gix_repo) {
            steps.push(RebaseStep::Reference(but_core::Reference::Virtual(head)));
        }
    }

    let mut builder = but_rebase::Rebase::new(&gix_repo, merge_base.to_gix(), None)?;
    let builder = builder.steps(steps)?;
    builder.rebase_noops(false);
    let output = builder.rebase()?;

    let new_stack_head = output.top_commit.to_git2();

    stack.set_stack_head(&vb_state, &gix_repo, new_stack_head, None)?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    update_uncommited_changes(ctx, old_workspace, new_workspace, perm)?;
    crate::integration::update_workspace_commit(&vb_state, ctx, false)
        .context("failed to update gitbutler workspace")?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;
    Ok(new_commit_oid)
}

fn validate(
    ctx: &Context,
    stack: &gitbutler_stack::Stack,
    branch_commit_oids: &[git2::Oid],
    commits_to_squash_together: &[git2::Commit<'_>],
    destination_commit: &git2::Commit<'_>,
) -> Result<()> {
    for source_commit in commits_to_squash_together {
        if !branch_commit_oids.contains(&source_commit.id()) {
            bail!("commit {} not in the stack", source_commit.id());
        }
    }

    if !branch_commit_oids.contains(&destination_commit.id()) {
        bail!("commit {} not in the stack", destination_commit.id());
    }

    for c in commits_to_squash_together {
        if c.is_conflicted() {
            bail!("cannot squash conflicted source commit {}", c.id());
        }
    }

    if destination_commit.is_conflicted() {
        bail!("cannot squash into conflicted destination commit",);
    }

    let git2_repo = &*ctx.git2_repo.get()?;
    let remote_commits = stack
        .branches()
        .iter()
        .flat_map(|b| b.commits(git2_repo, &ctx.legacy_project, stack))
        .flat_map(|c| c.remote_commits)
        .map(|c| c.id())
        .collect_vec();

    if !stack.allow_rebasing {
        for source_commit in commits_to_squash_together {
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
