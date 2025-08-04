use gitbutler_branch_actions::internal::PushResult;
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub request: CreateSeriesRequest,
}

pub fn create_branch(app: &App, params: CreateBranchParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::create_branch(&ctx, params.stack_id, params.request)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveBranchParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
}

pub fn remove_branch(app: &App, params: RemoveBranchParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::remove_branch(&ctx, params.stack_id, params.branch_name)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBranchNameParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
    pub new_name: String,
}

pub fn update_branch_name(app: &App, params: UpdateBranchNameParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_name(
        &ctx,
        params.stack_id,
        params.branch_name,
        params.new_name,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBranchDescriptionParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
    pub description: Option<String>,
}

pub fn update_branch_description(
    app: &App,
    params: UpdateBranchDescriptionParams,
) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_description(
        &ctx,
        params.stack_id,
        params.branch_name,
        params.description,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBranchPrNumberParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
    pub pr_number: Option<usize>,
}

pub fn update_branch_pr_number(app: &App, params: UpdateBranchPrNumberParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_pr_number(
        &ctx,
        params.stack_id,
        params.branch_name,
        params.pr_number,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushStackParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub with_force: bool,
    pub branch: String,
}

pub fn push_stack(app: &App, params: PushStackParams) -> Result<PushResult, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::push_stack(
        &ctx,
        params.stack_id,
        params.with_force,
        params.branch,
    )
    .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushStackToReviewParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub top_branch: String,
    pub user: User,
}

pub fn push_stack_to_review(app: &App, params: PushStackToReviewParams) -> Result<String, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let review_id = gitbutler_sync::stack_upload::push_stack_to_review(
        &ctx,
        &params.user,
        params.stack_id,
        params.top_branch,
    )?;

    Ok(review_id)
}
