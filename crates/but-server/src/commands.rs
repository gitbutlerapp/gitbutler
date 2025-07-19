use anyhow::Context;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_repo::RepositoryExt;
use gitbutler_repo_actions::RepoActionsExt;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::RequestContext;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitRemoteBranchesParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitTestPushParams {
    project_id: ProjectId,
    remote_name: String,
    branch_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitTestFetchParams {
    project_id: ProjectId,
    remote_name: String,
    action: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitIndexSizeParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHeadParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitSetGlobalConfigParams {
    key: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitRemoveGlobalConfigParams {
    key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitGetGlobalConfigParams {
    key: String,
}

pub fn git_remote_branches(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitRemoteBranchesParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches = command_ctx.repo().remote_branches()?;
    Ok(serde_json::to_value(branches)?)
}

pub fn git_test_push(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitTestPushParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    command_ctx.git_test_push(&params.remote_name, &params.branch_name, Some(None))?;
    Ok(json!({}))
}

pub fn git_test_fetch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitTestFetchParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    command_ctx.fetch(&params.remote_name, Some(params.action.unwrap_or_else(|| "test".to_string())))?;
    Ok(json!({}))
}

pub fn git_index_size(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitIndexSizeParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let size = command_ctx
        .repo()
        .index()
        .context("failed to get index size")?
        .len();
    Ok(json!(size))
}

pub fn git_head(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitHeadParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let head = command_ctx.repo().head().context("failed to get repository head")?;
    Ok(json!(head.name().unwrap().to_string()))
}

pub fn delete_all_data(ctx: &RequestContext, _params: Value) -> anyhow::Result<Value> {
    for project in ctx.project_controller.list().context("failed to list projects")? {
        ctx.project_controller
            .delete(project.id)
            .map_err(|err| err.context("failed to delete project"))?;
    }
    Ok(json!({}))
}

pub fn git_set_global_config(_ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitSetGlobalConfigParams = serde_json::from_value(params)?;
    
    let mut config = git2::Config::open_default()?;
    config.set_str(&params.key, &params.value)?;
    Ok(json!(params.value))
}

pub fn git_remove_global_config(_ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitRemoveGlobalConfigParams = serde_json::from_value(params)?;
    
    let mut config = git2::Config::open_default()?;
    config.remove(&params.key)?;
    Ok(json!({}))
}

pub fn git_get_global_config(_ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GitGetGlobalConfigParams = serde_json::from_value(params)?;
    
    let config = git2::Config::open_default()?;
    let value = config.get_string(&params.key);
    match value {
        Ok(value) => Ok(json!(Some(value))),
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                Ok(json!(null))
            } else {
                Err(e.into())
            }
        }
    }
}