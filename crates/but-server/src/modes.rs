use anyhow::Context;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_stack::VirtualBranchesHandle;
use serde_json::{json, Value};

use crate::RequestContext;

pub fn operating_mode(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mode = gitbutler_operating_modes::operating_mode(&command_ctx);
    Ok(serde_json::to_value(mode)?)
}

pub fn enter_edit_mode(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let commit_id: String = serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let handle = VirtualBranchesHandle::new(project.gb_dir());
    let stack = handle.get_stack(stack_id)?;

    let commit = git2::Oid::from_str(&commit_id).context("Failed to parse commit oid")?;

    let metadata = gitbutler_edit_mode::commands::enter_edit_mode(
        &command_ctx,
        commit,
        stack.refname()?.to_string().into(),
    )?;
    
    Ok(serde_json::to_value(metadata)?)
}

pub fn abort_edit_and_return_to_workspace(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    gitbutler_edit_mode::commands::abort_and_return_to_workspace(&command_ctx)?;

    Ok(json!({}))
}

pub fn save_edit_and_return_to_workspace(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    gitbutler_edit_mode::commands::save_and_return_to_workspace(&command_ctx)?;

    Ok(json!({}))
}

pub fn edit_initial_index_state(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let state = gitbutler_edit_mode::commands::starting_index_state(&command_ctx)?;
    Ok(serde_json::to_value(state)?)
}