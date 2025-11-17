//! In place of commands.rs
use anyhow::Result;
use but_api_macros::api_cmd_tauri;
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, WorkspaceRule, create_rule, delete_rule, list_rules,
    update_rule,
};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tracing::instrument;

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn create_workspace_rule(
    project_id: ProjectId,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    create_rule(ctx, request)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn delete_workspace_rule(project_id: ProjectId, id: String) -> Result<()> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    delete_rule(ctx, &id)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn update_workspace_rule(
    project_id: ProjectId,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    update_rule(ctx, request)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn list_workspace_rules(project_id: ProjectId) -> Result<Vec<WorkspaceRule>> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    list_rules(ctx)
}
