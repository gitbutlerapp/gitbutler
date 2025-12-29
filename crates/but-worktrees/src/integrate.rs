use crate::{WorktreeId, db::get_worktree_meta, git::git_worktree_remove};
use anyhow::{Context as _, Result, bail};
use bstr::{BString, ByteSlice};
use but_core::RepositoryExt;
use but_ctx::{
    Context,
    access::{WorktreeReadPermission, WorktreeWritePermission},
};
use but_oxidize::ObjectIdExt;
use but_rebase::{Rebase, RebaseOutput, RebaseStep};
use but_status::create_wd_tree;
use but_workspace::legacy::stack_ext::StackExt;
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{
    WorkspaceState, merge_workspace, move_tree, update_uncommitted_changes,
};
use gix::prelude::ObjectIdExt as _;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub enum WorktreeIntegrationStatus {
    NoMergeBaseFound,
    WorktreeIsBare,
    /// If we were to integrate this worktree back into the project, it would
    /// cause the workspace to conflict.
    ///
    /// If this is true, the worktree can't be integrated.
    CausesWorkspaceConflicts,
    Integratable {
        /// The cherry pick produced when integrating will be conflicted.
        cherry_pick_conflicts: bool,
        /// Commits above where this worktree will be cherry-picked are going to
        /// end up conflicted.
        commits_above_conflict: bool,
        /// Whether the uncommitted changes in the main checkout will end up
        /// conflicted
        working_dir_conflicts: bool,
    },
}

/// Determines whether a worktree is integrated
///
/// This function makes use of older APIs because there is not yet an
/// alternative to the rebase engine.
pub fn worktree_integration_status(
    ctx: &mut Context,
    perm: &WorktreeReadPermission,
    id: &WorktreeId,
    target: &gix::refs::FullNameRef,
) -> Result<WorktreeIntegrationStatus> {
    Ok(worktree_integration_inner(ctx, perm, id, target)?.0)
}

/// Integrates a worktree if it's integratable
///
/// This function makes use of older APIs because there is not yet an
/// alternative to the rebase engine.
pub fn worktree_integrate(
    ctx: &mut Context,
    perm: &mut WorktreeWritePermission,
    id: &WorktreeId,
    target: &gix::refs::FullNameRef,
) -> Result<()> {
    let before = WorkspaceState::create(ctx, perm.read_permission())?;

    let result = worktree_integration_inner(ctx, perm.read_permission(), id, target)?;
    let (WorktreeIntegrationStatus::Integratable { .. }, Some(mut status)) = result else {
        bail!("Worktree failed integration checks");
    };

    status
        .stack
        .set_heads_from_rebase_output(ctx, status.rebase_output.references)?;
    let after = WorkspaceState::create(ctx, perm.read_permission())?;
    update_uncommitted_changes(ctx, before, after, perm)?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    update_workspace_commit(&vb_state, ctx, false)?;

    git_worktree_remove(ctx.repo.get()?.common_dir(), id, true)?;

    Ok(())
}

struct IntegrationResult {
    rebase_output: RebaseOutput,
    stack: Stack,
}

