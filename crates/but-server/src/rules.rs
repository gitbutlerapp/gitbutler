use crate::RequestContext;
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, create_rule, delete_rule, list_rules, update_rule,
};
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRuleParams {
    project_id: ProjectId,
    request: CreateRuleRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteRuleParams {
    project_id: ProjectId,
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRuleParams {
    project_id: ProjectId,
    request: UpdateRuleRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListRulesParams {
    project_id: ProjectId,
}

pub fn create_workspace_rule(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: CreateRuleParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = &mut CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let rule = create_rule(cmd_ctx, params.request)?;
    Ok(serde_json::to_value(rule)?)
}

pub fn delete_workspace_rule(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: DeleteRuleParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = &mut CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    delete_rule(cmd_ctx, &params.id)?;
    Ok(serde_json::Value::Null)
}

pub fn update_workspace_rule(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: UpdateRuleParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = &mut CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let rule = update_rule(cmd_ctx, params.request)?;
    Ok(serde_json::to_value(rule)?)
}

pub fn list_workspace_rules(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ListRulesParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = &mut CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let rules = list_rules(cmd_ctx)?;
    Ok(serde_json::to_value(rules)?)
}
