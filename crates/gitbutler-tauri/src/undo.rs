use anyhow::Context;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::entry::OperationKind;
use gitbutler_oplog::{entry::Snapshot, OplogExt};
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn list_snapshots(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
    exclude_kind: Option<Vec<OperationKind>>,
) -> Result<Vec<Snapshot>, Error> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let snapshots = ctx.list_snapshots(
        limit,
        sha.map(|hex| hex.parse().map_err(anyhow::Error::from))
            .transpose()?,
        exclude_kind.unwrap_or_default(),
    )?;
    Ok(snapshots)
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn restore_snapshot(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    sha: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();
    ctx.restore_snapshot(
        sha.parse().map_err(anyhow::Error::from)?,
        guard.write_permission(),
    )?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn snapshot_diff(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    sha: String,
) -> Result<Vec<but_core::ui::TreeChange>, Error> {
    let project = gitbutler_project::get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let diff = ctx.snapshot_diff(sha.parse().map_err(anyhow::Error::from)?)?;
    let diff: Vec<but_core::ui::TreeChange> = diff.into_iter().map(Into::into).collect();
    Ok(diff)
}