/// Performs the workspace integration operations in memory, returning the
/// status, and output if it's integratable
fn worktree_integration_inner(
    ctx: &mut Context,
    perm: &WorktreeReadPermission,
    id: &WorktreeId,
    target: &gix::refs::FullNameRef,
) -> Result<(WorktreeIntegrationStatus, Option<IntegrationResult>)> {
    let repo = ctx.repo.get()?.clone().for_tree_diffing()?;

    let target_ref = repo.find_reference(target)?;

    let git_worktree = repo
        .worktrees()?
        .into_iter()
        .find(|w| w.id() == id.as_bstr())
        .expect("Health dictates this exists");
    let worktree_repo = git_worktree.into_repo()?;
    let worktree_head = worktree_repo.head()?;
    let Some(worktree_head_id) = worktree_head.id() else {
        return Ok((WorktreeIntegrationStatus::WorktreeIsBare, None));
    };

    // Find the base which we will use for the "cherry pick".
    let wt_meta = get_worktree_meta(&repo, id)?;
    let base = {
        // If we have worktree metadata and the base hasn't been dropped entirely
        // from history, we will use that.
        if let Some(wt_meta) = wt_meta
            && repo.find_object(wt_meta.base).is_ok()
        {
            wt_meta.base
        } else {
            let Ok(merge_base) = repo.merge_base(target_ref.id(), worktree_head_id) else {
                return Ok((WorktreeIntegrationStatus::NoMergeBaseFound, None));
            };
            merge_base.detach()
        }
    };

    // Create a squash commit which we will then cherry pick into the
    // target branch
    let wd_tree = create_wd_tree(&worktree_repo, 0)?;
    let author = repo
        .author()
        .transpose()
        .ok()
        .flatten()
        .context("Failed to find author signautre")?
        .to_owned()?;

    let commit = gix::objs::Commit {
        tree: wd_tree,
        parents: [base].into(),
        author: author.clone(),
        committer: author,
        encoding: None,
        message: BString::from("Integrated worktree"),
        extra_headers: vec![],
    };
    let commit_id = repo.write_object(commit)?;

    let vb_handle = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_handle.list_stacks_in_workspace()?;
    let stack = stacks
        .iter()
        .find(|s| {
            s.branches()
                .iter()
                .any(|b| b.name.as_bytes() == target.shorten())
        })
        .context("Failed to find branch in vb state")?
        .clone();

    let mut steps = stack.as_rebase_steps(ctx, &repo)?;
    let Some(to_insert_at) = steps.iter().enumerate().find_map(|(i, entry)| {
        if let RebaseStep::Reference(reference) = entry {
            let matches_target = match reference {
                but_core::Reference::Git(reference) => reference.as_ref() == target,
                but_core::Reference::Virtual(vref) => {
                    target.shorten() == BString::from(vref.clone()).as_bstr()
                }
            };

            if matches_target {
                return Some(i);
            }
        }

        None
    }) else {
        bail!("Failed to find point to insert at");
    };

    steps.insert(
        to_insert_at,
        RebaseStep::Pick {
            commit_id: commit_id.detach(),
            new_message: None,
        },
    );

    let mut rebase = Rebase::new(&repo, stack.merge_base(ctx)?, None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let output = rebase.rebase()?;

    // Does the new stack tip conflict with any of the other stacks.
    let tip_tree = repo.find_commit(output.top_commit)?.tree_id()?;
    for stack in stacks.iter().filter(|s| s.id != stack.id) {
        let head_id = stack.head_oid(&repo)?;
        let head_tree = repo.find_commit(head_id)?.tree_id()?;
        let merge_base = repo.merge_base(head_id, output.top_commit)?;
        let merge_base_tree = repo.find_commit(merge_base)?.tree_id()?;

        if !repo.merges_cleanly(
            merge_base_tree.detach(),
            head_tree.detach(),
            tip_tree.detach(),
        )? {
            return Ok((WorktreeIntegrationStatus::CausesWorkspaceConflicts, None));
        }
    }

    let cherry_pick_conflicts = {
        let Some(row) = output
            .commit_mapping
            .iter()
            .find(|m| m.1 == commit_id.detach())
        else {
            bail!("Cherry-pick did not end up in rebase output");
        };

        let commit = but_core::Commit::from_id(row.2.attach(&repo))?;
        commit.is_conflicted()
    };

    let commits_above_conflict = output
        .commit_mapping
        .iter()
        .filter(|r| r.1 != commit_id.detach())
        .any(|row| {
            if let Ok(commit) = but_core::Commit::from_id(row.2.attach(&repo)) {
                commit.is_conflicted()
            } else {
                false
            }
        });

    let wd_tree = create_wd_tree(&repo, 0)?;

    let working_dir_conflicts = {
        let before_heads = stacks
            .iter()
            .map(|s| s.head_oid(&repo))
            .collect::<Result<Vec<_>>>()?;
        let before = WorkspaceState::create_from_heads(ctx, perm, &before_heads)?;
        let before = merge_workspace(&*ctx.git2_repo.get()?, before)?;
        let mut after_heads = stacks
            .iter()
            .filter(|s| s.id != stack.id)
            .map(|s| s.head_oid(&repo))
            .collect::<Result<Vec<_>>>()?;
        after_heads.push(output.top_commit);
        let after = WorkspaceState::create_from_heads(ctx, perm, &after_heads)?;
        let after = merge_workspace(&*ctx.git2_repo.get()?, after)?;
        let index = move_tree(&*ctx.git2_repo.get()?, wd_tree.to_git2(), before, after)?;

        index.has_conflicts()
    };

    Ok((
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts,
            commits_above_conflict,
            working_dir_conflicts,
        },
        Some(IntegrationResult {
            rebase_output: output,
            stack,
        }),
    ))
}
