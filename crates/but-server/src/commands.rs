use anyhow::Context;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_repo::RepositoryExt;
use gitbutler_repo_actions::RepoActionsExt;
use serde_json::{json, Value};

use crate::RequestContext;

pub fn git_remote_branches(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches = command_ctx.repo().remote_branches()?;
    Ok(serde_json::to_value(branches)?)
}

pub fn git_test_push(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let remote_name: String = serde_json::from_value(params.get("remoteName").cloned().unwrap_or_default())?;
    let branch_name: String = serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    command_ctx.git_test_push(&remote_name, &branch_name, Some(None))?;
    Ok(json!({}))
}

pub fn git_test_fetch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let remote_name: String = serde_json::from_value(params.get("remoteName").cloned().unwrap_or_default())?;
    let action: Option<String> = serde_json::from_value(params.get("action").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    command_ctx.fetch(&remote_name, Some(action.unwrap_or_else(|| "test".to_string())))?;
    Ok(json!({}))
}

pub fn git_index_size(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let size = command_ctx
        .repo()
        .index()
        .context("failed to get index size")?
        .len();
    Ok(json!(size))
}

pub fn git_head(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
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
    let key: String = serde_json::from_value(params.get("key").cloned().unwrap_or_default())?;
    let value: String = serde_json::from_value(params.get("value").cloned().unwrap_or_default())?;
    
    let mut config = git2::Config::open_default()?;
    config.set_str(&key, &value)?;
    Ok(json!(value))
}

pub fn git_remove_global_config(_ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let key: String = serde_json::from_value(params.get("key").cloned().unwrap_or_default())?;
    
    let mut config = git2::Config::open_default()?;
    config.remove(&key)?;
    Ok(json!({}))
}

pub fn git_get_global_config(_ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let key: String = serde_json::from_value(params.get("key").cloned().unwrap_or_default())?;
    
    let config = git2::Config::open_default()?;
    let value = config.get_string(&key);
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