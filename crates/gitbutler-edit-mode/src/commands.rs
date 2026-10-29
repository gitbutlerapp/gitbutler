use anyhow::{Context as _, Result};
use but_core::{ref_metadata::StackId, ui::TreeChange};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use gitbutler_operating_modes::{
    EditModeMetadata, ensure_edit_mode, ensure_open_workspace_mode, in_edit_mode,
};
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

use crate::ConflictEntryPresence;

pub fn enter_edit_mode(
    ctx: &mut Context,
    commit_oid: gix::ObjectId,
    stack_id: StackId,
    perm: &mut RepoExclusive,
) -> Result<EditModeMetadata> {
    ensure_open_workspace_mode(ctx, perm.read_permission())
        .context("Entering edit mode may only be done when the workspace is open")?;

    let snapshot = ctx
        .prepare_snapshot(perm.read_permission())
        .context("Failed to prepare snapshot")?;

    let edit_mode_metadata = crate::enter_edit_mode(ctx, commit_oid, stack_id, perm)?;

    let _ = ctx.commit_snapshot(
        snapshot,
        SnapshotDetails::new(OperationKind::EnterEditMode),
        perm,
    );

    Ok(edit_mode_metadata)
}

pub fn save_and_return_to_workspace(ctx: &mut Context, perm: &mut RepoExclusive) -> Result<()> {
    ensure_edit_mode(ctx, perm.read_permission())
        .context("Edit mode may only be left while in edit mode")?;

    crate::save_and_return_to_workspace(ctx, perm)
}

pub fn abort_and_return_to_workspace(
    ctx: &mut Context,
    force: bool,
    perm: &mut RepoExclusive,
) -> Result<()> {
    ensure_edit_mode(ctx, perm.read_permission())
        .context("Edit mode may only be left while in edit mode")?;

    crate::abort_and_return_to_workspace(ctx, force, perm)
}

pub fn starting_index_state(
    ctx: &Context,
    perm: &RepoShared,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    ensure_edit_mode(ctx, perm)?;

    let state = crate::starting_index_state(ctx, perm)?;
    Ok(state.into_iter().map(|(a, b)| (a.into(), b)).collect())
}

pub fn changes_from_initial(ctx: &Context, perm: &RepoShared) -> Result<Vec<TreeChange>> {
    // The frontend's `worktree_changes` listener may fire one last event
    // after the workspace has already left edit mode. Treat that as "no
    // changes" instead of an error so the listener can stay simple and
    // PostHog/Sentry don't see a noisy benign error.
    if !in_edit_mode(ctx, perm)? {
        return Ok(Vec::new());
    }

    let state = crate::changes_from_initial(ctx, perm)?;
    Ok(state.into_iter().map(|a| a.into()).collect())
}
