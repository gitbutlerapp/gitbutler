use crate::error::Error;
use anyhow::Context;
use gitbutler_core::{
    projects::{self, ProjectId},
    snapshots::{self, entry::Snapshot, snapshot::Oplog},
};
use tauri::Manager;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn list_snapshots(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    limit: usize,
) -> Result<Vec<Snapshot>, Error> {
    let project = handle
        .state::<projects::Controller>()
        .get(&project_id)
        .context("failed to get project")?;
    let snapshots = project.list_snapshots(limit)?;
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

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn snapshots_enabled(
    handle: tauri::AppHandle,
    project_id: ProjectId,
) -> Result<bool, Error> {
    handle
        .state::<snapshots::Controller>()
        .snapshots_enabled(&project_id)
        .map_err(Error::from_error_with_context)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn set_snapshots_enabled(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    value: bool,
) -> Result<(), Error> {
    handle
        .state::<snapshots::Controller>()
        .set_snapshots_enabled(&project_id, value)
        .map_err(Error::from_error_with_context)
}
