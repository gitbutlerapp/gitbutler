pub mod branch_trees;

use anyhow::Result;
use but_ctx::{Context, access::RepoShared};
use but_oxidize::OidExt;
use gitbutler_stack::VirtualBranchesHandle;

/// Returns the oid of the base of the workspace
/// TODO: Ensure that this is the bottom most common ancestor of all the stacks
pub fn workspace_base(ctx: &Context, _perm: &RepoShared) -> Result<gix::ObjectId> {
    let repo = ctx.clone_repo_for_merging()?;
    let git2_repo = &*ctx.git2_repo.get()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let default_target = vb_state.get_default_target()?;
    let target_branch_commit = git2_repo.find_commit(default_target.sha)?.id().to_gix();
    let stacks = vb_state.list_stacks_in_workspace()?;
    let stack_heads = stacks
        .iter()
        .map(|b| b.head_oid(ctx))
        .collect::<Result<Vec<_>>>()?;
    let merge_base_id = repo
        .merge_base_octopus([stack_heads, vec![target_branch_commit]].concat())?
        .object()?
        .id()
        .detach();

    Ok(merge_base_id)
}

pub fn workspace_base_from_heads(
    ctx: &Context,
    _perm: &RepoShared,
    heads: &[gix::ObjectId],
) -> Result<gix::ObjectId> {
    let repo = ctx.clone_repo_for_merging()?;
    let git2_repo = &*ctx.git2_repo.get()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let default_target = vb_state.get_default_target()?;
    let target_branch_commit = git2_repo.find_commit(default_target.sha)?.id().to_gix();
    let merge_base_id = repo
        .merge_base_octopus([heads, &[target_branch_commit]].concat())?
        .object()?
        .id()
        .detach();

    Ok(merge_base_id)
}
