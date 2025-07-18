use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use but_core::ui::TreeChanges;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_command_context::CommandContext;
use gitbutler_diff::FileDiff;
use gitbutler_oplog::entry::OperationKind;
use gitbutler_oplog::{entry::Snapshot, OplogExt};
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use crate::error::Error;
use crate::from_json::HexHash;

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn list_snapshots(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
    exclude_kind: Option<Vec<OperationKind>>,
) -> Result<Vec<Snapshot>, Error> {
    let project = projects.get(project_id).context("failed to get project")?;
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
#[instrument(skip(projects, settings), err(Debug))]
pub fn restore_snapshot(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    sha: String,
) -> Result<(), Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();
    ctx.restore_snapshot(
        sha.parse().map_err(anyhow::Error::from)?,
        guard.write_permission(),
    )?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn snapshot_diff(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    sha: String,
) -> Result<HashMap<PathBuf, FileDiff>, Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let diff = ctx.snapshot_diff(sha.parse().map_err(anyhow::Error::from)?)?;
    Ok(diff)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn oplog_diff_worktrees(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    before: HexHash,
    after: HexHash,
) -> Result<TreeChanges, Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;

    let before = ctx.snapshot_workspace_tree(*before)?;
    let after = ctx.snapshot_workspace_tree(*after)?;

    let diff = but_core::diff::ui::changes_in_range(ctx.project().path.clone(), after, before)?;
    Ok(diff)
}
