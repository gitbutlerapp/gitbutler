use gitbutler_config::{api::ProjectCommands, git::GbConfig};
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn get_gb_config(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> Result<GbConfig, Error> {
    projects.get(project_id)?.gb_config().map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn set_gb_config(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    config: GbConfig,
) -> Result<(), Error> {
    projects
        .get(project_id)?
        .set_gb_config(config)
        .map_err(Into::into)
}
