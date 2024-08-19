use anyhow::{Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::{assure_edit_mode, assure_open_workspace_mode, EditModeMetadata};
use gitbutler_oplog::{
    entry::{OperationKind, SnapshotDetails},
    OplogExt,
};
use gitbutler_project::{access::WriteWorkspaceGuard, Project};
use gitbutler_reference::ReferenceName;

pub fn enter_edit_mode(
    project: &Project,
    editee: git2::Oid,
    editee_branch: ReferenceName,
) -> Result<EditModeMetadata> {
    let (ctx, mut guard) = open_with_permission(project)?;

    assure_open_workspace_mode(&ctx)
        .context("Entering edit mode may only be done when the workspace is open")?;

    let editee = ctx
        .repository()
        .find_commit(editee)
        .context("Failed to find editee commit")?;

    let editee_branch = ctx
        .repository()
        .find_reference(&editee_branch)
        .context("Failed to find editee branch reference")?;

    let snapshot = project
        .prepare_snapshot(guard.read_permission())
        .context("Failed to prepare snapshot")?;

    let edit_mode_metadata =
        crate::enter_edit_mode(&ctx, &editee, &editee_branch, guard.write_permission())?;

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

fn open_with_permission(project: &Project) -> Result<(CommandContext, WriteWorkspaceGuard)> {
    let ctx = CommandContext::open(project)?;
    let guard = project.exclusive_worktree_access();
    Ok((ctx, guard))
}
