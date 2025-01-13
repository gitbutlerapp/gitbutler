use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use gitbutler_diff::FileDiff;
use gitbutler_oplog::{entry::Snapshot, OplogExt};
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use tauri::State;
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn list_snapshots(
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
pub fn restore_snapshot(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    sha: String,
) -> Result<(), Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let mut guard = project.exclusive_worktree_access();
    project.restore_snapshot(
        sha.parse().map_err(anyhow::Error::from)?,
        guard.write_permission(),
    )?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn snapshot_diff(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    sha: String,
) -> Result<HashMap<PathBuf, FileDiff>, Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let diff = project.snapshot_diff(sha.parse().map_err(anyhow::Error::from)?)?;
    Ok(diff)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn take_synced_snapshot(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    user: User,
    stack_id: Option<StackId>,
) -> Result<String, Error> {
    let project = projects.get(project_id).context("failed to get project")?;
    let snapshot_oid = gitbutler_sync::cloud::take_synced_snapshot(&project, &user, stack_id)?;
    Ok(snapshot_oid.to_string())
}
