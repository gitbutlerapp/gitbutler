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
use gitbutler_stack::StackId;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn create_workspace_rule(
    ctx: &mut Context,
    request: CreateRuleRequest,
) -> Result<WorkspaceRule> {
    let guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?.clone();
    let (_, workspace) = ctx.workspace_and_read_only_meta_from_head(guard.read_permission())?;
    create_rule(ctx, &repo, &workspace, request)
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_workspace_rule(ctx: &mut Context, id: String) -> Result<()> {
    delete_rule(ctx, &id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_workspace_rule(
    ctx: &mut Context,
    request: UpdateRuleRequest,
) -> Result<WorkspaceRule> {
    let guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?.clone();
    let (_, workspace) = ctx.workspace_and_read_only_meta_from_head(guard.read_permission())?;
    update_rule(ctx, &repo, &workspace, request)
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_workspace_rules(ctx: &mut Context) -> Result<Vec<WorkspaceRule>> {
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
