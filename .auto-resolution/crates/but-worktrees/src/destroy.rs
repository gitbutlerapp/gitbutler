use anyhow::Result;
use but_ctx::{Context, access::WorktreeWritePermission};
use serde::Serialize;

use crate::{WorktreeId, git::git_worktree_remove, list::worktree_list};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct DestroyWorktreeOutcome {
    pub destroyed_ids: Vec<WorktreeId>,
}

/// Destroys a worktree by its ID.
pub fn worktree_destroy_by_id(
    ctx: &mut Context,
    _perm: &WorktreeWritePermission,
    id: &WorktreeId,
) -> Result<DestroyWorktreeOutcome> {
    // Remove the git worktree (force=true to handle uncommitted changes)
    git_worktree_remove(&ctx.legacy_project.common_git_dir()?, id, true)?;

    Ok(DestroyWorktreeOutcome {
        destroyed_ids: vec![id.clone()],
    })
}

/// Destroys all worktrees created from a given reference.
pub fn worktree_destroy_by_reference(
    ctx: &mut Context,
    perm: &WorktreeWritePermission,
    reference: &gix::refs::FullNameRef,
) -> Result<DestroyWorktreeOutcome> {
    // Use the existing list function to get all worktrees
    let list_outcome = worktree_list(ctx, perm.read_permission())?;

    // Filter for worktrees created from the specified reference
    let worktrees_to_destroy: Vec<_> = list_outcome
        .entries
        .into_iter()
        .filter(|w| {
            w.created_from_ref
                .as_ref()
                .map(|r| r.as_ref() == reference)
                .unwrap_or(false)
        })
        .collect();

    let mut destroyed_ids = Vec::new();

    // Destroy each matching worktree
    for worktree in worktrees_to_destroy {
        // Remove the git worktree (force=true to handle uncommitted changes)
        git_worktree_remove(&ctx.legacy_project.common_git_dir()?, &worktree.id, true)?;

        destroyed_ids.push(worktree.id);
    }

    Ok(DestroyWorktreeOutcome { destroyed_ids })
}
