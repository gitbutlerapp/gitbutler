use crate::error::Error;
use anyhow::Context;
use gitbutler_core::projects::{self, ProjectId};
use tauri::Manager;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn get_sign_commits_config(
    handle: tauri::AppHandle,
    project_id: ProjectId,
) -> Result<Option<bool>, Error> {
    handle
        .state::<projects::Controller>()
        .get(project_id)
        .context("failed to get project")?
        .sign_commits()
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn set_sign_commits_config(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    value: bool,
) -> Result<(), Error> {
    handle
        .state::<projects::Controller>()
        .get(project_id)
        .context("failed to get project")?
        .set_sign_commits(value)
        .map_err(Into::into)
}
