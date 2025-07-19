use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use serde_json::{json, Value};

use crate::RequestContext;

pub fn create_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let request: CreateSeriesRequest = serde_json::from_value(params.get("request").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::create_branch(&command_ctx, stack_id, request)?;
    Ok(json!({}))
}

pub fn remove_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let branch_name: String = serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::remove_branch(&command_ctx, stack_id, branch_name)?;
    Ok(json!({}))
}

pub fn update_branch_name(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let branch_name: String = serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;
    let new_name: String = serde_json::from_value(params.get("newName").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_name(&command_ctx, stack_id, branch_name, new_name)?;
    Ok(json!({}))
}

pub fn update_branch_description(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let branch_name: String = serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;
    let description: Option<String> = serde_json::from_value(params.get("description").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_description(
        &command_ctx,
        stack_id,
        branch_name,
        description,
    )?;
    Ok(json!({}))
}

pub fn update_branch_pr_number(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let branch_name: String = serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;
    let pr_number: Option<usize> = serde_json::from_value(params.get("prNumber").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_pr_number(
        &command_ctx,
        stack_id,
        branch_name,
        pr_number,
    )?;
    Ok(json!({}))
}

pub fn push_stack(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let with_force: bool = serde_json::from_value(params.get("withForce").cloned().unwrap_or_default())?;
    let branch: String = serde_json::from_value(params.get("branch").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::push_stack(&command_ctx, stack_id, with_force, branch)?;
    Ok(json!({}))
}

pub fn push_stack_to_review(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let top_branch: String = serde_json::from_value(params.get("topBranch").cloned().unwrap_or_default())?;
    let user: User = serde_json::from_value(params.get("user").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let review_id = gitbutler_sync::stack_upload::push_stack_to_review(&command_ctx, &user, stack_id, top_branch)?;

    Ok(json!(review_id))
}