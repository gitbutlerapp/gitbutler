use anyhow::{Context, anyhow};
use but_workspace::DiffSpec;
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::branch_upstream_integration::IntegrationStrategy;
use gitbutler_branch_actions::upstream_integration::{
    BaseBranchResolution, BaseBranchResolutionApproach, Resolution,
};
use gitbutler_branch_actions::{BranchListingFilter, StackOrder};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::{FetchResult, ProjectId};
use gitbutler_reference::{Refname, RemoteRefname, normalize_branch_name as normalize_name};
use gitbutler_stack::{BranchOwnershipClaims, StackId, VirtualBranchesHandle};
use serde_json::{Value, json};

use crate::RequestContext;

pub fn normalize_branch_name(params: Value) -> anyhow::Result<Value> {
    let name: String = serde_json::from_value(params.get("name").cloned().unwrap_or_default())?;
    let normalized = normalize_name(&name)?;
    Ok(json!(normalized))
}

pub fn create_virtual_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch: BranchCreateRequest =
        serde_json::from_value(params.get("branch").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        &command_ctx,
        &branch,
        command_ctx
            .project()
            .exclusive_worktree_access()
            .write_permission(),
    )?;
    Ok(serde_json::to_value(stack_entry)?)
}

pub fn delete_local_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let refname: Refname =
        serde_json::from_value(params.get("refname").cloned().unwrap_or_default())?;
    let given_name: String =
        serde_json::from_value(params.get("givenName").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::delete_local_branch(&command_ctx, &refname, given_name)?;
    Ok(json!({}))
}

pub fn create_virtual_branch_from_branch(
    ctx: &RequestContext,
    params: Value,
) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch: Refname =
        serde_json::from_value(params.get("branch").cloned().unwrap_or_default())?;
    let remote: Option<RemoteRefname> =
        serde_json::from_value(params.get("remote").cloned().unwrap_or_default())?;
    let pr_number: Option<usize> =
        serde_json::from_value(params.get("prNumber").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        &command_ctx,
        &branch,
        remote,
        pr_number,
    )?;
    Ok(serde_json::to_value(branch_id)?)
}

pub fn integrate_upstream_commits(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let series_name: String =
        serde_json::from_value(params.get("seriesName").cloned().unwrap_or_default())?;
    let integration_strategy: Option<IntegrationStrategy> = serde_json::from_value(
        params
            .get("integrationStrategy")
            .cloned()
            .unwrap_or_default(),
    )?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::integrate_upstream_commits(
        &command_ctx,
        stack_id,
        series_name,
        integration_strategy,
    )?;
    Ok(json!({}))
}

pub fn get_base_branch_data(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    if let Ok(base_branch) = gitbutler_branch_actions::base::get_base_branch_data(&command_ctx) {
        Ok(serde_json::to_value(Some(base_branch))?)
    } else {
        Ok(json!(null))
    }
}

pub fn set_base_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch: String = serde_json::from_value(params.get("branch").cloned().unwrap_or_default())?;
    let push_remote: Option<String> =
        serde_json::from_value(params.get("pushRemote").cloned().unwrap_or_default())?;
    let stash_uncommitted: Option<bool> =
        serde_json::from_value(params.get("stashUncommitted").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branch_name = format!("refs/remotes/{}", branch)
        .parse()
        .context("Invalid branch name")?;
    let base_branch = gitbutler_branch_actions::set_base_branch(
        &command_ctx,
        &branch_name,
        stash_uncommitted.unwrap_or_default(),
        command_ctx
            .project()
            .exclusive_worktree_access()
            .write_permission(),
    )?;

    if let Some(push_remote) = push_remote {
        gitbutler_branch_actions::set_target_push_remote(&command_ctx, &push_remote)?;
    }
    Ok(serde_json::to_value(base_branch)?)
}

pub fn push_base_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let with_force: bool =
        serde_json::from_value(params.get("withForce").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::push_base_branch(&command_ctx, with_force)?;
    Ok(json!({}))
}

