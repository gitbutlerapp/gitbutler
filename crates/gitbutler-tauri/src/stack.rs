use but_api::{commands::stack, App};
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn create_reference(
    app: State<App>,
    project_id: ProjectId,
    request: stack::create_reference::Request,
) -> Result<(), Error> {
    stack::create_reference(
        &app,
        stack::create_reference::Params {
            project_id,
            request,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn create_branch(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    request: CreateSeriesRequest,
) -> Result<(), Error> {
    stack::create_branch(
        &app,
        stack::CreateBranchParams {
            project_id,
            stack_id,
            request,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn remove_branch(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
) -> Result<(), Error> {
    stack::remove_branch(
        &app,
        stack::RemoveBranchParams {
            project_id,
            stack_id,
            branch_name,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_branch_name(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<(), Error> {
    stack::update_branch_name(
        &app,
        stack::UpdateBranchNameParams {
            project_id,
            stack_id,
            branch_name,
            new_name,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_branch_description(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    description: Option<String>,
) -> Result<(), Error> {
    stack::update_branch_description(
        &app,
        stack::UpdateBranchDescriptionParams {
            project_id,
            stack_id,
            branch_name,
            description,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_branch_pr_number(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<(), Error> {
    stack::update_branch_pr_number(
        &app,
        stack::UpdateBranchPrNumberParams {
            project_id,
            stack_id,
            branch_name,
            pr_number,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn push_stack(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    with_force: bool,
    branch: String,
) -> Result<PushResult, Error> {
    stack::push_stack(
        &app,
        stack::PushStackParams {
            project_id,
            stack_id,
            with_force,
            branch,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn push_stack_to_review(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    top_branch: String,
    user: User,
) -> Result<String, Error> {
    stack::push_stack_to_review(
        &app,
        stack::PushStackToReviewParams {
            project_id,
            stack_id,
            top_branch,
            user,
        },
    )
}
