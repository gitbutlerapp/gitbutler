use std::path::PathBuf;

use anyhow::Result;
use but_api_macros::but_api;
use gitbutler_project::{self as projects, ProjectId};
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn update_project(project: projects::UpdateRequest) -> Result<projects::api::Project> {
    Ok(gitbutler_project::update(project)?.into())
}

/// Adds an existing git repository as a GitButler project.
/// If the directory is not a git repository, an error is returned.
#[but_api]
#[instrument(err(Debug))]
pub fn add_project(path: PathBuf) -> Result<projects::AddProjectOutcome> {
    gitbutler_project::add(&path)
}

/// Add a project by a given path.
/// It will look for other existing projects and try to match the path
/// to them, allowing to open projects from paths within the repository.
#[but_api]
#[instrument(err(Debug))]
pub fn add_project_best_effort(path: PathBuf) -> Result<projects::AddProjectOutcome> {
    gitbutler_project::add_with_best_effort(&path)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_project(
    project_id: ProjectId,
    no_validation: Option<bool>,
) -> Result<projects::api::Project> {
    if no_validation.unwrap_or(false) {
        Ok(gitbutler_project::get_raw(project_id)?.migrated()?.into())
    } else {
        Ok(gitbutler_project::get_validated(project_id)?.into())
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_projects(opened_projects: Vec<ProjectId>) -> Result<Vec<ProjectForFrontend>> {
    gitbutler_project::assure_app_can_startup_or_fix_it(
        gitbutler_project::dangerously_list_projects_without_migration(),
    )
    .map(|projects| {
        projects
            .into_iter()
            .map(|project| {
                anyhow::Ok(ProjectForFrontend {
                    is_open: opened_projects.contains(&project.id),
                    inner: project.migrated().map(Into::into)?,
                })
            })
            .filter_map(|res| match res {
                Ok(p) => Some(p),
                Err(err) => {
                    tracing::warn!(?err, "Skipping over project as it failed migration");
                    None
                }
            })
            .collect()
    })
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_project(project_id: ProjectId) -> Result<()> {
    gitbutler_project::delete(project_id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn is_gerrit(project_id: ProjectId) -> Result<bool> {
    let project = gitbutler_project::get_raw(project_id)?;
    let repo = project.open_repo()?;
    gitbutler_project::gerrit::is_used_by_default_remote(&repo)
}

#[derive(serde::Serialize)]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: gitbutler_project::api::Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}
