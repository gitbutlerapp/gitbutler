use but_api::commands::config::StoreAuthorGloballyParams;
use but_api::error::Error;
use but_api::{commands::config, App};
use but_core::settings::git::ui::GitConfigSettings;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

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

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn store_author_globally_if_unset(
    app: State<App>,
    project_id: ProjectId,
    name: String,
    email: String,
) -> Result<(), Error> {
    config::store_author_globally_if_unset(
        &app,
        StoreAuthorGloballyParams {
            project_id,
            name,
            email,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn get_author_info(
    app: State<App>,
    project_id: ProjectId,
) -> Result<config::AuthorInfo, Error> {
    config::get_author_info(&app, project_id)
}
