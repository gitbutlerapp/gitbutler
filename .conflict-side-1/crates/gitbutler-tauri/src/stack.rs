use but_api::commands::stack;
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn create_reference(
    project_id: ProjectId,
    request: stack::create_reference::Request,
) -> Result<(), Error> {
    stack::create_reference(stack::create_reference::Params {
        project_id,
        request,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn create_branch(
    project_id: ProjectId,
    stack_id: StackId,
    request: CreateSeriesRequest,
) -> Result<(), Error> {
    stack::create_branch(stack::CreateBranchParams {
        project_id,
        stack_id,
        request,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn remove_branch(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
) -> Result<(), Error> {
    stack::remove_branch(stack::RemoveBranchParams {
        project_id,
        stack_id,
        branch_name,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn update_branch_name(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<(), Error> {
    stack::update_branch_name(stack::UpdateBranchNameParams {
        project_id,
        stack_id,
        branch_name,
        new_name,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn update_branch_description(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    description: Option<String>,
) -> Result<(), Error> {
    stack::update_branch_description(stack::UpdateBranchDescriptionParams {
        project_id,
        stack_id,
        branch_name,
        description,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn update_branch_pr_number(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<(), Error> {
    stack::update_branch_pr_number(stack::UpdateBranchPrNumberParams {
        project_id,
        stack_id,
        branch_name,
        pr_number,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn push_stack(
    project_id: ProjectId,
    stack_id: StackId,
    with_force: bool,
    skip_force_push_protection: bool,
    branch: String,
) -> Result<PushResult, Error> {
    stack::push_stack(stack::PushStackParams {
        project_id,
        stack_id,
        with_force,
        skip_force_push_protection,
        branch,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn push_stack_to_review(
    project_id: ProjectId,
    stack_id: StackId,
    top_branch: String,
    user: User,
) -> Result<String, Error> {
    stack::push_stack_to_review(stack::PushStackToReviewParams {
        project_id,
        stack_id,
        top_branch,
        user,
    })
}
