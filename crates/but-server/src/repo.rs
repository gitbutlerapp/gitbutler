use crate::RequestContext;
use anyhow::Context as _;
use but_workspace::DiffSpec;
use gitbutler_branch_actions::hooks;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use gitbutler_stack::BranchOwnershipClaims;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalConfigParams {
    id: ProjectId,
    key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetLocalConfigParams {
    id: ProjectId,
    key: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectParams {
    id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloneParams {
    repository_url: String,
    target_dir: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetFileParams {
    project_id: ProjectId,
    relative_path: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetCommitFileParams {
    project_id: ProjectId,
    relative_path: PathBuf,
    commit_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreCommitParams {
    project_id: ProjectId,
    ownership: BranchOwnershipClaims,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreCommitDiffSpecParams {
    project_id: ProjectId,
    changes: Vec<DiffSpec>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageHookParams {
    project_id: ProjectId,
    message: String,
}

pub fn git_get_local_config(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: LocalConfigParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.id)?;
    let result = project.get_local_config(&params.key)?;
    Ok(serde_json::to_value(result)?)
}

pub fn git_set_local_config(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: SetLocalConfigParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.id)?;
    project.set_local_config(&params.key, &params.value)?;
    Ok(serde_json::Value::Null)
}

pub fn check_signing_settings(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ProjectParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.id)?;
    let result = project.check_signing_settings()?;
    Ok(serde_json::to_value(result)?)
}

pub fn git_clone_repository(
    _ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: CloneParams = serde_json::from_value(params)?;
    let should_interrupt = AtomicBool::new(false);

    gix::prepare_clone(params.repository_url.as_str(), &params.target_dir)?
        .fetch_then_checkout(gix::progress::Discard, &should_interrupt)
        .map(|(checkout, _outcome)| checkout)?
        .main_worktree(gix::progress::Discard, &should_interrupt)?;
    Ok(serde_json::Value::Null)
}

pub fn get_uncommited_files(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ProjectParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.id)?;

    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let result = gitbutler_branch_actions::get_uncommited_files(&cmd_ctx)?;
    Ok(serde_json::to_value(result)?)
}

pub fn get_commit_file(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: GetCommitFileParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(anyhow::Error::from)?;
    let result = project.read_file_from_commit(commit_id, &params.relative_path)?;
    Ok(serde_json::to_value(result)?)
}

pub fn get_workspace_file(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: GetFileParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let result = project.read_file_from_workspace(&params.relative_path)?;
    Ok(serde_json::to_value(result)?)
}

pub fn pre_commit_hook(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: PreCommitParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let result = hooks::pre_commit(&cmd_ctx, &params.ownership)?;
    Ok(serde_json::to_value(result)?)
}

pub fn pre_commit_hook_diffspecs(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: PreCommitDiffSpecParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let repository = cmd_ctx.gix_repo()?;
    let head = repository
        .head_tree_id_or_empty()
        .context("Failed to get head tree")?;

    let context_lines = ctx.app_settings.get()?.context_lines;

    let mut changes = params.changes.into_iter().map(Ok).collect::<Vec<_>>();

    let (new_tree, ..) = but_workspace::commit_engine::apply_worktree_changes(
        head.detach(),
        &repository,
        &mut changes,
        context_lines,
    )?;

    let result = hooks::pre_commit_with_tree(&cmd_ctx, new_tree.to_git2())?;
    Ok(serde_json::to_value(result)?)
}

pub fn post_commit_hook(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ProjectParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.id)?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let result = gitbutler_repo::hooks::post_commit(&cmd_ctx)?;
    Ok(serde_json::to_value(result)?)
}

pub fn message_hook(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: MessageHookParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let result = gitbutler_repo::hooks::commit_msg(&cmd_ctx, params.message)?;
    Ok(serde_json::to_value(result)?)
}
