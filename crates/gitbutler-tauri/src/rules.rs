use crate::error::Error;
use but_rules::{
    create_rule, delete_rule, list_rules, update_rule, CreateRuleRequest, UpdateRuleRequest,
    WorkspaceRule,
};
use but_settings::AppSettingsWithDiskSync;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn create_workspace_rule(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        settings.get()?.clone(),
    )?;
    create_rule(ctx, request).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn delete_workspace_rule(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    id: String,
) -> Result<(), Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        settings.get()?.clone(),
    )?;
    delete_rule(ctx, &id).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn update_workspace_rule(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        settings.get()?.clone(),
    )?;
    update_rule(ctx, request).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn list_workspace_rules(
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<Vec<WorkspaceRule>, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        settings.get()?.clone(),
    )?;
    list_rules(ctx).map_err(Into::into)
}
