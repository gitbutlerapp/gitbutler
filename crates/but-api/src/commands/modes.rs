//! In place of commands.rs
use anyhow::Context;
use but_api_macros::api_cmd;
use but_core::ui::TreeChange;
use but_settings::AppSettings;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::{EditModeMetadata, OperatingMode};
use gitbutler_project::ProjectId;
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn operating_mode(project_id: ProjectId) -> Result<OperatingMode, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(gitbutler_operating_modes::operating_mode(&ctx))
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn enter_edit_mode(
    project_id: ProjectId,
    commit_id: String,
    stack_id: StackId,
) -> Result<EditModeMetadata, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit = git2::Oid::from_str(&commit_id).context("Failed to parse commit oid")?;

    gitbutler_edit_mode::commands::enter_edit_mode(&ctx, commit, stack_id).map_err(Into::into)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn abort_edit_and_return_to_workspace(project_id: ProjectId) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    gitbutler_edit_mode::commands::abort_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn save_edit_and_return_to_workspace(project_id: ProjectId) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    gitbutler_edit_mode::commands::save_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn edit_initial_index_state(
    project_id: ProjectId,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    gitbutler_edit_mode::commands::starting_index_state(&ctx).map_err(Into::into)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn edit_changes_from_initial(project_id: ProjectId) -> Result<Vec<TreeChange>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    gitbutler_edit_mode::commands::changes_from_initial(&ctx).map_err(Into::into)
}
