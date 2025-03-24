use but_settings::AppSettingsWithDiskSync;
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use tauri::State;
use tracing::instrument;

use crate::virtual_branches::commands::emit_vbranches;
use crate::{error::Error, WindowState};

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn create_branch(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    request: CreateSeriesRequest,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::create_branch(&ctx, stack_id, request)?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn remove_branch(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::remove_branch(&ctx, stack_id, branch_name)?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn update_branch_name(
    windows: State<'_, WindowState>,
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
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn update_branch_description(
    windows: State<'_, WindowState>,
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
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn update_branch_pr_number(
    windows: State<'_, WindowState>,
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
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn push_stack(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    with_force: bool,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::push_stack(&ctx, stack_id, with_force)?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings, windows), err(Debug))]
pub fn push_stack_to_review(
    windows: State<'_, WindowState>,
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

    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(review_id)
}
