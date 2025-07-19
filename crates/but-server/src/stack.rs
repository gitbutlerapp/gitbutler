use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::RequestContext;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBranchParams {
    project_id: ProjectId,
    stack_id: StackId,
    request: CreateSeriesRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveBranchParams {
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBranchNameParams {
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBranchDescriptionParams {
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBranchPrNumberParams {
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushStackParams {
    project_id: ProjectId,
    stack_id: StackId,
    with_force: bool,
    branch: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushStackToReviewParams {
    project_id: ProjectId,
    stack_id: StackId,
    top_branch: String,
    user: User,
}

pub fn create_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: CreateBranchParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::create_branch(&command_ctx, params.stack_id, params.request)?;
    Ok(json!({}))
}

pub fn remove_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: RemoveBranchParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::remove_branch(&command_ctx, params.stack_id, params.branch_name)?;
    Ok(json!({}))
}

pub fn update_branch_name(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UpdateBranchNameParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_name(&command_ctx, params.stack_id, params.branch_name, params.new_name)?;
    Ok(json!({}))
}

pub fn update_branch_description(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UpdateBranchDescriptionParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_description(
        &command_ctx,
        params.stack_id,
        params.branch_name,
        params.description,
    )?;
    Ok(json!({}))
}

pub fn update_branch_pr_number(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UpdateBranchPrNumberParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_pr_number(
        &command_ctx,
        params.stack_id,
        params.branch_name,
        params.pr_number,
    )?;
    Ok(json!({}))
}

pub fn push_stack(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: PushStackParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::push_stack(&command_ctx, params.stack_id, params.with_force, params.branch)?;
    Ok(json!({}))
}

pub fn push_stack_to_review(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: PushStackToReviewParams = serde_json::from_value(params)?;
    
    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let review_id = gitbutler_sync::stack_upload::push_stack_to_review(&command_ctx, &params.user, params.stack_id, params.top_branch)?;

    Ok(json!(review_id))
}