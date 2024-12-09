use anyhow::Context;
use gitbutler_branch_actions::RemoteBranchFile;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::EditModeMetadata;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_project::Controller;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use crate::error::Error;
use crate::virtual_branches::commands::emit_vbranches;
use crate::WindowState;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn operating_mode(
    projects: State<'_, Controller>,
    project_id: ProjectId,
) -> Result<OperatingMode, Error> {
    let project = projects.get(project_id)?;
    gitbutler_operating_modes::commands::operating_mode(&project).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn enter_edit_mode(
    projects: State<'_, Controller>,
    project_id: ProjectId,
    commit_oid: String,
    branch_reference: String,
) -> Result<EditModeMetadata, Error> {
    let project = projects.get(project_id)?;

    let commit = git2::Oid::from_str(&commit_oid).context("Failed to parse commit oid")?;

    gitbutler_edit_mode::commands::enter_edit_mode(&project, commit, branch_reference.into())
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(windows, projects), err(Debug))]
pub fn abort_edit_and_return_to_workspace(
    windows: State<'_, WindowState>,
    projects: State<'_, Controller>,
    project_id: ProjectId,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;

    gitbutler_edit_mode::commands::abort_and_return_to_workspace(&project)?;

    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(windows, projects), err(Debug))]
pub fn save_edit_and_return_to_workspace(
    windows: State<'_, WindowState>,
    projects: State<'_, Controller>,
    project_id: ProjectId,
) -> Result<(), Error> {
    let project = projects.get(project_id)?;

    gitbutler_edit_mode::commands::save_and_return_to_workspace(&project)?;

    emit_vbranches(&windows, project_id);
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn edit_initial_index_state(
    projects: State<'_, Controller>,
    project_id: ProjectId,
) -> Result<Vec<(RemoteBranchFile, Option<ConflictEntryPresence>)>, Error> {
    let project = projects.get(project_id)?;

    gitbutler_edit_mode::commands::starting_index_state(&project).map_err(Into::into)
}
