//! In place of commands.rs
use but_api_macros::api_cmd;
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, WorkspaceRule, create_rule, delete_rule, list_rules,
    update_rule,
};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;

use crate::error::Error;

#[api_cmd]
pub fn create_workspace_rule(
    project_id: ProjectId,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    create_rule(ctx, request).map_err(Into::into)
}

#[api_cmd]
pub fn delete_workspace_rule(project_id: ProjectId, id: String) -> Result<(), Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    delete_rule(ctx, &id).map_err(Into::into)
}

#[api_cmd]
pub fn update_workspace_rule(
    project_id: ProjectId,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    update_rule(ctx, request).map_err(Into::into)
}

#[api_cmd]
pub fn list_workspace_rules(project_id: ProjectId) -> Result<Vec<WorkspaceRule>, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(project_id)?,
        AppSettings::load_from_default_path_creating()?,
    )?;
    list_rules(ctx).map_err(Into::into)
}
