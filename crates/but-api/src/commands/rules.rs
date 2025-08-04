//! In place of commands.rs
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, WorkspaceRule, create_rule, delete_rule, list_rules,
    update_rule,
};
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRuleParams {
    pub project_id: ProjectId,
    pub request: CreateRuleRequest,
}

pub fn create_workspace_rule(
    app: &App,
    params: CreateWorkspaceRuleParams,
) -> Result<WorkspaceRule, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(params.project_id)?,
        app.app_settings.get()?.clone(),
    )?;
    create_rule(ctx, params.request).map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWorkspaceRuleParams {
    pub project_id: ProjectId,
    pub id: String,
}

pub fn delete_workspace_rule(app: &App, params: DeleteWorkspaceRuleParams) -> Result<(), Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(params.project_id)?,
        app.app_settings.get()?.clone(),
    )?;
    delete_rule(ctx, &params.id).map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceRuleParams {
    pub project_id: ProjectId,
    pub request: UpdateRuleRequest,
}

pub fn update_workspace_rule(
    app: &App,
    params: UpdateWorkspaceRuleParams,
) -> Result<WorkspaceRule, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(params.project_id)?,
        app.app_settings.get()?.clone(),
    )?;
    update_rule(ctx, params.request).map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkspaceRulesParams {
    pub project_id: ProjectId,
}

pub fn list_workspace_rules(
    app: &App,
    params: ListWorkspaceRulesParams,
) -> Result<Vec<WorkspaceRule>, Error> {
    let ctx = &mut CommandContext::open(
        &gitbutler_project::get(params.project_id)?,
        app.app_settings.get()?.clone(),
    )?;
    list_rules(ctx).map_err(Into::into)
}
