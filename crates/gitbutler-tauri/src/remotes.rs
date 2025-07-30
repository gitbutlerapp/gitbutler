use crate::error::Error;
use gitbutler_project::ProjectId;
use gitbutler_repo::{GitRemote, RepoCommands};
use tracing::instrument;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn list_remotes(project_id: ProjectId) -> Result<Vec<GitRemote>, Error> {
    let project = gitbutler_project::get(project_id)?;
    Ok(project.remotes()?)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn add_remote(project_id: ProjectId, name: &str, url: &str) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    Ok(project.add_remote(name, url)?)
}
