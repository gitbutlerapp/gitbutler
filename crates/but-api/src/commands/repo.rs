use anyhow::{Context as _, Result};
use but_graph::virtual_branches_legacy_types::BranchOwnershipClaims;
use but_workspace::DiffSpec;
use gitbutler_branch_actions::{RemoteBranchFile, hooks};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::ProjectId;
use gitbutler_repo::hooks::{HookResult, MessageHookResult};
use gitbutler_repo::{FileInfo, RepoCommands};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::error::ToError;
use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGetLocalConfigParams {
    pub project_id: ProjectId,
    pub key: String,
}

pub fn git_get_local_config(
    _app: &App,
    params: GitGetLocalConfigParams,
) -> Result<Option<String>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    Ok(project.get_local_config(&params.key)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSetLocalConfigParams {
    pub project_id: ProjectId,
    pub key: String,
    pub value: String,
}

pub fn git_set_local_config(_app: &App, params: GitSetLocalConfigParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    project
        .set_local_config(&params.key, &params.value)
        .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckSigningSettingsParams {
    pub project_id: ProjectId,
}

pub fn check_signing_settings(
    _app: &App,
    params: CheckSigningSettingsParams,
) -> Result<bool, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    project.check_signing_settings().map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCloneRepositoryParams {
    pub repository_url: String,
    pub target_dir: PathBuf,
}

pub fn git_clone_repository(_app: &App, params: GitCloneRepositoryParams) -> Result<(), Error> {
    let should_interrupt = AtomicBool::new(false);

    gix::prepare_clone(params.repository_url.as_str(), &params.target_dir)
        .to_error()?
        .fetch_then_checkout(gix::progress::Discard, &should_interrupt)
        .map(|(checkout, _outcome)| checkout)
        .to_error()?
        .main_worktree(gix::progress::Discard, &should_interrupt)
        .to_error()?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUncommittedFilesParams {
    pub project_id: ProjectId,
}

pub fn get_uncommitted_files(
    app: &App,
    params: GetUncommittedFilesParams,
) -> Result<Vec<RemoteBranchFile>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    Ok(gitbutler_branch_actions::get_uncommited_files(&ctx)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCommitFileParams {
    pub project_id: ProjectId,
    pub relative_path: PathBuf,
    pub commit_id: String,
}

pub fn get_commit_file(_app: &App, params: GetCommitFileParams) -> Result<FileInfo, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(anyhow::Error::from)?;
    Ok(project.read_file_from_commit(commit_id, &params.relative_path)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWorkspaceFileParams {
    pub project_id: ProjectId,
    pub relative_path: PathBuf,
}

pub fn get_workspace_file(_app: &App, params: GetWorkspaceFileParams) -> Result<FileInfo, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    Ok(project.read_file_from_workspace(&params.relative_path)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreCommitHookParams {
    pub project_id: ProjectId,
    pub ownership: BranchOwnershipClaims,
}

pub fn pre_commit_hook(app: &App, params: PreCommitHookParams) -> Result<HookResult, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let claim = params.ownership.into();
    Ok(hooks::pre_commit(&ctx, &claim)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreCommitHookDiffspecsParams {
    pub project_id: ProjectId,
    pub changes: Vec<DiffSpec>,
}

pub fn pre_commit_hook_diffspecs(
    app: &App,
    params: PreCommitHookDiffspecsParams,
) -> Result<HookResult, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;

    let repository = ctx.gix_repo()?;
    let head = repository
        .head_tree_id_or_empty()
        .context("Failed to get head tree")?;

    let context_lines = app.app_settings.get()?.context_lines;

    let mut changes = params.changes.into_iter().map(Ok).collect::<Vec<_>>();

    let (new_tree, ..) = but_workspace::commit_engine::apply_worktree_changes(
        head.detach(),
        &repository,
        &mut changes,
        context_lines,
    )?;

    Ok(hooks::pre_commit_with_tree(&ctx, new_tree.to_git2())?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostCommitHookParams {
    pub project_id: ProjectId,
}

pub fn post_commit_hook(app: &App, params: PostCommitHookParams) -> Result<HookResult, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    Ok(gitbutler_repo::hooks::post_commit(&ctx)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageHookParams {
    pub project_id: ProjectId,
    pub message: String,
}

pub fn message_hook(app: &App, params: MessageHookParams) -> Result<MessageHookResult, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    Ok(gitbutler_repo::hooks::commit_msg(&ctx, params.message)?)
}
