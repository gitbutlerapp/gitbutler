//! In place of commands.rs
use anyhow::Result;
use but_api_macros::but_api;
use but_ctx::Context;
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, WorkspaceRule, create_rule, delete_rule, list_rules,
    update_rule,
};
use gitbutler_project::ProjectId;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn create_workspace_rule(
    project_id: ProjectId,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule> {
    let ctx = &mut Context::new_from_legacy_project_id(project_id)?;
    create_rule(ctx, request)
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_workspace_rule(project_id: ProjectId, id: String) -> Result<()> {
    let ctx = &mut Context::new_from_legacy_project_id(project_id)?;
    delete_rule(ctx, &id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_workspace_rule(
    project_id: ProjectId,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule> {
    let ctx = &mut Context::new_from_legacy_project_id(project_id)?;
    update_rule(ctx, request)
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_workspace_rules(project_id: ProjectId) -> Result<Vec<WorkspaceRule>> {
    let ctx = &mut Context::new_from_legacy_project_id(project_id)?;
    list_rules(ctx)
}
