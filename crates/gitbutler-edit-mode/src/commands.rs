use anyhow::{Context, Result};
use gitbutler_branch_actions::RemoteBranchFile;
use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::{assure_edit_mode, assure_open_workspace_mode, EditModeMetadata};
use gitbutler_oplog::{
    entry::{OperationKind, SnapshotDetails},
    OplogExt,
};
use gitbutler_project::{access::WriteWorkspaceGuard, Project};
use gitbutler_reference::ReferenceName;

use crate::ConflictEntryPresence;

pub fn enter_edit_mode(
    project: &Project,
    commit_oid: git2::Oid,
    branch_reference_name: ReferenceName,
) -> Result<EditModeMetadata> {
    let (ctx, mut guard) = open_with_permission(project)?;

    assure_open_workspace_mode(&ctx)
        .context("Entering edit mode may only be done when the workspace is open")?;

    let commit = ctx
        .repository()
        .find_commit(commit_oid)
        .context("Failed to find commit")?;

    let branch = ctx
        .repository()
        .find_reference(&branch_reference_name)
        .context("Failed to find branch reference")?;

    let snapshot = project
        .prepare_snapshot(guard.read_permission())
        .context("Failed to prepare snapshot")?;

    let edit_mode_metadata =
        crate::enter_edit_mode(&ctx, &commit, &branch, guard.write_permission())?;

    let _ = project.commit_snapshot(
        snapshot,
        SnapshotDetails::new(OperationKind::EnterEditMode),
        guard.write_permission(),
    );

    Ok(edit_mode_metadata)
}

pub fn save_and_return_to_workspace(project: &Project) -> Result<()> {
    let (ctx, mut guard) = open_with_permission(project)?;

    assure_edit_mode(&ctx).context("Edit mode may only be left while in edit mode")?;

    crate::save_and_return_to_workspace(&ctx, guard.write_permission())
}

pub fn abort_and_return_to_workspace(project: &Project) -> Result<()> {
    let (ctx, mut guard) = open_with_permission(project)?;

    assure_edit_mode(&ctx).context("Edit mode may only be left while in edit mode")?;

    crate::abort_and_return_to_workspace(&ctx, guard.write_permission())
}

pub fn starting_index_state(
    project: &Project,
) -> Result<Vec<(RemoteBranchFile, Option<ConflictEntryPresence>)>> {
    let (ctx, guard) = open_with_permission(project)?;

    assure_edit_mode(&ctx)?;

    crate::starting_index_state(&ctx, guard.read_permission())
}

fn open_with_permission(project: &Project) -> Result<(CommandContext, WriteWorkspaceGuard)> {
    let ctx = CommandContext::open(project)?;
    let guard = project.exclusive_worktree_access();
    Ok((ctx, guard))
}
