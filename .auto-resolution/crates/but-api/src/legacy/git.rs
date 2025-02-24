//! In place of commands.rs
use anyhow::{Context as _, Result, anyhow};
use but_api_macros::but_api;
use but_ctx::Context;
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_repo_actions::RepoActionsExt as _;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn git_remote_branches(project_id: ProjectId) -> Result<Vec<RemoteRefname>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let repo = ctx.git2_repo.get()?;
    repo.remote_branches()
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_test_push(
    project_id: ProjectId,
    remote_name: String,
    branch_name: String,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    ctx.git_test_push(&remote_name, &branch_name, Some(None))?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_test_fetch(
    project_id: ProjectId,
    remote_name: String,
    action: Option<String>,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    ctx.fetch(
        &remote_name,
        Some(action.unwrap_or_else(|| "test".to_string())),
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_index_size(project_id: ProjectId) -> Result<usize> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let size = ctx
        .git2_repo
        .get()?
        .index()
        .context("failed to get index size")?
        .len();
    Ok(size)
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_all_data() -> Result<()> {
    for project in gitbutler_project::dangerously_list_projects_without_migration()
        .context("failed to list projects")?
    {
        gitbutler_project::delete(project.id)
            .map_err(|err| err.context("failed to delete project"))?;
    }
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_set_global_config(key: String, value: String) -> Result<String> {
    let mut config = git2::Config::open_default()?;
    config.set_str(&key, &value)?;
    Ok(value)
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_remove_global_config(key: String) -> Result<()> {
    let mut config = git2::Config::open_default()?;
    config.remove(&key)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_get_global_config(key: String) -> Result<Option<String>> {
    let config = git2::Config::open_default()?;
    let value = config.get_string(&key);
    match value {
        Ok(value) => Ok(Some(value)),
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                Ok(None)
            } else {
                Err(anyhow!(e))
            }
        }
    }
}
