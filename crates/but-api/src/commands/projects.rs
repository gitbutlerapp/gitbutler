use crate::error::Error;
use anyhow::Context;
use but_api_macros::api_cmd;
use gitbutler_project::{self as projects, ProjectId};
use std::path::PathBuf;
use tracing::instrument;

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn update_project(project: projects::UpdateRequest) -> Result<projects::Project, Error> {
    Ok(gitbutler_project::update(&project)?)
}

/// Adds an existing git repository as a GitButler project.
/// If the directory is not a git repository, an error is returned.
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn add_project(path: PathBuf) -> Result<projects::AddProjectOutcome, Error> {
    Ok(gitbutler_project::add(&path)?)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn get_project(
    project_id: ProjectId,
    no_validation: Option<bool>,
) -> Result<projects::Project, Error> {
    if no_validation.unwrap_or(false) {
        Ok(gitbutler_project::get_raw(project_id)?)
    } else {
        Ok(gitbutler_project::get_validated(project_id)?)
    }
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn delete_project(project_id: ProjectId) -> Result<(), Error> {
    gitbutler_project::delete(project_id).map_err(Into::into)
}

/// Initialize a Git repository at the given path
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn init_git_repository(path: String) -> Result<(), Error> {
    let path: PathBuf = path.into();
    git2::Repository::init(&path)
        .with_context(|| format!("Failed to initialize Git repository at {}", path.display()))?;
    Ok(())
}
