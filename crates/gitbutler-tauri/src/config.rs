use crate::error::Error;
use but_core::{settings::git::ui::GitConfigSettings, RepositoryExt};
use gitbutler_project::ProjectId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn get_gb_config(project_id: ProjectId) -> Result<GitConfigSettings, Error> {
    but_core::open_repo(gitbutler_project::get(project_id)?.path)?
        .git_settings()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn set_gb_config(project_id: ProjectId, config: GitConfigSettings) -> Result<(), Error> {
    but_core::open_repo(gitbutler_project::get(project_id)?.path)?
        .set_git_settings(&config.into())
        .map_err(Into::into)
}
