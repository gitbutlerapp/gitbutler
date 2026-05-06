use anyhow::{Context as _, Result};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::{
    OplogExt, SnapshotExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_reference::normalize_branch_name;
use gitbutler_stack::{PatchReferenceUpdate, Stack, StackBranch};
use serde::{Deserialize, Serialize};

use crate::{VirtualBranchesExt, actions::Verify};

/// Return the legacy stack identified by `stack_id`.
///
/// This keeps legacy virtual-branches access encapsulated within
/// `gitbutler-branch-actions` for callers that still operate on
/// `gitbutler_stack::Stack`.
pub fn get_stack(ctx: &Context, stack_id: StackId) -> Result<Stack> {
    ctx.virtual_branches().get_stack(stack_id)
}

/// Adds a new "series/branch" to the Stack.
/// This is in fact just creating a new  GitButler patch reference (head) and associates it with the stack.
/// The name cannot be the same as existing git references or existing patch references.
/// The target must reference a commit (or change) that is part of the stack.
/// The branch name must be a valid reference name (i.e. can not contain spaces, special characters etc.)
///
/// When creating heads, it is possible to have multiple heads that point to the same patch/commit.
/// If this is the case, the order can be disambiguated by specifying the `preceding_head`.
/// If there are multiple heads pointing to the same patch and `preceding_head` is not specified,
/// that means the new head will be first in order for that patch.
/// The argument `preceding_head` is only used if there are multiple heads that point to the same patch, otherwise it is ignored.
pub fn create_branch(ctx: &mut Context, stack_id: StackId, req: CreateSeriesRequest) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.snapshot_create_dependent_branch(&req.name, guard.write_permission());
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Requires an open workspace mode")?;
    let mut stack = ctx.virtual_branches().get_stack(stack_id)?;
    let normalized_head_name = normalize_branch_name(&req.name)?;
    let repo = ctx.repo.get()?;
    // If target_patch is None, create a new head that points to the top of the stack (most recent patch)
    if let Some(target_patch) = req.target_patch {
        let target_oid = gix::ObjectId::from_hex(target_patch.as_bytes())?;
        stack.add_series(
            ctx,
            StackBranch::new(target_oid, normalized_head_name, &repo)?,
            req.preceding_head,
        )
    } else {
        stack.add_series_top_of_stack(ctx, normalized_head_name)
    }
}

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
/// Same invariants as `create_branch` apply.
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
    stack.update_branch(
        ctx,
        branch_name,
        &PatchReferenceUpdate {
            name: Some(normalized_head_name.clone()),
        },
    )?;
    Ok(normalized_head_name)
}

/// Sets the forge identifier for a given series/branch. Existing value is overwritten.
///
/// # Errors
/// This method will return an error if:
///  - The series does not exist
///  - The stack can't be found
///  - The stack has not been initialized
///  - The project is not in workspace mode
///  - Persisting the changes failed
pub fn update_branch_pr_number(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateDependentBranchPrNumber),
        guard.write_permission(),
    );
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Requires an open workspace mode")?;
    let mut stack = ctx.virtual_branches().get_stack(stack_id)?;
    stack.set_pr_number(ctx, &branch_name, pr_number)
}
