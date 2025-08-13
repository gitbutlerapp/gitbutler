use but_api::{commands::rules, App};
use but_rules::{CreateRuleRequest, UpdateRuleRequest, WorkspaceRule};
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn create_workspace_rule(
    app: State<'_, App>,
    project_id: ProjectId,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    rules::create_workspace_rule(
        &app,
        rules::CreateWorkspaceRuleParams {
            project_id,
            request,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn delete_workspace_rule(
    app: State<'_, App>,
    project_id: ProjectId,
    id: String,
) -> Result<(), Error> {
    rules::delete_workspace_rule(&app, rules::DeleteWorkspaceRuleParams { project_id, id })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_workspace_rule(
    app: State<'_, App>,
    project_id: ProjectId,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    rules::update_workspace_rule(
        &app,
        rules::UpdateWorkspaceRuleParams {
            project_id,
            request,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn list_workspace_rules(
    app: State<'_, App>,
    project_id: ProjectId,
) -> Result<Vec<WorkspaceRule>, Error> {
    rules::list_workspace_rules(&app, rules::ListWorkspaceRulesParams { project_id })
}
