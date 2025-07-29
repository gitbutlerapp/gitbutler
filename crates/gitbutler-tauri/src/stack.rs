use but_settings::AppSettingsWithDiskSync;
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use tauri::State;
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn create_branch(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    request: CreateSeriesRequest,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::create_branch(&ctx, stack_id, request)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn remove_branch(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::remove_branch(&ctx, stack_id, branch_name)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn update_branch_name(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_name(&ctx, stack_id, branch_name, new_name)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn update_branch_description(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    description: Option<String>,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_description(
        &ctx,
        stack_id,
        branch_name,
        description,
    )?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn update_branch_pr_number(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_pr_number(
        &ctx,
        stack_id,
        branch_name,
        pr_number,
    )?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn push_stack(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    with_force: bool,
    branch: String,
) -> Result<PushResult, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::push_stack(&ctx, stack_id, with_force, branch)
        .map_err(|e| e.into())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings,), err(Debug))]
pub fn push_stack_to_review(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    top_branch: String,
    user: User,
) -> Result<String, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let review_id =
        gitbutler_sync::stack_upload::push_stack_to_review(&ctx, &user, stack_id, top_branch)?;

    Ok(review_id)
}
