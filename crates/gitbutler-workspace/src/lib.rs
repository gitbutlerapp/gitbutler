pub mod branch_trees;

#[allow(deprecated)]
pub use branch_trees::{
    checkout_branch_trees, compute_updated_branch_head, compute_updated_branch_head_for_commits,
    BranchHeadAndTree,
};

use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::OidExt;
use gitbutler_project::access::WorktreeReadPermission;
use gitbutler_stack::VirtualBranchesHandle;

/// Returns the oid of the base of the workspace
/// TODO: Ensure that this is the bottom most common ancestor of all the stacks
pub fn workspace_base(
    ctx: &CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<gix::ObjectId> {
    let gix_repo = ctx.gix_repo_for_merging()?;
    let repo = ctx.repo();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let default_target = vb_state.get_default_target()?;
    let target_branch_commit = repo.find_commit(default_target.sha)?.id().to_gix();
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack_heads = stacks
        .iter()
        .map(|b| b.head(&gix_repo))
        .collect::<Result<Vec<_>>>()?;
    let merge_base_id = gix_repo
        .merge_base_octopus([stack_heads, vec![target_branch_commit]].concat())?
        .object()?
        .id()
        .detach();

    Ok(merge_base_id)
}
