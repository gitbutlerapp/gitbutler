use std::path::PathBuf;

use anyhow::Result;
use but_api_macros::but_api;
use but_ctx::Context;
use gitbutler_project::ProjectHandleOrLegacyProjectId;
use tracing::instrument;

use super::legacy_project;

#[but_api]
#[instrument(err(Debug))]
pub fn update_project(
    project: gitbutler_project::UpdateRequest,
) -> Result<gitbutler_project::api::Project> {
    Ok(gitbutler_project::update(project)?.into())
}

/// Adds an existing git repository as a GitButler project.
/// `path` is the Git repository to remember as project.
#[but_api]
#[instrument(err(Debug))]
pub fn add_project(path: PathBuf) -> Result<gitbutler_project::AddProjectOutcome> {
    gitbutler_project::add(&path)
}

/// Add a project by a given path.
/// It will look for other existing projects and try to match the path
/// to them, allowing to open projects from paths within the repository.
#[but_api]
#[instrument(err(Debug))]
pub fn add_project_best_effort(path: PathBuf) -> Result<gitbutler_project::AddProjectOutcome> {
    gitbutler_project::add_with_best_effort(&path)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_project(
    project_id: ProjectHandleOrLegacyProjectId,
    no_validation: Option<bool>,
) -> Result<gitbutler_project::api::Project> {
    let no_validation = no_validation.unwrap_or(false);
    match project_id {
        ProjectHandleOrLegacyProjectId::ProjectHandle(handle) => {
            let ctx = Context::new_from_project_handle(handle)?;
            if no_validation {
                Ok(ctx.legacy_project.migrated()?.into())
            } else {
                Ok(gitbutler_project::get_validated(ctx.legacy_project.id)?.into())
            }
        }
        ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => {
            if no_validation {
                Ok(
                    gitbutler_project::get_raw(ProjectHandleOrLegacyProjectId::LegacyProjectId(
                        project_id,
                    ))?
                    .migrated()?
                    .into(),
                )
            } else {
                Ok(gitbutler_project::get_validated(
                    ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id),
                )?
                .into())
            }
        }
    }
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_projects(
    opened_projects: Vec<ProjectHandleOrLegacyProjectId>,
) -> Result<Vec<ProjectForFrontend>> {
    let opened_projects: std::collections::HashSet<_> = opened_projects
        .into_iter()
        .map(|project_id| legacy_project(project_id).map(|project| project.id))
        .collect::<Result<_>>()?;

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
pub fn delete_project(project_id: ProjectHandleOrLegacyProjectId) -> Result<()> {
    let project = legacy_project(project_id)?;
    gitbutler_project::delete(project.id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn is_gerrit(ctx: &but_ctx::Context) -> Result<bool> {
    gitbutler_project::gerrit::is_used_by_default_remote(&*ctx.repo.get()?)
}

#[derive(serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: gitbutler_project::api::Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}
