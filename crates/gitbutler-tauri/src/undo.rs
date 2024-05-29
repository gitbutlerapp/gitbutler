use crate::error::Error;
use anyhow::Context;
use gitbutler_core::git::diff::FileDiff;
use gitbutler_core::{
    ops::entry::Snapshot,
    projects::{self, ProjectId},
};
use std::collections::HashMap;
use std::path::PathBuf;
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
        .get(project_id)
        .context("failed to get project")?;
    let snapshots = project.list_snapshots(
        limit,
        sha.map(|hex| hex.parse().map_err(anyhow::Error::from))
            .transpose()?,
    )?;
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
        .get(project_id)
        .context("failed to get project")?;
    project.restore_snapshot(sha.parse().map_err(anyhow::Error::from)?)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn snapshot_diff(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    sha: String,
) -> Result<HashMap<PathBuf, FileDiff>, Error> {
    let project = handle
        .state::<projects::Controller>()
        .get(project_id)
        .context("failed to get project")?;
    let diff = project.snapshot_diff(sha.parse().map_err(anyhow::Error::from)?)?;
    Ok(diff)
}
