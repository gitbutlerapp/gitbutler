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
use serde::Deserialize;
use serde_json::{Value, json};

use crate::RequestContext;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NormalizeBranchNameParams {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateVirtualBranchParams {
    project_id: ProjectId,
    branch: BranchCreateRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteLocalBranchParams {
    project_id: ProjectId,
    refname: Refname,
    given_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateVirtualBranchFromBranchParams {
    project_id: ProjectId,
    branch: Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntegrateUpstreamCommitsParams {
    project_id: ProjectId,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetBaseBranchDataParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetBaseBranchParams {
    project_id: ProjectId,
    branch: String,
    push_remote: Option<String>,
    stash_uncommitted: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushBaseBranchParams {
    project_id: ProjectId,
    with_force: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStackOrderParams {
    project_id: ProjectId,
    stacks: Vec<BranchUpdateRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnapplyStackParams {
    project_id: ProjectId,
    stack_id: StackId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CanApplyRemoteBranchParams {
    project_id: ProjectId,
    branch: RemoteRefname,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListCommitFilesParams {
    project_id: ProjectId,
    commit_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmendVirtualBranchParams {
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    worktree_changes: Vec<DiffSpec>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveCommitFileParams {
    project_id: ProjectId,
    stack_id: StackId,
    from_commit_id: String,
    to_commit_id: String,
    ownership: BranchOwnershipClaims,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UndoCommitParams {
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InsertBlankCommitParams {
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: Option<String>,
    offset: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReorderStackParams {
    project_id: ProjectId,
    stack_id: StackId,
    stack_order: StackOrder,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindGitBranchesParams {
    project_id: ProjectId,
    branch_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListBranchesParams {
    project_id: ProjectId,
    filter: Option<BranchListingFilter>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetBranchListingDetailsParams {
    project_id: ProjectId,
    branch_names: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SquashCommitsParams {
    project_id: ProjectId,
    stack_id: StackId,
    source_commit_ids: Vec<String>,
    target_commit_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchFromRemotesParams {
    project_id: ProjectId,
    action: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveCommitParams {
    project_id: ProjectId,
    commit_id: String,
    target_stack_id: StackId,
    source_stack_id: StackId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCommitMessageParams {
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindCommitParams {
    project_id: ProjectId,
    commit_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamIntegrationStatusesParams {
    project_id: ProjectId,
    target_commit_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntegrateUpstreamParams {
    project_id: ProjectId,
    resolutions: Vec<Resolution>,
    base_branch_resolution: Option<BaseBranchResolution>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveUpstreamIntegrationParams {
    project_id: ProjectId,
    resolution_approach: BaseBranchResolutionApproach,
}

pub fn normalize_branch_name(params: Value) -> anyhow::Result<Value> {
    let params: NormalizeBranchNameParams = serde_json::from_value(params)?;
    let normalized = normalize_name(&params.name)?;
    Ok(json!(normalized))
}

pub fn create_virtual_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: CreateVirtualBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        &command_ctx,
        &params.branch,
        command_ctx
            .project()
            .exclusive_worktree_access()
            .write_permission(),
    )?;
    Ok(serde_json::to_value(stack_entry)?)
}

pub fn delete_local_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: DeleteLocalBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::delete_local_branch(
        &command_ctx,
        &params.refname,
        params.given_name,
    )?;
    Ok(json!({}))
}

pub fn create_virtual_branch_from_branch(
    ctx: &RequestContext,
    params: Value,
) -> anyhow::Result<Value> {
    let params: CreateVirtualBranchFromBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        &command_ctx,
        &params.branch,
        params.remote,
        params.pr_number,
    )?;
    Ok(serde_json::to_value(branch_id)?)
}

pub fn integrate_upstream_commits(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: IntegrateUpstreamCommitsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::integrate_upstream_commits(
        &command_ctx,
        params.stack_id,
        params.series_name,
        params.integration_strategy,
    )?;
    Ok(json!({}))
}

pub fn get_base_branch_data(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GetBaseBranchDataParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    if let Ok(base_branch) = gitbutler_branch_actions::base::get_base_branch_data(&command_ctx) {
        Ok(serde_json::to_value(Some(base_branch))?)
    } else {
        Ok(json!(null))
    }
}

pub fn set_base_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: SetBaseBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branch_name = format!("refs/remotes/{}", params.branch)
        .parse()
        .context("Invalid branch name")?;
    let base_branch = gitbutler_branch_actions::set_base_branch(
        &command_ctx,
        &branch_name,
        params.stash_uncommitted.unwrap_or_default(),
        command_ctx
            .project()
            .exclusive_worktree_access()
            .write_permission(),
    )?;

    if let Some(push_remote) = params.push_remote {
        gitbutler_branch_actions::set_target_push_remote(&command_ctx, &push_remote)?;
    }
    Ok(serde_json::to_value(base_branch)?)
}

pub fn push_base_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: PushBaseBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::push_base_branch(&command_ctx, params.with_force)?;
    Ok(json!({}))
}

pub fn update_stack_order(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UpdateStackOrderParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::update_stack_order(&command_ctx, params.stacks)?;
    Ok(json!({}))
}

pub async fn unapply_stack(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UnapplyStackParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
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
            .filter(|a| a.stack_id == Some(params.stack_id))
            .map(|a| a.into())
            .collect::<Vec<DiffSpec>>(),
    );
    gitbutler_branch_actions::unapply_stack(&command_ctx, params.stack_id, assigned_diffspec)?;
    Ok(json!({}))
}

pub fn can_apply_remote_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: CanApplyRemoteBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let can_apply =
        gitbutler_branch_actions::can_apply_remote_branch(&command_ctx, &params.branch)?;
    Ok(json!(can_apply))
}

pub fn list_commit_files(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ListCommitFilesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    let files = gitbutler_branch_actions::list_commit_files(&command_ctx, commit_id)?;
    Ok(serde_json::to_value(files)?)
}

pub fn amend_virtual_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: AmendVirtualBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    let oid = gitbutler_branch_actions::amend(
        &command_ctx,
        params.stack_id,
        commit_id,
        params.worktree_changes,
    )?;
    Ok(json!(oid.to_string()))
}

pub fn move_commit_file(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: MoveCommitFileParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let from_commit_id = git2::Oid::from_str(&params.from_commit_id).map_err(|e| anyhow!(e))?;
    let to_commit_id = git2::Oid::from_str(&params.to_commit_id).map_err(|e| anyhow!(e))?;
    let oid = gitbutler_branch_actions::move_commit_file(
        &command_ctx,
        params.stack_id,
        from_commit_id,
        to_commit_id,
        &params.ownership,
    )?;
    Ok(json!(oid.to_string()))
}

pub fn undo_commit(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UndoCommitParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::undo_commit(&command_ctx, params.stack_id, commit_id)?;
    Ok(json!({}))
}

pub fn insert_blank_commit(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: InsertBlankCommitParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = match params.commit_id {
        Some(oid) => git2::Oid::from_str(&oid).map_err(|e| anyhow!(e))?,
        None => {
            let state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
            let stack = state.get_stack(params.stack_id)?;
            let gix_repo = command_ctx.gix_repo()?;
            stack.head_oid(&gix_repo)?.to_git2()
        }
    };
    gitbutler_branch_actions::insert_blank_commit(
        &command_ctx,
        params.stack_id,
        commit_id,
        params.offset,
        None,
    )?;
    Ok(json!({}))
}

pub fn reorder_stack(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ReorderStackParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    gitbutler_branch_actions::reorder_stack(&command_ctx, params.stack_id, params.stack_order)?;
    Ok(json!({}))
}

pub fn find_git_branches(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: FindGitBranchesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches = gitbutler_branch_actions::find_git_branches(&command_ctx, &params.branch_name)?;
    Ok(serde_json::to_value(branches)?)
}

pub fn list_branches(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ListBranchesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches = gitbutler_branch_actions::list_branches(&command_ctx, params.filter, None)?;
    Ok(serde_json::to_value(branches)?)
}

pub fn get_branch_listing_details(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: GetBranchListingDetailsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branches =
        gitbutler_branch_actions::get_branch_listing_details(&command_ctx, params.branch_names)?;
    Ok(serde_json::to_value(branches)?)
}

pub fn squash_commits(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: SquashCommitsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let source_commit_ids: Vec<git2::Oid> = params
        .source_commit_ids
        .into_iter()
        .map(|oid| git2::Oid::from_str(&oid))
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow!(e))?;
    let destination_commit_id =
        git2::Oid::from_str(&params.target_commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::squash_commits(
        &command_ctx,
        params.stack_id,
        source_commit_ids,
        destination_commit_id,
    )?;
    Ok(json!({}))
}

pub fn fetch_from_remotes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: FetchFromRemotesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
        &command_ctx,
        Some(params.action.unwrap_or_else(|| "unknown".to_string())),
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
    let params: MoveCommitParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::move_commit(
        &command_ctx,
        params.target_stack_id,
        commit_id,
        params.source_stack_id,
    )?;
    Ok(json!({}))
}

pub fn update_commit_message(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UpdateCommitMessageParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    let new_commit_id = gitbutler_branch_actions::update_commit_message(
        &command_ctx,
        params.stack_id,
        commit_id,
        &params.message,
    )?;
    Ok(json!(new_commit_id.to_string()))
}

pub fn find_commit(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: FindCommitParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    let commit = gitbutler_branch_actions::find_commit(&command_ctx, commit_id)?;
    Ok(serde_json::to_value(commit)?)
}

pub fn upstream_integration_statuses(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UpstreamIntegrationStatusesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commit_id = params
        .target_commit_id
        .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
        .transpose()?;
    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(&command_ctx, commit_id)?;
    Ok(serde_json::to_value(statuses)?)
}

pub fn integrate_upstream(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: IntegrateUpstreamParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let outcome = gitbutler_branch_actions::integrate_upstream(
        &command_ctx,
        &params.resolutions,
        params.base_branch_resolution,
    )?;

    Ok(serde_json::to_value(outcome)?)
}

pub fn resolve_upstream_integration(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ResolveUpstreamIntegrationParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let new_target_id = gitbutler_branch_actions::resolve_upstream_integration(
        &command_ctx,
        params.resolution_approach,
    )?;
    let commit_id = git2::Oid::to_string(&new_target_id);
    Ok(json!(commit_id))
}
