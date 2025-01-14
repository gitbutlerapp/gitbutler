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
#[instrument(skip(projects, windows), err(Debug))]
pub fn create_series(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    branch_id: StackId,
    request: CreateSeriesRequest,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    gitbutler_branch_actions::stack::create_series(&project, branch_id, request)?;
    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows), err(Debug))]
pub fn remove_series(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    branch_id: StackId,
    head_name: String,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    gitbutler_branch_actions::stack::remove_series(&project, branch_id, head_name)?;
    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows), err(Debug))]
pub fn update_series_name(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    branch_id: StackId,
    head_name: String,
    new_head_name: String,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    gitbutler_branch_actions::stack::update_series_name(
        &project,
        branch_id,
        head_name,
        new_head_name,
    )?;
    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows), err(Debug))]
pub fn update_series_description(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    branch_id: StackId,
    head_name: String,
    description: Option<String>,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    gitbutler_branch_actions::stack::update_series_description(
        &project,
        branch_id,
        head_name,
        description,
    )?;
    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows), err(Debug))]
pub fn update_series_pr_number(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    stack_id: StackId,
    head_name: String,
    pr_number: Option<usize>,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    gitbutler_branch_actions::stack::update_series_pr_number(
        &project, stack_id, head_name, pr_number,
    )?;
    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects, windows), err(Debug))]
pub fn push_stack(
    windows: State<'_, WindowState>,
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    branch_id: StackId,
    with_force: bool,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    gitbutler_branch_actions::stack::push_stack(&project, branch_id, with_force)?;
    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn push_stack_to_review(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    stack_id: StackId,
    user: User,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project)?;
    gitbutler_sync::stack_upload::push_stack_to_review(&ctx, &user, stack_id)?;

    Ok(())
}
