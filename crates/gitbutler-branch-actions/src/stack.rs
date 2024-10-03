use anyhow::{Context, Result};
use gitbutler_branch::BranchId;
use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};
use gitbutler_project::Project;
use gitbutler_stack::{PatchReferenceUpdate, Stack};
use serde::{Deserialize, Serialize};

use crate::{actions::open_with_verify, VirtualBranchesExt};
use gitbutler_operating_modes::assure_open_workspace_mode;

/// Adds a new "series/branch" to the Stack.
/// This is in fact just creating a new  GitButler patch reference (head) and associates it with the stack.
/// The name cannot be the same as existing git references or existing patch references.
/// The target must reference a commit (or change) that is part of the stack.
/// The branch name must be a valid reference name (i.e. can not contain spaces, special characters etc.)
///
/// When creating heads, it is possible to have multiple heads that point to the same patch/commit.
/// If this is the case, the order can be disambiguated by specifying the `preceding_head`.
/// If there are multiple heads pointing to the same patch and `preceding_head` is not spcified,
/// that means the new head will be first in order for that patch.
/// The argument `preceding_head` is only used if there are multiple heads that point to the same patch, otherwise it is ignored.
pub fn create_series(
    project: &Project,
    branch_id: BranchId,
    req: CreateSeriesRequest,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    // If target_patch is None, create a new head that points to the top of the stack (most recent patch)
    if let Some(target_patch) = req.target_patch {
        stack.add_series(
            ctx,
            PatchReference {
                target: target_patch,
                name: req.name,
                description: req.description,
            },
            req.preceding_head,
        )
    } else {
        stack.add_series_top_of_stack(ctx, req.name, req.description)
    }
}

/// Request to create a new series in a stack
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateSeriesRequest {
    /// Name of the new series
    name: String,
    /// Description of the new series - can be markdown or antyhing really
    description: Option<String>,
    /// The target patch (head) to create these series for. If let None, the new series will be at the top of the stack
    target_patch: Option<CommitOrChangeId>,
    /// The name of the series that preceed the newly created series.
    /// This is used to disambiguate the order whne they point to the same patch
    // TODO: the API can be furthe simplified name here should be sufficient
    preceding_head: Option<PatchReference>,
}

/// Removes series grouping from the Stack. This will not touch the patches / commits contained in the series.
/// The very last branch (reference) cannot be removed (A Stack must always contains at least one reference)
/// If there were commits/changes that were *only* referenced by the removed branch,
/// those commits are moved to the branch underneath it (or more accurately, the precee)
pub fn remove_series(project: &Project, branch_id: BranchId, head_name: String) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    stack.remove_series(ctx, head_name)
}

/// Updates the name an existing series in the stack.
/// Same invariants as `create_series` apply.
/// If the series have been pushed to a remote, the name can not be changed as it corresponds to a remote ref.
pub fn update_series_name(
    project: &Project,
    branch_id: BranchId,
    head_name: String,
    new_head_name: String,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    stack.update_series(
        ctx,
        head_name,
        &PatchReferenceUpdate {
            name: Some(new_head_name),
            ..Default::default()
        },
    )
}

/// Updates the description of an existing series in the stack.
/// The description can be set to `None` to remove it.
pub fn update_series_description(
    project: &Project,
    branch_id: BranchId,
    head_name: String,
    description: Option<String>,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    stack.update_series(
        ctx,
        head_name,
        &PatchReferenceUpdate {
            description: Some(description),
            ..Default::default()
        },
    )
}

/// Pushes all series in the stack to the remote.
/// This operation will error out if the target has no push remote configured.
pub fn push_stack(project: &Project, branch_id: BranchId, with_force: bool) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    let stack_series = stack.list_series(ctx)?;
    for series in stack_series {
        stack.push_series(ctx, series.head.name, with_force)?;
    }
    Ok(())
}
