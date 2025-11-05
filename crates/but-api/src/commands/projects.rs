use std::path::PathBuf;

use but_api_macros::api_cmd;
use gitbutler_project::{self as projects, ProjectId};
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn update_project(project: projects::UpdateRequest) -> Result<projects::api::Project, Error> {
    Ok(gitbutler_project::update(&project)?.into())
}

/// Adds an existing git repository as a GitButler project.
/// If the directory is not a git repository, an error is returned.
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn add_project(path: PathBuf) -> Result<projects::AddProjectOutcome, Error> {
    Ok(gitbutler_project::add(&path)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_project(
    project_id: ProjectId,
    no_validation: Option<bool>,
) -> Result<projects::api::Project, Error> {
    if no_validation.unwrap_or(false) {
        Ok(gitbutler_project::get_raw(project_id)?.migrated()?.into())
    } else {
        Ok(gitbutler_project::get_validated(project_id)?.into())
    }
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn delete_project(project_id: ProjectId) -> Result<(), Error> {
    gitbutler_project::delete(project_id).map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn is_gerrit(project_id: ProjectId) -> Result<bool, Error> {
    let project = gitbutler_project::get_raw(project_id)?;
    let repo = project.open()?;
    gitbutler_project::gerrit::is_used_by_default_remote(&repo).map_err(Into::into)
}
