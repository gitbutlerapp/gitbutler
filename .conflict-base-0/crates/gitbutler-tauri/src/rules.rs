use but_api::commands::rules;
use but_rules::{CreateRuleRequest, UpdateRuleRequest, WorkspaceRule};
use gitbutler_project::ProjectId;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn create_workspace_rule(
    project_id: ProjectId,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    rules::create_workspace_rule(project_id, request)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn delete_workspace_rule(project_id: ProjectId, id: String) -> Result<(), Error> {
    rules::delete_workspace_rule(project_id, id)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn update_workspace_rule(
    project_id: ProjectId,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule, Error> {
    rules::update_workspace_rule(project_id, request)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn list_workspace_rules(project_id: ProjectId) -> Result<Vec<WorkspaceRule>, Error> {
    rules::list_workspace_rules(project_id)
}
