use crate::error::Error;
use anyhow::Context;
use gitbutler_branch::diff::FileDiff;
use gitbutler_oplog::entry::Snapshot;
use gitbutler_oplog::OplogExt;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub async fn list_snapshots(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    limit: usize,
    sha: Option<String>,
) -> Result<Vec<Snapshot>, Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let snapshots = project.list_snapshots(
        limit,
        sha.map(|hex| hex.parse().map_err(anyhow::Error::from))
            .transpose()?,
    )?;
    Ok(snapshots)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub async fn restore_snapshot(
    projects: State<'_, projects::Controller>,
    handle: tauri::AppHandle,
    project_id: ProjectId,
    sha: String,
) -> Result<(), Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    project.restore_snapshot(sha.parse().map_err(anyhow::Error::from)?)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub async fn snapshot_diff(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    sha: String,
) -> Result<HashMap<PathBuf, FileDiff>, Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let diff = project.snapshot_diff(sha.parse().map_err(anyhow::Error::from)?)?;
    Ok(diff)
}
