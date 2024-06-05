use crate::error::Error;
use gitbutler_core::{
    config::git::GbConfig,
    projects::{self, ProjectId},
};
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