pub fn update_stack_order(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stacks: Vec<BranchUpdateRequest> =
        serde_json::from_value(params.get("stacks").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::update_stack_order(&command_ctx, stacks)?;
    Ok(json!({}))
}

pub async fn unapply_stack(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let mut command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        &mut command_ctx,
        false,
        Some(but_core::diff::ui::worktree_changes_by_worktree_dir(project.path)?.changes),
        None,
    )?;
    let assigned_diffspec = but_workspace::flatten_diff_specs(
        assignments
            .into_iter()
            .filter(|a| a.stack_id == Some(stack_id))
            .map(|a| a.into())
            .collect::<Vec<DiffSpec>>(),
    );
    gitbutler_branch_actions::unapply_stack(&command_ctx, stack_id, assigned_diffspec)?;
    Ok(json!({}))
}

pub fn can_apply_remote_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch: RemoteRefname =
        serde_json::from_value(params.get("branch").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let can_apply = gitbutler_branch_actions::can_apply_remote_branch(&command_ctx, &branch)?;
    Ok(json!(can_apply))
}

pub fn list_commit_files(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let commit_id: String =
        serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let files = gitbutler_branch_actions::list_commit_files(&command_ctx, commit_id)?;
    Ok(serde_json::to_value(files)?)
}

pub fn amend_virtual_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let commit_id: String =
        serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;
    let worktree_changes: Vec<DiffSpec> =
        serde_json::from_value(params.get("worktreeChanges").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let oid = gitbutler_branch_actions::amend(&command_ctx, stack_id, commit_id, worktree_changes)?;
    Ok(json!(oid.to_string()))
}

