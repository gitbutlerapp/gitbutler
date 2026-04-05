use std::path::PathBuf;

use anyhow::{Result, anyhow};
use but_api_macros::but_api;
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use but_error::Code;
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
            if no_validation {
                let ctx = Context::new_from_project_handle(handle)?;
                Ok(ctx.legacy_project.migrated()?.into())
            } else {
                Ok(gitbutler_project::get_validated(
                    ProjectHandleOrLegacyProjectId::ProjectHandle(handle),
                )?
                .into())
            }
        }
        ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => Ok(if no_validation {
            gitbutler_project::get_raw(ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id))?
                .migrated()?
                .into()
        } else {
            gitbutler_project::get_validated(ProjectHandleOrLegacyProjectId::LegacyProjectId(
                project_id,
            ))?
            .into()
        }),
    }
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_projects_stateless() -> Result<Vec<ProjectForFrontend>> {
    list_projects(vec![])
}

#[but_api]
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

/// Prepare an already-known project for activation in the UI or server.
///
/// This repairs missing target metadata in freshly selected storage locations and then reconciles
/// the legacy metadata view with the workspace currently present in Git. It is safe for activation
/// paths because it avoids rewriting `gitbutler/workspace`.
pub fn prepare_project_for_activation(ctx: &mut Context) -> Result<()> {
    assure_repo_ownership(&*ctx.repo.get()?)?;
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::base::bootstrap_default_target_if_missing(ctx)?;
    super::meta::reconcile_in_workspace_state_of_vb_toml(ctx, guard.write_permission()).ok();
    Ok(())
}

// TODO(gix): remove this once there is no `git2` as `gix` provides safety by not trusting Git configuration instead.
fn assure_repo_ownership(repo: &gix::Repository) -> Result<()> {
    if repo.git_dir_trust() == gix::sec::Trust::Full {
        return Ok(());
    }

    let path = repo.workdir().unwrap_or(repo.git_dir());
    Err(anyhow!(
        "The git directory is considered unsafe as it's not owned by the current user. Use `git config --global --add safe.directory '{}'` to allow it",
        path.display()
    )
    .context(Code::RepoOwnership))
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
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ProjectForFrontend);
