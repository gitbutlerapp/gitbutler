use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use but_api_macros::but_api;
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use but_error::Code;
use tracing::instrument;

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
                let project = gitbutler_project::get_raw(
                    ProjectHandleOrLegacyProjectId::ProjectHandle(handle),
                )?
                .migrated()?
                .into();
                Ok(project)
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

/// List all stored projects for the frontend.
///
/// `opened_projects` identifies projects the frontend currently considers open so the returned
/// entries can be annotated with `is_open`. Stale opened-project handles are ignored because the
/// frontend may still hold them briefly after project deletion.
///
/// This front-end specific behaviour needs review when this comes out of legacy.
#[but_api]
#[instrument(err(Debug))]
pub fn list_projects(
    opened_projects: Vec<ProjectHandleOrLegacyProjectId>,
) -> Result<Vec<ProjectForFrontend>> {
    // Skip handles that can no longer be resolved — e.g. the project was just deleted
    // from storage but the frontend's `opened_projects` set hasn't caught up yet.
    // Failing the whole listing on a stale entry would break the post-deletion refresh
    // flow. Mirrors the warn-and-skip pattern used below for migration failures.
    let opened_projects: std::collections::HashSet<_> = opened_projects
        .into_iter()
        .filter_map(
            |project_id| match gitbutler_project::get_raw(project_id.clone()) {
                Ok(project) => Some(project.id),
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        ?project_id,
                        "Skipping over opened project as its handle could not be resolved"
                    );
                    None
                }
            },
        )
        .collect();

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
    delete_project_at_app_data_dir(but_path::app_data_dir()?, project_id)
}

fn delete_project_at_app_data_dir(
    app_data_dir: impl AsRef<Path>,
    project_id: ProjectHandleOrLegacyProjectId,
) -> Result<()> {
    gitbutler_project::delete_with_path(app_data_dir, project_id)
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
    let repo = ctx.repo.get()?;
    Ok(
        gitbutler_project::gerrit::is_used_by_default_remote(&repo).unwrap_or_else(|err| {
            tracing::debug!(?err, "Gerrit detection failed");
            false
        }),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_project_is_idempotent() -> Result<()> {
        let app_data_dir = tempfile::tempdir()?;
        let repo_dir = tempfile::tempdir()?;
        gix::init(repo_dir.path())?;
        let project = gitbutler_project::add_at_app_data_dir(app_data_dir.path(), repo_dir.path())?
            .unwrap_project();
        let project_id = project.id.clone();

        delete_project_at_app_data_dir(app_data_dir.path(), project_id.clone())?;
        delete_project_at_app_data_dir(app_data_dir.path(), project_id.clone())?;

        assert!(gitbutler_project::get_with_path(app_data_dir.path(), project_id).is_err());
        Ok(())
    }
}