pub fn move_commit_file(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let from_commit_id: String =
        serde_json::from_value(params.get("fromCommitId").cloned().unwrap_or_default())?;
    let to_commit_id: String =
        serde_json::from_value(params.get("toCommitId").cloned().unwrap_or_default())?;
    let ownership: BranchOwnershipClaims =
        serde_json::from_value(params.get("ownership").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let from_commit_id = git2::Oid::from_str(&from_commit_id).map_err(|e| anyhow!(e))?;
    let to_commit_id = git2::Oid::from_str(&to_commit_id).map_err(|e| anyhow!(e))?;
    let oid = gitbutler_branch_actions::move_commit_file(
        &command_ctx,
        stack_id,
        from_commit_id,
        to_commit_id,
        &ownership,
    )?;
    Ok(json!(oid.to_string()))
}

pub fn undo_commit(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let commit_id: String =
        serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::undo_commit(&command_ctx, stack_id, commit_id)?;
    Ok(json!({}))
}

pub fn insert_blank_commit(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let commit_id: Option<String> =
        serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;
    let offset: i32 = serde_json::from_value(params.get("offset").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = match commit_id {
        Some(oid) => git2::Oid::from_str(&oid).map_err(|e| anyhow!(e))?,
        None => {
            let state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
            let stack = state.get_stack(stack_id)?;
            let gix_repo = command_ctx.gix_repo()?;
            stack.head_oid(&gix_repo)?.to_git2()
        }
    };
    gitbutler_branch_actions::insert_blank_commit(&command_ctx, stack_id, commit_id, offset, None)?;
    Ok(json!({}))
}

pub fn reorder_stack(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let stack_order: StackOrder =
        serde_json::from_value(params.get("stackOrder").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::reorder_stack(&command_ctx, stack_id, stack_order)?;
    Ok(json!({}))
}

pub fn find_git_branches(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch_name: String =
        serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches = gitbutler_branch_actions::find_git_branches(&command_ctx, &branch_name)?;
    Ok(serde_json::to_value(branches)?)
}

pub fn list_branches(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let filter: Option<BranchListingFilter> =
        serde_json::from_value(params.get("filter").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches = gitbutler_branch_actions::list_branches(&command_ctx, filter, None)?;
    Ok(serde_json::to_value(branches)?)
}

pub fn get_branch_listing_details(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch_names: Vec<String> =
        serde_json::from_value(params.get("branchNames").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches =
        gitbutler_branch_actions::get_branch_listing_details(&command_ctx, branch_names)?;
    Ok(serde_json::to_value(branches)?)
}

pub fn squash_commits(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let source_commit_ids: Vec<String> =
        serde_json::from_value(params.get("sourceCommitIds").cloned().unwrap_or_default())?;
    let target_commit_id: String =
        serde_json::from_value(params.get("targetCommitId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let source_commit_ids: Vec<git2::Oid> = source_commit_ids
        .into_iter()
        .map(|oid| git2::Oid::from_str(&oid))
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow!(e))?;
    let destination_commit_id = git2::Oid::from_str(&target_commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::squash_commits(
        &command_ctx,
        stack_id,
        source_commit_ids,
        destination_commit_id,
    )?;
    Ok(json!({}))
}

pub fn fetch_from_remotes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let action: Option<String> =
        serde_json::from_value(params.get("action").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
        &command_ctx,
        Some(action.unwrap_or_else(|| "unknown".to_string())),
    )?;

    ctx.project_controller
        .update(&gitbutler_project::UpdateRequest {
            id: project.id,
            project_data_last_fetched: Some(project_data_last_fetched.clone()),
            ..Default::default()
        })
        .context("failed to update project with last fetched timestamp")?;

    if let FetchResult::Error { error, .. } = project_data_last_fetched {
        return Err(anyhow!(error));
    }

    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&command_ctx)?;
    Ok(serde_json::to_value(base_branch)?)
}

pub fn move_commit(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let commit_id: String =
        serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;
    let target_stack_id: StackId =
        serde_json::from_value(params.get("targetStackId").cloned().unwrap_or_default())?;
    let source_stack_id: StackId =
        serde_json::from_value(params.get("sourceStackId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::move_commit(
        &command_ctx,
        target_stack_id,
        commit_id,
        source_stack_id,
    )?;
    Ok(json!({}))
}

pub fn update_commit_message(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId =
        serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let commit_id: String =
        serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;
    let message: String =
        serde_json::from_value(params.get("message").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let new_commit_id = gitbutler_branch_actions::update_commit_message(
        &command_ctx,
        stack_id,
        commit_id,
        &message,
    )?;
    Ok(json!(new_commit_id.to_string()))
}

pub fn find_commit(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let commit_id: String =
        serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let commit = gitbutler_branch_actions::find_commit(&command_ctx, commit_id)?;
    Ok(serde_json::to_value(commit)?)
}

pub fn upstream_integration_statuses(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let target_commit_id: Option<String> =
        serde_json::from_value(params.get("targetCommitId").cloned().unwrap_or_default())?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = target_commit_id
        .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
        .transpose()?;
    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(&command_ctx, commit_id)?;
    Ok(serde_json::to_value(statuses)?)
}

pub fn integrate_upstream(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let resolutions: Vec<Resolution> =
        serde_json::from_value(params.get("resolutions").cloned().unwrap_or_default())?;
    let base_branch_resolution: Option<BaseBranchResolution> = serde_json::from_value(
        params
            .get("baseBranchResolution")
            .cloned()
            .unwrap_or_default(),
    )?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let outcome = gitbutler_branch_actions::integrate_upstream(
        &command_ctx,
        &resolutions,
        base_branch_resolution,
    )?;

    Ok(serde_json::to_value(outcome)?)
}

pub fn resolve_upstream_integration(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId =
        serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let resolution_approach: BaseBranchResolutionApproach = serde_json::from_value(
        params
            .get("resolutionApproach")
            .cloned()
            .unwrap_or_default(),
    )?;

    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let new_target_id =
        gitbutler_branch_actions::resolve_upstream_integration(&command_ctx, resolution_approach)?;
    let commit_id = git2::Oid::to_string(&new_target_id);
    Ok(json!(commit_id))
}
