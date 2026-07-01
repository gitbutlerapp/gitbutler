pub mod branch_trees;

use anyhow::Result;
use but_ctx::{Context, access::RepoShared};
use but_meta::VirtualBranchesTomlMetadata;

/// Returns the oid of the base of the workspace
/// TODO: Ensure that this is the bottom most common ancestor of all the stacks
pub fn workspace_base(ctx: &Context, _perm: &RepoShared) -> Result<gix::ObjectId> {
    let repo = ctx.clone_repo_for_merging()?;
    let meta = ctx.legacy_meta()?;
    let target_base_oid = ctx.project_meta()?.target_commit_id_or_err()?;
    let stack_heads = legacy_workspace_stack_heads_from_meta(&meta, &repo, target_base_oid)?;
    workspace_base_from_heads_and_target(&repo, &stack_heads, target_base_oid)
}

pub fn workspace_base_from_heads(
    ctx: &Context,
    _perm: &RepoShared,
    heads: &[gix::ObjectId],
) -> Result<gix::ObjectId> {
    let repo = ctx.clone_repo_for_merging()?;
    let target_base_oid = ctx.project_meta()?.target_commit_id_or_err()?;
    workspace_base_from_heads_and_target(&repo, heads, target_base_oid)
}

pub(crate) fn legacy_workspace_stack_heads(
    ctx: &Context,
    repo: &gix::Repository,
    target_base_oid: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    let meta = ctx.legacy_meta()?;
    legacy_workspace_stack_heads_from_meta(&meta, repo, target_base_oid)
}

fn legacy_workspace_stack_heads_from_meta(
    meta: &VirtualBranchesTomlMetadata,
    repo: &gix::Repository,
    target_base_oid: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    meta.data()
        .branches
        .values()
        .filter(|stack| stack.in_workspace)
        .map(|stack| {
            stack
                .heads
                .last()
                .map(|head| -> Result<gix::ObjectId> {
                    if let Some(mut reference) = repo.try_find_reference(&head.name)? {
                        Ok(reference.peel_to_commit()?.id)
                    } else {
                        Ok(head.head)
                    }
                })
                .unwrap_or(Ok(target_base_oid))
        })
        .collect()
}

pub(crate) fn legacy_target_base_oid(ctx: &Context) -> Result<gix::ObjectId> {
    ctx.project_meta()?.target_commit_id_or_err()
}

pub(crate) fn workspace_base_from_heads_and_target(
    repo: &gix::Repository,
    heads: &[gix::ObjectId],
    target_base_oid: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let merge_base_id = repo
        .merge_base_octopus([heads, &[target_base_oid]].concat())?
        .object()?
        .id()
        .detach();

    Ok(merge_base_id)
}
