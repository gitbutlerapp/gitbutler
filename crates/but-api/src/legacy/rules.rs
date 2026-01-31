//! In place of commands.rs
use std::str::FromStr;

use anyhow::Result;
use but_api_macros::but_api;
use but_ctx::Context;
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, WorkspaceRule, create_rule, delete_rule, list_rules,
    update_rule,
};
use gitbutler_stack::StackId;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn create_workspace_rule(
    ctx: &mut Context,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule> {
    let mut guard = ctx.exclusive_worktree_access();
    create_rule(ctx, request, guard.write_permission())
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_workspace_rule(ctx: &mut Context, rule_id: String) -> Result<()> {
    delete_rule(ctx, &rule_id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_workspace_rule(
    ctx: &mut Context,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule> {
    let mut guard = ctx.exclusive_worktree_access();
    update_rule(ctx, request, guard.write_permission())
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_workspace_rules(ctx: &mut Context) -> Result<Vec<WorkspaceRule>> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;

    let in_workspace = {
        let meta = ctx.legacy_meta()?;
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
