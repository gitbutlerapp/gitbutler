use anyhow::{Context as _, Result};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::SnapshotExt;
use gitbutler_reference::normalize_branch_name;
use serde::{Deserialize, Serialize};

use crate::{VirtualBranchesExt, actions::Verify};

/// Request to create a new series in a stack
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateSeriesRequest {
    /// Name of the new series
    pub name: String,
    /// The target patch (head) to create these series for. If let None, the new series will be at the top of the stack
    pub target_patch: Option<String>,
    /// The name of the series that preceded the newly created series.
    /// This is used to disambiguate the order when they point to the same patch
    pub preceding_head: Option<String>,
}

/// Updates the name an existing branch and resets the pr_number to None.
///
/// Returns the new normalized name of the branch.
pub fn update_branch_name(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<String> {
    let mut guard = ctx.exclusive_worktree_access();
    update_branch_name_with_perm(
        ctx,
        stack_id,
        branch_name,
        new_name,
        guard.write_permission(),
    )
}

pub fn update_branch_name_with_perm(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
    perm: &mut but_core::sync::RepoExclusive,
) -> Result<String> {
    ctx.verify(perm)?;
    let _ = ctx.snapshot_update_dependent_branch_name(&branch_name, perm);
    ensure_open_workspace_mode(ctx, perm.read_permission())
        .context("Requires an open workspace mode")?;
    let mut stack = ctx.virtual_branches().get_stack(stack_id)?;
    let normalized_head_name = normalize_branch_name(&new_name)?;
    stack.rename_branch(ctx, branch_name, normalized_head_name.clone())?;
    Ok(normalized_head_name)
}
