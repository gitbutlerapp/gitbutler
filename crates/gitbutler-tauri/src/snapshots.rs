use crate::error::Error;
use anyhow::Context;
use gitbutler_core::{
    projects::{self, ProjectId},
    snapshots::{entry::Snapshot, snapshot::Oplog},
};
use tauri::Manager;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn list_snapshots(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
) -> Result<Vec<Snapshot>, Error> {
    let project = handle
        .state::<projects::Controller>()
        .get(&project_id)
        .context("failed to get project")?;
    let snapshots = project.list_snapshots(limit, sha)?;
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
    project.restore_snapshot(sha)?;
    Ok(())
}
