//! In place of commands.rs
use anyhow::Context;
use anyhow::anyhow;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_repo_actions::RepoActionsExt as _;
use serde::Deserialize;

use crate::NoParams;
use crate::error::Error;
use crate::error::ToError as _;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoteBranchesParams {
    pub project_id: ProjectId,
}

pub fn git_remote_branches(params: GitRemoteBranchesParams) -> Result<Vec<RemoteRefname>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(ctx.repo().remote_branches()?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTestPushParams {
    pub project_id: ProjectId,
    pub remote_name: String,
    pub branch_name: String,
}

pub fn git_test_push(params: GitTestPushParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    ctx.git_test_push(&params.remote_name, &params.branch_name, Some(None))?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTestFetchParams {
    pub project_id: ProjectId,
    pub remote_name: String,
    pub action: Option<String>,
}

pub fn git_test_fetch(params: GitTestFetchParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    ctx.fetch(
        &params.remote_name,
        Some(params.action.unwrap_or_else(|| "test".to_string())),
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIndexSizeParams {
    pub project_id: ProjectId,
}

pub fn git_index_size(params: GitIndexSizeParams) -> Result<usize, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let size = ctx
        .repo()
        .index()
        .context("failed to get index size")?
        .len();
    Ok(size)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHeadParams {
    pub project_id: ProjectId,
}

pub fn git_head(params: GitHeadParams) -> Result<String, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let head = ctx.repo().head().context("failed to get repository head")?;
    Ok(head.name().unwrap().to_string())
}

pub fn delete_all_data(_params: NoParams) -> Result<(), Error> {
    for project in gitbutler_project::list().context("failed to list projects")? {
        gitbutler_project::delete(project.id)
            .map_err(|err| err.context("failed to delete project"))?;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSetGlobalConfigParams {
    pub key: String,
    pub value: String,
}

pub fn git_set_global_config(params: GitSetGlobalConfigParams) -> Result<String, Error> {
    let mut config = git2::Config::open_default().to_error()?;
    config.set_str(&params.key, &params.value).to_error()?;
    Ok(params.value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoveGlobalConfigParams {
    pub key: String,
}

pub fn git_remove_global_config(params: GitRemoveGlobalConfigParams) -> Result<(), Error> {
    let mut config = git2::Config::open_default().to_error()?;
    config.remove(&params.key).to_error()?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGetGlobalConfigParams {
    pub key: String,
}

pub fn git_get_global_config(params: GitGetGlobalConfigParams) -> Result<Option<String>, Error> {
    let config = git2::Config::open_default().to_error()?;
    let value = config.get_string(&params.key);
    match value {
        Ok(value) => Ok(Some(value)),
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                Ok(None)
            } else {
                Err(anyhow!(e).into())
            }
        }
    }
}
