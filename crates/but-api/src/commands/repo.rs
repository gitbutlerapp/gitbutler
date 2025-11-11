use std::{path::PathBuf, sync::atomic::AtomicBool};

use crate::error::{Error, ToError};
use anyhow::{Context as _, Result};
use but_api_macros::api_cmd;
use but_core::DiffSpec;
use but_graph::virtual_branches_legacy_types::BranchOwnershipClaims;
use but_oxidize::ObjectIdExt;
use but_settings::AppSettings;
use gitbutler_branch_actions::{RemoteBranchFile, hooks};
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_repo::{
    FileInfo, RepoCommands,
    hooks::{HookResult, MessageHookResult},
};
use tracing::instrument;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn git_get_local_config(project_id: ProjectId, key: String) -> Result<Option<String>, Error> {
    let project = gitbutler_project::get(project_id)?;
    Ok(project.get_local_config(&key)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn git_set_local_config(
    project_id: ProjectId,
    key: String,
    value: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    project.set_local_config(&key, &value).map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn check_signing_settings(project_id: ProjectId) -> Result<bool, Error> {
    let project = gitbutler_project::get(project_id)?;
    project.check_signing_settings().map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn git_clone_repository(repository_url: String, target_dir: PathBuf) -> Result<(), Error> {
    let should_interrupt = AtomicBool::new(false);

    gix::prepare_clone(repository_url.as_str(), &target_dir)
        .to_error()?
        .fetch_then_checkout(gix::progress::Discard, &should_interrupt)
        .map(|(checkout, _outcome)| checkout)
        .to_error()?
        .main_worktree(gix::progress::Discard, &should_interrupt)
        .to_error()?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_uncommited_files(project_id: ProjectId) -> Result<Vec<RemoteBranchFile>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(gitbutler_branch_actions::get_uncommited_files(&ctx)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_commit_file(
    project_id: ProjectId,
    relative_path: PathBuf,
    commit_id: String,
) -> Result<FileInfo, Error> {
    let project = gitbutler_project::get(project_id)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(anyhow::Error::from)?;
    Ok(project.read_file_from_commit(commit_id, &relative_path)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_workspace_file(
    project_id: ProjectId,
    relative_path: PathBuf,
) -> Result<FileInfo, Error> {
    let project = gitbutler_project::get(project_id)?;
    Ok(project.read_file_from_workspace(&relative_path)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn pre_commit_hook(
    project_id: ProjectId,
    ownership: BranchOwnershipClaims,
) -> Result<HookResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let claim = ownership.into();
    Ok(hooks::pre_commit(&ctx, &claim)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn pre_commit_hook_diffspecs(
    project_id: ProjectId,
    changes: Vec<DiffSpec>,
) -> Result<HookResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let repository = ctx.gix_repo()?;
    let head = repository
        .head_tree_id_or_empty()
        .context("Failed to get head tree")?;

    let context_lines = ctx.app_settings().context_lines;

    let mut changes = changes.into_iter().map(Ok).collect::<Vec<_>>();

    let (new_tree, ..) = but_core::tree::apply_worktree_changes(
        head.detach(),
        &repository,
        &mut changes,
        context_lines,
    )?;

    Ok(hooks::pre_commit_with_tree(&ctx, new_tree.to_git2())?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn post_commit_hook(project_id: ProjectId) -> Result<HookResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(gitbutler_repo::hooks::post_commit(&ctx)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn message_hook(project_id: ProjectId, message: String) -> Result<MessageHookResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(gitbutler_repo::hooks::commit_msg(&ctx, message)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn find_files(
    project_id: ProjectId,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<String>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let limit = limit.unwrap_or(10);
    Ok(project.find_files(&query, limit)?)
}
