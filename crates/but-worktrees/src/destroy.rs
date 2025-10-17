use std::path::Path;

use anyhow::{Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use serde::Serialize;

use crate::{git::git_worktree_remove, list::worktree_list};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct DestroyWorktreeOutcome {
    pub destroyed_paths: Vec<std::path::PathBuf>,
}

/// Destroys a worktree by its path.
pub fn worktree_destroy_by_path(
    ctx: &mut CommandContext,
    _perm: &WorktreeWritePermission,
    path: &Path,
) -> Result<DestroyWorktreeOutcome> {
    // Canonicalize the path to match what's in the database
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?;

    // Remove the git worktree (force=true to handle uncommitted changes)
    git_worktree_remove(&ctx.project().path, &canonical_path, true).with_context(|| {
        format!(
            "Failed to remove git worktree at {}",
            canonical_path.display()
        )
    })?;

    Ok(DestroyWorktreeOutcome {
        destroyed_paths: vec![canonical_path],
    })
}

/// Destroys all worktrees created from a given reference.
pub fn worktree_destroy_by_reference(
    ctx: &mut CommandContext,
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

    let mut destroyed_paths = Vec::new();

    // Destroy each matching worktree
    for worktree in worktrees_to_destroy {
        // Remove the git worktree (force=true to handle uncommitted changes)
        git_worktree_remove(&ctx.project().path, &worktree.path, true).with_context(|| {
            format!(
                "Failed to remove git worktree at {}",
                worktree.path.display()
            )
        })?;

        destroyed_paths.push(worktree.path);
    }

    Ok(DestroyWorktreeOutcome { destroyed_paths })
}
