//! In place of commands.rs
use anyhow::Context;
use but_core::ui::TreeChange;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_edit_mode::ConflictEntryPresence;
use gitbutler_operating_modes::{EditModeMetadata, OperatingMode};
use gitbutler_project::ProjectId;
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatingModeParams {
    pub project_id: ProjectId,
}

pub fn operating_mode(app: &App, params: OperatingModeParams) -> Result<OperatingMode, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    Ok(gitbutler_operating_modes::operating_mode(&ctx))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterEditModeParams {
    pub project_id: ProjectId,
    pub commit_id: String,
    pub stack_id: StackId,
}

pub fn enter_edit_mode(app: &App, params: EnterEditModeParams) -> Result<EditModeMetadata, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit = git2::Oid::from_str(&params.commit_id).context("Failed to parse commit oid")?;

    gitbutler_edit_mode::commands::enter_edit_mode(&ctx, commit, params.stack_id)
        .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbortEditAndReturnToWorkspaceParams {
    pub project_id: ProjectId,
}

pub fn abort_edit_and_return_to_workspace(
    app: &App,
    params: AbortEditAndReturnToWorkspaceParams,
) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;

    gitbutler_edit_mode::commands::abort_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEditAndReturnToWorkspaceParams {
    pub project_id: ProjectId,
}

pub fn save_edit_and_return_to_workspace(
    app: &App,
    params: SaveEditAndReturnToWorkspaceParams,
) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;

    gitbutler_edit_mode::commands::save_and_return_to_workspace(&ctx)?;

    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditInitialIndexStateParams {
    pub project_id: ProjectId,
}

pub fn edit_initial_index_state(
    app: &App,
    params: EditInitialIndexStateParams,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;

    gitbutler_edit_mode::commands::starting_index_state(&ctx).map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditChangesFromInitialParams {
    pub project_id: ProjectId,
}

pub fn edit_changes_from_initial(
    app: &App,
    params: EditChangesFromInitialParams,
) -> Result<Vec<TreeChange>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;

    gitbutler_edit_mode::commands::changes_from_initial(&ctx).map_err(Into::into)
}
