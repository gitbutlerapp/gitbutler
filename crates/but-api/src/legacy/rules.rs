//! In place of commands.rs
use std::str::FromStr;

use anyhow::Result;
use but_api_macros::but_api;
use but_ctx::Context;
use but_meta::VirtualBranchesTomlMetadata;
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, WorkspaceRule, create_rule, delete_rule, list_rules,
    update_rule,
};
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
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
    let repo = ctx.clone_repo_for_merging_non_persisting()?;

    let in_workspace = {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.legacy_project.gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::legacy::stacks_v3(
            &repo,
            &meta,
            but_workspace::legacy::StacksFilter::InWorkspace,
            None,
        )
    }?
    .iter()
    .filter_map(|s| s.id)
    .collect::<Vec<StackId>>();

    // Filter out specifically Codegen related rules that are refering to a stack that is not in the workspace.
    let rules = list_rules(ctx)?
        .into_iter()
        .filter(|rule| {
            if let (Some(_), Some(stack_id)) = (
                rule.session_id(),
                rule.target_stack_id()
                    .and_then(|id| StackId::from_str(&id).ok()),
            ) {
                return in_workspace.contains(&stack_id);
            }
            true
        })
        .collect();

    Ok(rules)
}
