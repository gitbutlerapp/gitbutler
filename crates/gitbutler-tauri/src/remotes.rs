use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use tauri::State;
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn list_remotes(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> Result<Vec<String>, Error> {
    let project = projects.get(project_id)?;
    project.remotes().map_err(Into::into)
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
    project.add_remote(name, url).map_err(Into::into)
}
