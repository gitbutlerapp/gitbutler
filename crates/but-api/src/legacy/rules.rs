//! In place of commands.rs
use std::str::FromStr;

use anyhow::Result;
use but_api_macros::but_api;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_rules::{
    CreateRuleRequest, UpdateRuleRequest, WorkspaceRule, create_rule, delete_rule, list_rules,
    update_rule,
};
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
pub fn delete_workspace_rule(ctx: &Context, rule_id: String) -> Result<()> {
    let mut db = ctx.db.get_cache_mut()?;
    delete_rule(&mut db, &rule_id)
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
pub fn list_workspace_rules(ctx: &Context) -> Result<Vec<WorkspaceRule>> {
    let in_workspace = crate::legacy::workspace::stacks_v3_from_ctx(
        ctx,
        but_workspace::legacy::StacksFilter::InWorkspace,
    )?
    .iter()
    .filter_map(|s| s.id)
    .collect::<Vec<StackId>>();

    // Filter out specifically Codegen related rules that are refering to a stack that is not in the workspace.
    let db = ctx.db.get_cache()?;
    let rules = list_rules(&db)?
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
