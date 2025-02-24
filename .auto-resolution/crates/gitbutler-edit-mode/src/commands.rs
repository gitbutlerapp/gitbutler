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
    ctx: &Context,
    commit_oid: git2::Oid,
    stack_id: StackId,
) -> Result<EditModeMetadata> {
    let mut guard = ctx.exclusive_worktree_access();

    ensure_open_workspace_mode(ctx)
        .context("Entering edit mode may only be done when the workspace is open")?;

    let git2_repo = ctx.git2_repo.get()?;
    let commit = git2_repo
        .find_commit(commit_oid)
        .context("Failed to find commit")?;

    let snapshot = ctx
        .prepare_snapshot(guard.read_permission())
        .context("Failed to prepare snapshot")?;

    let edit_mode_metadata =
        crate::enter_edit_mode(ctx, commit, stack_id, guard.write_permission())?;

    let _ = ctx.commit_snapshot(
        snapshot,
        SnapshotDetails::new(OperationKind::EnterEditMode),
        guard.write_permission(),
    );

    Ok(edit_mode_metadata)
}

pub fn save_and_return_to_workspace(ctx: &Context) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx).context("Edit mode may only be left while in edit mode")?;

    crate::save_and_return_to_workspace(ctx, guard.write_permission())
}

pub fn abort_and_return_to_workspace(ctx: &Context) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx).context("Edit mode may only be left while in edit mode")?;

    crate::abort_and_return_to_workspace(ctx, guard.write_permission())
}

pub fn starting_index_state(
    ctx: &Context,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    let guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx)?;

    let state = crate::starting_index_state(ctx, guard.read_permission())?;
    Ok(state.into_iter().map(|(a, b)| (a.into(), b)).collect())
}

pub fn changes_from_initial(ctx: &Context) -> Result<Vec<TreeChange>> {
    let guard = ctx.exclusive_worktree_access();

    ensure_edit_mode(ctx)?;

    let state = crate::changes_from_initial(ctx, guard.read_permission())?;
    Ok(state.into_iter().map(|a| a.into()).collect())
}
