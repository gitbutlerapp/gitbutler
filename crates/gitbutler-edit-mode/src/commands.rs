use anyhow::{Context as _, Result};
use but_core::{ref_metadata::StackId, ui::TreeChange};
use but_ctx::Context;
use gitbutler_operating_modes::{EditModeMetadata, ensure_edit_mode, ensure_open_workspace_mode};
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

use crate::ConflictEntryPresence;

pub fn enter_edit_mode(
    ctx: &mut Context,
    commit_oid: gix::ObjectId,
    stack_id: StackId,
) -> Result<EditModeMetadata> {
    let mut guard = ctx.exclusive_worktree_access();

    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Entering edit mode may only be done when the workspace is open")?;

    let snapshot = ctx
        .prepare_snapshot(guard.read_permission())
        .context("Failed to prepare snapshot")?;

    let edit_mode_metadata =
        crate::enter_edit_mode(ctx, commit_oid, stack_id, guard.write_permission())?;

    let _ = ctx.commit_snapshot(
        snapshot,
        SnapshotDetails::new(OperationKind::EnterEditMode),
        guard.write_permission(),
    );

    Ok(edit_mode_metadata)
}

pub fn save_and_return_to_workspace(ctx: &mut Context) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx, guard.read_permission())
        .context("Edit mode may only be left while in edit mode")?;

    crate::save_and_return_to_workspace(ctx, guard.write_permission())
}

pub fn abort_and_return_to_workspace(ctx: &mut Context, force: bool) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx, guard.read_permission())
        .context("Edit mode may only be left while in edit mode")?;

    crate::abort_and_return_to_workspace(ctx, force, guard.write_permission())
}

pub fn starting_index_state(
    ctx: &mut Context,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    let guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx, guard.read_permission())?;

    let state = crate::starting_index_state(ctx, guard.read_permission())?;
    Ok(state.into_iter().map(|(a, b)| (a.into(), b)).collect())
}

pub fn changes_from_initial(ctx: &mut Context) -> Result<Vec<TreeChange>> {
    let guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx, guard.read_permission())?;

    let state = crate::changes_from_initial(ctx, guard.read_permission())?;
    Ok(state.into_iter().map(|a| a.into()).collect())
}
