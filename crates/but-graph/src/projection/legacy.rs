use anyhow::{Context as _, Result};
use but_core::RefMetadata;

use crate::{init::Overlay, projection::Workspace};

/// Return the managed workspace projection used by legacy workspace calculations, and always
/// recalcuate it if `gitbutler/workspace` is available as some code still requires that.
///
/// This is a transitional helper for code paths that still assume edit mode can temporarily move
/// `HEAD` away from the managed workspace commit. When that happens, the current [`Workspace`]
/// becomes an ad-hoc projection around the temporary edit branch, which is not suitable for
/// workspace snapshot and tree-base calculations that are still defined in terms of the managed
/// `refs/heads/gitbutler/workspace` projection.
///
/// To preserve existing behavior, this helper redoes the traversal with
/// `refs/heads/gitbutler/workspace` as entrypoint when that ref exists, and otherwise falls back
/// to `ws` unchanged.
///
/// This should go away once edit mode no longer takes the user out of the commit-graph and legacy
/// workspace-tree calculations can operate directly on the current projection.
///
/// If the `gitbutler/workspace` reference doesn't exist, it will return `ws` unchanged.
pub fn to_global_workspace(
    repo: &gix::Repository,
    ws: &Workspace,
    meta: &impl RefMetadata,
) -> Result<Workspace> {
    let workspace_ref_name = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")
        .expect("known-valid ref name");

    let Some(mut workspace_ref) = repo.try_find_reference(workspace_ref_name.as_ref())? else {
        return Ok(ws.clone());
    };
    let workspace_ref_id = workspace_ref
        .peel_to_id()
        .context("gitbutler/workspace should point to a commit")?
        .detach();

    ws.graph
        .redo_traversal_with_overlay(
            repo,
            meta,
            Overlay::default().with_entrypoint(workspace_ref_id, Some(workspace_ref_name)),
        )?
        .into_workspace()
}
