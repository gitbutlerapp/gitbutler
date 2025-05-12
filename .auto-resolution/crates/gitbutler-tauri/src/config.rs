use crate::error::Error;
use but_core::{settings::git::ui::GitConfigSettings, RepositoryExt};
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn get_gb_config(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> Result<GitConfigSettings, Error> {
    but_core::open_repo(projects.get(project_id)?.path)?
        .git_settings()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn set_gb_config(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    config: GitConfigSettings,
) -> Result<(), Error> {
    but_core::open_repo(projects.get(project_id)?.path)?
        .set_git_settings(&config.into())
        .map_err(Into::into)
}
