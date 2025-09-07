use but_api::commands::config;
use but_api::error::Error;
use but_core::settings::git::ui::GitConfigSettings;
use gitbutler_project::ProjectId;

#[tauri::command(async)]
pub fn get_gb_config(project_id: ProjectId) -> Result<GitConfigSettings, Error> {
    config::get_gb_config(project_id)
}

#[tauri::command(async)]
pub fn set_gb_config(project_id: ProjectId, config: GitConfigSettings) -> Result<(), Error> {
    config::set_gb_config(project_id, config)
}

#[tauri::command(async)]
pub fn store_author_globally_if_unset(
    project_id: ProjectId,
    name: String,
    email: String,
) -> Result<(), Error> {
    config::store_author_globally_if_unset(project_id, name, email)
}

#[tauri::command(async)]
pub fn get_author_info(project_id: ProjectId) -> Result<config::AuthorInfo, Error> {
    config::get_author_info(project_id)
}
