use crate::RequestContext;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListActionsParams {
    project_id: ProjectId,
    offset: i64,
    limit: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandleChangesParams {
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListWorkflowsParams {
    project_id: ProjectId,
    offset: i64,
    limit: i64,
}

pub fn list_actions(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ListActionsParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = &mut CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let result = but_action::list_actions(cmd_ctx, params.offset, params.limit)?;
    Ok(serde_json::to_value(result)?)
}

pub fn handle_changes(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: HandleChangesParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = &mut CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let (_id, outcome) = but_action::handle_changes(
        cmd_ctx,
        &params.change_summary,
        None,
        params.handler,
        but_action::Source::GitButler,
    )?;
    Ok(serde_json::to_value(outcome)?)
}

pub fn list_workflows(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ListWorkflowsParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let cmd_ctx = &mut CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let result = but_action::list_workflows(cmd_ctx, params.offset, params.limit)?;
    Ok(serde_json::to_value(result)?)
}
