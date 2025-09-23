//! In place of commands.rs
use anyhow::Context;
use anyhow::anyhow;
use but_api_macros::api_cmd;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_repo_actions::RepoActionsExt as _;
use tracing::instrument;

use crate::error::Error;
use crate::error::ToError as _;

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_remote_branches(project_id: ProjectId) -> Result<Vec<RemoteRefname>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(ctx.repo().remote_branches()?)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_test_push(
    project_id: ProjectId,
    remote_name: String,
    branch_name: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    ctx.git_test_push(&remote_name, &branch_name, Some(None))?;
    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_test_fetch(
    project_id: ProjectId,
    remote_name: String,
    action: Option<String>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    ctx.fetch(
        &remote_name,
        Some(action.unwrap_or_else(|| "test".to_string())),
    )?;
    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_index_size(project_id: ProjectId) -> Result<usize, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let size = ctx
        .repo()
        .index()
        .context("failed to get index size")?
        .len();
    Ok(size)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn delete_all_data() -> Result<(), Error> {
    for project in gitbutler_project::list().context("failed to list projects")? {
        gitbutler_project::delete(project.id)
            .map_err(|err| err.context("failed to delete project"))?;
    }
    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_set_global_config(key: String, value: String) -> Result<String, Error> {
    let mut config = git2::Config::open_default().to_error()?;
    config.set_str(&key, &value).to_error()?;
    Ok(value)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_remove_global_config(key: String) -> Result<(), Error> {
    let mut config = git2::Config::open_default().to_error()?;
    config.remove(&key).to_error()?;
    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn git_get_global_config(key: String) -> Result<Option<String>, Error> {
    let config = git2::Config::open_default().to_error()?;
    let value = config.get_string(&key);
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
