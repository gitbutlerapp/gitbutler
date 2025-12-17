//! In place of commands.rs
use but_api_macros::api_cmd;
use gitbutler_project::ProjectId;
use gitbutler_repo::{GitRemote, RepoCommands};
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn list_remotes(project_id: ProjectId) -> Result<Vec<GitRemote>, Error> {
    let project = gitbutler_project::get(project_id)?;
    Ok(project.remotes()?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn add_remote(project_id: ProjectId, name: String, url: String) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    Ok(project.add_remote(&name, &url)?)
}
