pub mod branch_trees;

use anyhow::{Context as _, Result};
use but_ctx::{Context, access::RepoShared};
use but_graph::projection::{Workspace, legacy};

/// Returns the oid of the base of the workspace
/// TODO: Ensure that this is the bottom most common ancestor of all the stacks
pub fn workspace_base(ctx: &Context, perm: &RepoShared) -> Result<gix::ObjectId> {
    let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm)?;
    let meta = ctx.meta()?;
    let ws = legacy::to_global_workspace(&repo, &ws, &meta)?;
    ws.lower_bound
        .context("Cannot handle workspaces without a base")
}

/// Note that this recomputes the merge-base from the actual commit-graph, instead of using the one in
/// `ctx.ws`, which is left here as it's legacy and supposed to be removed at some point soon.
pub fn workspace_base_from_heads(
    ctx: &Context,
    perm: &RepoShared,
    heads: &[gix::ObjectId],
) -> Result<gix::ObjectId> {
    let (_, ws, _) = ctx.workspace_and_db_with_perm(perm)?;
    let repo = ctx.clone_repo_for_merging()?;
    let meta = ctx.meta()?;
    let ws = legacy::to_global_workspace(&repo, &ws, &meta)?;
    let target_branch_commit = ws.target_commit_id()?;
    let merge_base_id = repo
        .merge_base_octopus([heads, &[target_branch_commit]].concat())?
        .object()?
        .id()
        .detach();

    Ok(merge_base_id)
}

pub(crate) fn workspace_stack_heads(ws: &Workspace) -> Vec<gix::ObjectId> {
    ws.stacks
        .iter()
        .filter_map(|stack| stack.tip_skip_empty().or_else(|| stack.base()))
        .collect()
}
