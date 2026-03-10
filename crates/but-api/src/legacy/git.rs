//! In place of commands.rs
use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_core::git_config::{
    open_user_global_config_for_editing, remove_config_value, set_config_value, write_config,
};
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_repo_actions::RepoActionsExt as _;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn git_remote_branches(ctx: &but_ctx::Context) -> Result<Vec<RemoteRefname>> {
    let repo = ctx.git2_repo.get()?;
    repo.remote_branches()
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_test_push(
    ctx: &but_ctx::Context,
    remote_name: String,
    branch_name: String,
) -> Result<()> {
    ctx.git_test_push(&remote_name, &branch_name, Some(None))?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_test_fetch(
    ctx: &but_ctx::Context,
    remote_name: String,
    action: Option<String>,
) -> Result<()> {
    ctx.fetch(
        &remote_name,
        Some(action.unwrap_or_else(|| "test".to_string())),
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_index_size(ctx: &but_ctx::Context) -> Result<usize> {
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
    let (mut config, path) = open_user_global_config_for_editing()?;
    set_config_value(&mut config, &key, &value)?;
    write_config(&path, &config)?;
    Ok(value)
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_remove_global_config(key: String) -> Result<()> {
    let (mut config, path) = open_user_global_config_for_editing()?;
    remove_config_value(&mut config, &key)?;
    write_config(&path, &config)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_get_global_config(key: String) -> Result<Option<String>> {
    let (config, _) = open_user_global_config_for_editing()?;
    Ok(get_config_string(&config, &key))
}

fn get_config_string(config: &gix::config::File<'_>, key: &str) -> Option<String> {
    config.string(key).map(|s| s.to_string())
}
