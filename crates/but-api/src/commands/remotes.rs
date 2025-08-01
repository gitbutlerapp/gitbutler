//! In place of commands.rs
use gitbutler_project::ProjectId;
use gitbutler_repo::{GitRemote, RepoCommands};
use serde::Deserialize;

use crate::{IpcContext, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRemotesParams {
    pub project_id: ProjectId,
}

pub fn list_remotes(
    _ipc_ctx: &IpcContext,
    params: ListRemotesParams,
) -> Result<Vec<GitRemote>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    Ok(project.remotes()?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRemoteParams {
    pub project_id: ProjectId,
    pub name: String,
    pub url: String,
}

pub fn add_remote(_ipc_ctx: &IpcContext, params: AddRemoteParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    Ok(project.add_remote(&params.name, &params.url)?)
}
