//! In place of commands.rs
use anyhow::Result;
use but_api_macros::api_cmd_tauri;
use gitbutler_project::ProjectId;
use gitbutler_repo::{GitRemote, RepoCommands};
use tracing::instrument;

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn list_remotes(project_id: ProjectId) -> Result<Vec<GitRemote>> {
    let project = gitbutler_project::get(project_id)?;
    project.remotes()
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn add_remote(project_id: ProjectId, name: String, url: String) -> Result<()> {
    let project = gitbutler_project::get(project_id)?;
    project.add_remote(&name, &url)
}
