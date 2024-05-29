use crate::error::Error;
use gitbutler_core::{projects::ProjectId, remotes::Controller};
use tauri::Manager;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn list_remotes(
    handle: tauri::AppHandle,
    project_id: ProjectId,
) -> Result<Vec<String>, Error> {
    handle
        .state::<Controller>()
        .remotes(project_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn add_remote(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    name: &str,
    url: &str,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .add_remote(project_id, name, url)
        .await
        .map_err(Into::into)
}
