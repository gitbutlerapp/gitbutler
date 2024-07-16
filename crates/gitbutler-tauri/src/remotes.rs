use crate::error::Error;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub async fn list_remotes(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> Result<Vec<String>, Error> {
    let project = projects.get(project_id)?;
    project.remotes().map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub async fn add_remote(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    name: &str,
    url: &str,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    project.add_remote(name, url).map_err(Into::into)
}
