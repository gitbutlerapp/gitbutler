use crate::error::Error;
use anyhow::Context;
use gitbutler_core::{
    projects, projects::ProjectId, snapshots::snapshot, snapshots::snapshot::SnapshotEntry,
};
use tauri::Manager;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn list_snapshots(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    limit: usize,
) -> Result<Vec<SnapshotEntry>, Error> {
    let project = handle
        .state::<projects::Controller>()
        .get(&project_id)
        .context("failed to get project")?;
    let snapshots = snapshot::list(project, limit)?;
    Ok(snapshots)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn restore_snapshot(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    sha: String,
) -> Result<(), Error> {
    let project = handle
        .state::<projects::Controller>()
        .get(&project_id)
        .context("failed to get project")?;
    snapshot::restore(&project, sha)?;
    Ok(())
}
