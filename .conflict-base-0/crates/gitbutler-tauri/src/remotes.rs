use crate::error::Error;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_repo::{GitRemote, RepoCommands};
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn list_remotes(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> Result<Vec<GitRemote>, Error> {
    let project = projects.get(project_id)?;
    Ok(project.remotes()?)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn add_remote(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    name: &str,
    url: &str,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    Ok(project.add_remote(name, url)?)
}
