use anyhow::{Context, Result, anyhow};
use but_graph::virtual_branches_legacy_types::BranchOwnershipClaims;
use but_workspace::DiffSpec;
use but_workspace::ui::{StackEntryNoOpt, StackHeadInfo};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::branch_upstream_integration::IntegrationStrategy;
use gitbutler_branch_actions::upstream_integration::{
    BaseBranchResolution, BaseBranchResolutionApproach, IntegrationOutcome, Resolution,
    StackStatuses,
};
use gitbutler_branch_actions::{
    BaseBranch, BranchListing, BranchListingDetails, BranchListingFilter, RemoteBranchData,
    RemoteBranchFile, RemoteCommit, StackOrder,
};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::{FetchResult, ProjectId};
use gitbutler_reference::{Refname, RemoteRefname, normalize_branch_name as normalize_name};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gix::reference::Category;
use serde::Deserialize;

use crate::commands::workspace::{CannedBranchNameParams, canned_branch_name};
use crate::{App, error::Error};
// Parameter structs for all functions

pub fn normalize_branch_name(_app: &App, name: String) -> Result<String, Error> {
    Ok(normalize_name(&name)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVirtualBranchParams {
    pub project_id: ProjectId,
    pub branch: BranchCreateRequest,
}

pub fn create_virtual_branch(
    app: &App,
    params: CreateVirtualBranchParams,
) -> Result<StackEntryNoOpt, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ws3_enabled = app.app_settings.get()?.feature_flags.ws3;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let stack_entry = if ws3_enabled {
        let (repo, mut meta, graph) = ctx.graph_and_meta_and_repo()?;
        let ws = graph.to_workspace()?;
        let new_ref = Category::LocalBranch
            .to_full_name(
                params
                    .branch
                    .name
                    .map(Ok)
                    .unwrap_or_else(|| {
                        canned_branch_name(
                            app,
                            CannedBranchNameParams {
                                project_id: params.project_id,
                            },
                        )
                    })?
                    .as_str(),
            )
            .map_err(anyhow::Error::from)?;

        let _guard = project.exclusive_worktree_access();
        let graph =
            but_workspace::branch::create_reference(new_ref.as_ref(), None, &repo, &ws, &mut meta)?;

        let ws = graph.to_workspace()?;
        let (stack_idx, segment_idx) = ws
            .find_segment_owner_indexes_by_refname(new_ref.as_ref())
            .context("BUG: didn't find a stack that was just created")?;
        let stack = &ws.stacks[stack_idx];
        let tip = stack.segments[segment_idx]
            .tip()
            .unwrap_or(repo.object_hash().null());

        StackEntryNoOpt {
            id: stack
                .id
                .context("BUG: all new stacks are created with an ID")?,
            heads: vec![StackHeadInfo {
                name: new_ref.shorten().into(),
                tip,
            }],
            tip,
            order: Some(stack_idx),
        }
    } else {
        gitbutler_branch_actions::create_virtual_branch(
            &ctx,
            &params.branch,
            ctx.project().exclusive_worktree_access().write_permission(),
        )?
    };
    Ok(stack_entry)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteLocalBranchParams {
    pub project_id: ProjectId,
    pub refname: Refname,
    pub given_name: String,
}

pub fn delete_local_branch(app: &App, params: DeleteLocalBranchParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::delete_local_branch(&ctx, &params.refname, params.given_name)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVirtualBranchFromBranchParams {
    pub project_id: ProjectId,
    pub branch: Refname,
    pub remote: Option<RemoteRefname>,
    pub pr_number: Option<usize>,
}

pub fn create_virtual_branch_from_branch(
    app: &App,
    params: CreateVirtualBranchFromBranchParams,
) -> Result<StackId, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        &ctx,
        &params.branch,
        params.remote,
        params.pr_number,
    )?;
    Ok(branch_id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrateUpstreamCommitsParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub series_name: String,
    pub integration_strategy: Option<IntegrationStrategy>,
}

pub fn integrate_upstream_commits(
    app: &App,
    params: IntegrateUpstreamCommitsParams,
) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::integrate_upstream_commits(
        &ctx,
        params.stack_id,
        params.series_name,
        params.integration_strategy,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBaseBranchDataParams {
    pub project_id: ProjectId,
}

pub fn get_base_branch_data(
    app: &App,
    params: GetBaseBranchDataParams,
) -> Result<Option<BaseBranch>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    if let Ok(base_branch) = gitbutler_branch_actions::base::get_base_branch_data(&ctx) {
        Ok(Some(base_branch))
    } else {
        Ok(None)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBaseBranchParams {
    pub project_id: ProjectId,
    pub branch: String,
    pub push_remote: Option<String>,
    pub stash_uncommitted: Option<bool>,
}

pub fn set_base_branch(app: &App, params: SetBaseBranchParams) -> Result<BaseBranch, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let branch_name = format!("refs/remotes/{}", params.branch)
        .parse()
        .context("Invalid branch name")?;
    let base_branch = gitbutler_branch_actions::set_base_branch(
        &ctx,
        &branch_name,
        params.stash_uncommitted.unwrap_or_default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )?;

    // if they also sent a different push remote, set that too
    if let Some(push_remote) = params.push_remote {
        gitbutler_branch_actions::set_target_push_remote(&ctx, &push_remote)?;
    }
    Ok(base_branch)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushBaseBranchParams {
    pub project_id: ProjectId,
    pub with_force: bool,
}

pub fn push_base_branch(app: &App, params: PushBaseBranchParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::push_base_branch(&ctx, params.with_force)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStackOrderParams {
    pub project_id: ProjectId,
    pub stacks: Vec<BranchUpdateRequest>,
}

pub fn update_stack_order(app: &App, params: UpdateStackOrderParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::update_stack_order(&ctx, params.stacks)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnapplyStackParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub fn unapply_stack(app: &App, params: UnapplyStackParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = &mut CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
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
    gitbutler_branch_actions::unapply_stack(ctx, params.stack_id, assigned_diffspec)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanApplyRemoteBranchParams {
    pub project_id: ProjectId,
    pub branch: RemoteRefname,
}

pub fn can_apply_remote_branch(
    app: &App,
    params: CanApplyRemoteBranchParams,
) -> Result<bool, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    Ok(gitbutler_branch_actions::can_apply_remote_branch(
        &ctx,
        &params.branch,
    )?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCommitFilesParams {
    pub project_id: ProjectId,
    pub commit_id: String,
}

pub fn list_commit_files(
    app: &App,
    params: ListCommitFilesParams,
) -> Result<Vec<RemoteBranchFile>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::list_commit_files(&ctx, commit_id).map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmendVirtualBranchParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub commit_id: String,
    pub worktree_changes: Vec<DiffSpec>,
}

pub fn amend_virtual_branch(app: &App, params: AmendVirtualBranchParams) -> Result<String, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    let oid =
        gitbutler_branch_actions::amend(&ctx, params.stack_id, commit_id, params.worktree_changes)?;
    Ok(oid.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCommitFileParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub from_commit_id: String,
    pub to_commit_id: String,
    pub ownership: BranchOwnershipClaims,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoCommitParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub commit_id: String,
}

pub fn undo_commit(app: &App, params: UndoCommitParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::undo_commit(&ctx, params.stack_id, commit_id)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertBlankCommitParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub commit_id: Option<String>,
    pub offset: i32,
}

pub fn insert_blank_commit(app: &App, params: InsertBlankCommitParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = match params.commit_id {
        Some(oid) => git2::Oid::from_str(&oid).map_err(|e| anyhow!(e))?,
        None => {
            let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
            let stack = state.get_stack(params.stack_id)?;
            let gix_repo = ctx.gix_repo()?;
            stack.head_oid(&gix_repo)?.to_git2()
        }
    };
    gitbutler_branch_actions::insert_blank_commit(
        &ctx,
        params.stack_id,
        commit_id,
        params.offset,
        None,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderStackParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub stack_order: StackOrder,
}

pub fn reorder_stack(app: &App, params: ReorderStackParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::reorder_stack(&ctx, params.stack_id, params.stack_order)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindGitBranchesParams {
    pub project_id: ProjectId,
    pub branch_name: String,
}

pub fn find_git_branches(
    app: &App,
    params: FindGitBranchesParams,
) -> Result<Vec<RemoteBranchData>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let branches = gitbutler_branch_actions::find_git_branches(&ctx, &params.branch_name)?;
    Ok(branches)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListBranchesParams {
    pub project_id: ProjectId,
    pub filter: Option<BranchListingFilter>,
}

pub fn list_branches(app: &App, params: ListBranchesParams) -> Result<Vec<BranchListing>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let branches = gitbutler_branch_actions::list_branches(&ctx, params.filter, None)?;
    Ok(branches)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBranchListingDetailsParams {
    pub project_id: ProjectId,
    pub branch_names: Vec<String>,
}

pub fn get_branch_listing_details(
    app: &App,
    params: GetBranchListingDetailsParams,
) -> Result<Vec<BranchListingDetails>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let branches = gitbutler_branch_actions::get_branch_listing_details(&ctx, params.branch_names)?;
    Ok(branches)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquashCommitsParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub source_commit_ids: Vec<String>,
    pub target_commit_id: String,
}

pub fn squash_commits(app: &App, params: SquashCommitsParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let source_commit_ids: Vec<git2::Oid> = params
        .source_commit_ids
        .into_iter()
        .map(|oid| git2::Oid::from_str(&oid))
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow!(e))?;
    let destination_commit_id =
        git2::Oid::from_str(&params.target_commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::squash_commits(
        &ctx,
        params.stack_id,
        source_commit_ids,
        destination_commit_id,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchFromRemotesParams {
    pub project_id: ProjectId,
    pub action: Option<String>,
}

pub fn fetch_from_remotes(app: &App, params: FetchFromRemotesParams) -> Result<BaseBranch, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;

    let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
        &ctx,
        Some(params.action.unwrap_or_else(|| "unknown".to_string())),
    )?;

    // Updates the project controller with the last fetched timestamp
    //
    // TODO: This cross dependency likely indicates that last_fetched is stored in the wrong place - value is coupled with virtual branches state
    gitbutler_project::update(&gitbutler_project::UpdateRequest {
        id: project.id,
        project_data_last_fetched: Some(project_data_last_fetched.clone()),
        ..Default::default()
    })
    .context("failed to update project with last fetched timestamp")?;

    if let FetchResult::Error { error, .. } = project_data_last_fetched {
        return Err(anyhow!(error).into());
    }

    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    Ok(base_branch)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCommitParams {
    pub project_id: ProjectId,
    pub commit_id: String,
    pub target_stack_id: StackId,
    pub source_stack_id: StackId,
}

pub fn move_commit(app: &App, params: MoveCommitParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::move_commit(
        &ctx,
        params.target_stack_id,
        commit_id,
        params.source_stack_id,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCommitMessageParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub commit_id: String,
    pub message: String,
}

pub fn update_commit_message(
    app: &App,
    params: UpdateCommitMessageParams,
) -> Result<String, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    let new_commit_id = gitbutler_branch_actions::update_commit_message(
        &ctx,
        params.stack_id,
        commit_id,
        &params.message,
    )?;
    Ok(new_commit_id.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindCommitParams {
    pub project_id: ProjectId,
    pub commit_id: String,
}

pub fn find_commit(app: &App, params: FindCommitParams) -> Result<Option<RemoteCommit>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = git2::Oid::from_str(&params.commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::find_commit(&ctx, commit_id).map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamIntegrationStatusesParams {
    pub project_id: ProjectId,
    pub target_commit_id: Option<String>,
}

pub fn upstream_integration_statuses(
    app: &App,
    params: UpstreamIntegrationStatusesParams,
) -> Result<StackStatuses, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let commit_id = params
        .target_commit_id
        .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
        .transpose()?;
    Ok(gitbutler_branch_actions::upstream_integration_statuses(
        &ctx, commit_id,
    )?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrateUpstreamParams {
    pub project_id: ProjectId,
    pub resolutions: Vec<Resolution>,
    pub base_branch_resolution: Option<BaseBranchResolution>,
}

pub fn integrate_upstream(
    app: &App,
    params: IntegrateUpstreamParams,
) -> Result<IntegrationOutcome, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let outcome = gitbutler_branch_actions::integrate_upstream(
        &ctx,
        &params.resolutions,
        params.base_branch_resolution,
    )?;

    Ok(outcome)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveUpstreamIntegrationParams {
    pub project_id: ProjectId,
    pub resolution_approach: BaseBranchResolutionApproach,
}

pub fn resolve_upstream_integration(
    app: &App,
    params: ResolveUpstreamIntegrationParams,
) -> Result<String, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;

    let new_target_id =
        gitbutler_branch_actions::resolve_upstream_integration(&ctx, params.resolution_approach)?;
    let commit_id = git2::Oid::to_string(&new_target_id);
    Ok(commit_id)
}
