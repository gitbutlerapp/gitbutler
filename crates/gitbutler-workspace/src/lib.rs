pub mod branch_trees;

use anyhow::Result;
use but_ctx::Context;
use but_meta::VirtualBranchesTomlMetadata;

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
