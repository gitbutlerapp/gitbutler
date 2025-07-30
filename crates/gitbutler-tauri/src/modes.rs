use anyhow::Context;
use but_core::ui::TreeChange;
use but_settings::AppSettingsWithDiskSync;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::EditModeMetadata;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_project::ProjectId;
use gitbutler_stack::VirtualBranchesHandle;
use tauri::State;
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn operating_mode(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<OperatingMode, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    Ok(gitbutler_operating_modes::operating_mode(&ctx))
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn enter_edit_mode(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    commit_id: String,
    stack_id: StackId,
) -> Result<EditModeMetadata, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let handle = VirtualBranchesHandle::new(project.gb_dir());
    let stack = handle.get_stack(stack_id)?;

    let commit = git2::Oid::from_str(&commit_id).context("Failed to parse commit oid")?;

    gitbutler_edit_mode::commands::enter_edit_mode(
        &ctx,
        commit,
        stack.refname()?.to_string().into(),
    )
    .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn abort_edit_and_return_to_workspace(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;

    gitbutler_edit_mode::commands::abort_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn save_edit_and_return_to_workspace(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;

    gitbutler_edit_mode::commands::save_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn edit_initial_index_state(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;

    gitbutler_edit_mode::commands::starting_index_state(&ctx).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn edit_changes_from_initial(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<Vec<TreeChange>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;

    gitbutler_edit_mode::commands::changes_from_initial(&ctx).map_err(Into::into)
}
