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
use gitbutler_repo::RepositoryExt;
use gitbutler_stack::VirtualBranchesHandle;

pub fn workspace_base(
    ctx: &CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<gix::ObjectId> {
    let gix_repo = ctx.gix_repo_for_merging()?;
    let repo = ctx.repo();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let default_target = vb_state.get_default_target()?;
    let target_branch = repo.find_branch_by_refname(&default_target.branch.into())?;
    let target_branch_commit = target_branch.get().peel_to_commit()?.id().to_gix();

    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack_heads = stacks
        .iter()
        .map(|b| b.head(&gix_repo).map(|h| h.to_gix()))
        .collect::<Result<Vec<_>>>()?;
    let merge_base_tree_id = gix_repo
        .merge_base_octopus([stack_heads, vec![target_branch_commit]].concat())?
        .object()?
        .id()
        .detach();

    Ok(merge_base_tree_id)
}
