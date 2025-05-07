use anyhow::{Context, Result};
use gitbutler_branch_actions::RemoteBranchFile;
use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::{assure_edit_mode, assure_open_workspace_mode, EditModeMetadata};
use gitbutler_oplog::{
    entry::{OperationKind, SnapshotDetails},
    OplogExt,
};
use gitbutler_reference::ReferenceName;

use crate::ConflictEntryPresence;

pub fn enter_edit_mode(
    ctx: &CommandContext,
    commit_oid: git2::Oid,
    branch_reference_name: ReferenceName,
) -> Result<EditModeMetadata> {
    let mut guard = ctx.project().exclusive_worktree_access();

    assure_open_workspace_mode(ctx)
        .context("Entering edit mode may only be done when the workspace is open")?;

    let commit = ctx
        .repo()
        .find_commit(commit_oid)
        .context("Failed to find commit")?;

    let branch = ctx
        .repo()
        .find_reference(&branch_reference_name)
        .context("Failed to find branch reference")?;

    let snapshot = ctx
        .prepare_snapshot(guard.read_permission())
        .context("Failed to prepare snapshot")?;

    let edit_mode_metadata =
        crate::enter_edit_mode(ctx, commit, &branch, guard.write_permission())?;

    let _ = ctx.commit_snapshot(
        snapshot,
        SnapshotDetails::new(OperationKind::EnterEditMode),
        guard.write_permission(),
    );

    Ok(edit_mode_metadata)
}

pub fn save_and_return_to_workspace(ctx: &CommandContext) -> Result<()> {
    let mut guard = ctx.project().exclusive_worktree_access();

    assure_edit_mode(ctx).context("Edit mode may only be left while in edit mode")?;

    crate::save_and_return_to_workspace(ctx, guard.write_permission())
}

pub fn abort_and_return_to_workspace(ctx: &CommandContext) -> Result<()> {
    let mut guard = ctx.project().exclusive_worktree_access();

    assure_edit_mode(ctx).context("Edit mode may only be left while in edit mode")?;

    crate::abort_and_return_to_workspace(ctx, guard.write_permission())
}

pub fn starting_index_state(
    ctx: &CommandContext,
) -> Result<Vec<(RemoteBranchFile, Option<ConflictEntryPresence>)>> {
    let guard = ctx.project().exclusive_worktree_access();

    assure_edit_mode(ctx)?;

    crate::starting_index_state(ctx, guard.read_permission())
}
