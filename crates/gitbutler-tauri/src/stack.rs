use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_settings::AppSettingsWithDiskSync;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use tauri::State;
use tracing::instrument;

use crate::virtual_branches::commands::emit_vbranches;
use crate::{error::Error, WindowState};

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn create_series(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    branch_id: StackId,
    request: CreateSeriesRequest,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::create_series(&ctx, branch_id, request)?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn remove_series(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    branch_id: StackId,
    head_name: String,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::remove_series(&ctx, branch_id, head_name)?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn update_series_name(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    branch_id: StackId,
    head_name: String,
    new_head_name: String,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_series_name(&ctx, branch_id, head_name, new_head_name)?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn update_series_description(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    branch_id: StackId,
    head_name: String,
    description: Option<String>,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_series_description(
        &ctx,
        branch_id,
        head_name,
        description,
    )?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows, settings), err(Debug))]
pub fn update_series_pr_number(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    head_name: String,
    pr_number: Option<usize>,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_series_pr_number(&ctx, stack_id, head_name, pr_number)?;
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
    branch_id: StackId,
    with_force: bool,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_branch_actions::stack::push_stack(&ctx, branch_id, with_force)?;
    emit_vbranches(&windows, project_id, ctx.app_settings());
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn push_stack_to_review(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    user: User,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_sync::stack_upload::push_stack_to_review(&ctx, &user, stack_id)?;

    Ok(())
}
