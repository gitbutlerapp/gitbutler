use crate::error::Error;
use gitbutler_config::{api::ProjectCommands, git::GbConfig};
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use tauri::Manager;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn get_gb_config(
    handle: tauri::AppHandle,
    project_id: ProjectId,
) -> Result<GbConfig, Error> {
    handle
        .state::<projects::Controller>()
        .get(project_id)?
        .gb_config()
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn set_gb_config(
    handle: tauri::AppHandle,
    project_id: ProjectId,
    config: GbConfig,
) -> Result<(), Error> {
    handle
        .state::<projects::Controller>()
        .get(project_id)?
        .set_gb_config(config)
        .map_err(Into::into)
}
