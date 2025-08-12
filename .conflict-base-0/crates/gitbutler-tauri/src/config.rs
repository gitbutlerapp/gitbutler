use but_api::{commands::config, App};
use but_core::settings::git::ui::GitConfigSettings;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn get_gb_config(app: State<App>, project_id: ProjectId) -> Result<GitConfigSettings, Error> {
    config::get_gb_config(&app, config::GetGbConfigParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn set_gb_config(
    app: State<App>,
    project_id: ProjectId,
    config: GitConfigSettings,
) -> Result<(), Error> {
    config::set_gb_config(&app, config::SetGbConfigParams { project_id, config })
}
