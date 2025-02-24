use std::collections::HashMap;

use anyhow::{Context as _, Result, anyhow};
use but_api_macros::but_api;
use but_core::DiffSpec;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use but_workspace::legacy::ui::{StackEntryNoOpt, StackHeadInfo};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::{
    BaseBranch, BranchListing, BranchListingDetails, BranchListingFilter, MoveBranchResult,
    MoveCommitIllegalAction, RemoteBranchData, RemoteBranchFile, RemoteCommit, StackOrder,
    branch_upstream_integration::IntegrationStrategy,
    upstream_integration::{
        BaseBranchResolution, BaseBranchResolutionApproach, IntegrationOutcome, Resolution,
        StackStatuses,
    },
};
use gitbutler_project::{FetchResult, ProjectId};
use gitbutler_reference::{Refname, RemoteRefname, normalize_branch_name as normalize_name};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gix::reference::Category;
use tracing::instrument;

use crate::{json::Error, legacy::workspace::canned_branch_name};
// Parameter structs for all functions

#[but_api]
#[instrument(err(Debug))]
pub fn normalize_branch_name(name: String) -> Result<String> {
    normalize_name(&name)
}

#[but_api]
#[instrument(err(Debug))]
pub fn create_virtual_branch(
    project_id: ProjectId,
    branch: BranchCreateRequest,
) -> Result<StackEntryNoOpt> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let ws3_enabled = ctx.settings().feature_flags.ws3;
    let stack_entry = if ws3_enabled {
        let mut guard = ctx.exclusive_worktree_access();
        let (repo, mut meta, graph) =
            ctx.graph_and_meta_mut_and_repo_from_head(guard.write_permission())?;
        let ws = graph.to_workspace()?;
        let new_ref = Category::LocalBranch
            .to_full_name(
                branch
                    .name
                    .map(Ok)
                    .unwrap_or_else(|| canned_branch_name(project_id))?
                    .as_str(),
            )
            .map_err(anyhow::Error::from)?;

        let graph = but_workspace::branch::create_reference(
            new_ref.as_ref(),
            None,
            &repo,
            &ws,
            &mut meta,
            |_| StackId::generate(),
            branch.order,
        )?;

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
                is_checked_out: false,
            }],
            tip,
            order: Some(stack_idx),
            is_checked_out: false,
        }
    } else {
        gitbutler_branch_actions::create_virtual_branch(
            &ctx,
            &branch,
            ctx.exclusive_worktree_access().write_permission(),
        )?
    };
    Ok(stack_entry)
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_local_branch(
    project_id: ProjectId,
    refname: Refname,
    given_name: String,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::delete_local_branch(&ctx, &refname, given_name)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn create_virtual_branch_from_branch(
    project_id: ProjectId,
    branch: Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
) -> Result<gitbutler_branch_actions::CreateBranchFromBranchOutcome> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let outcome = gitbutler_branch_actions::create_virtual_branch_from_branch(
        &ctx, &branch, remote, pr_number,
    )?;
    Ok(outcome.into())
}

#[but_api]
#[instrument(err(Debug))]
pub fn integrate_upstream_commits(
    project_id: ProjectId,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::integrate_upstream_commits(
        &ctx,
        stack_id,
        series_name,
        integration_strategy,
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_initial_integration_steps_for_branch(
    project_id: ProjectId,
    stack_id: Option<StackId>,
    branch_name: String,
) -> Result<
    Vec<gitbutler_branch_actions::branch_upstream_integration::InteractiveIntegrationStep>,
    Error,
> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let steps = gitbutler_branch_actions::branch_upstream_integration::get_initial_integration_steps_for_branch(
        &ctx,
        stack_id,
        branch_name,
    )?;
    Ok(steps)
}

#[but_api]
#[instrument(err(Debug))]
pub fn integrate_branch_with_steps(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    steps: Vec<gitbutler_branch_actions::branch_upstream_integration::InteractiveIntegrationStep>,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::integrate_branch_with_steps(&ctx, stack_id, branch_name, steps)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_base_branch_data(project_id: ProjectId) -> Result<Option<BaseBranch>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    if let Ok(base_branch) = gitbutler_branch_actions::base::get_base_branch_data(&ctx) {
        Ok(Some(base_branch))
    } else {
        Ok(None)
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn set_base_branch(
    project_id: ProjectId,
    branch: String,
    push_remote: Option<String>,
) -> Result<BaseBranch> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let branch_name = format!("refs/remotes/{branch}")
        .parse()
        .context("Invalid branch name")?;
    let base_branch = gitbutler_branch_actions::set_base_branch(
        &ctx,
        &branch_name,
        ctx.exclusive_worktree_access().write_permission(),
    )?;

    // if they also sent a different push remote, set that too
    if let Some(push_remote) = push_remote {
        gitbutler_branch_actions::set_target_push_remote(&ctx, &push_remote)?;
    }
    {
        let mut guard = ctx.exclusive_worktree_access();
        crate::legacy::meta::reconcile_in_workspace_state_of_vb_toml(
            &mut ctx,
            guard.write_permission(),
        )
        .ok();
    }

    Ok(base_branch)
}

#[but_api]
#[instrument(err(Debug))]
pub fn push_base_branch(project_id: ProjectId, with_force: bool) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::push_base_branch(&ctx, with_force)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_stack_order(project_id: ProjectId, stacks: Vec<BranchUpdateRequest>) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::update_stack_order(&ctx, stacks)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn unapply_stack(project_id: ProjectId, stack_id: StackId) -> Result<()> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut Context::new_from_legacy_project(project.clone())?;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        false,
        Some(
            but_core::diff::ui::worktree_changes_by_worktree_dir(project.worktree_dir()?.into())?
                .changes,
        ),
        None,
    )?;
    let assigned_diffspec = but_workspace::flatten_diff_specs(
        assignments
            .into_iter()
            .filter(|a| a.stack_id == Some(stack_id))
            .map(|a| a.into())
            .collect::<Vec<DiffSpec>>(),
    );
    gitbutler_branch_actions::unapply_stack(ctx, stack_id, assigned_diffspec)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn can_apply_remote_branch(project_id: ProjectId, branch: RemoteRefname) -> Result<bool> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::can_apply_remote_branch(&ctx, &branch)
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_commit_files(
    project_id: ProjectId,
    commit_id: String,
) -> Result<Vec<RemoteBranchFile>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::list_commit_files(&ctx, commit_id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn amend_virtual_branch(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    worktree_changes: Vec<DiffSpec>,
) -> Result<String> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let oid = gitbutler_branch_actions::amend(&ctx, stack_id, commit_id, worktree_changes)?;
    Ok(oid.to_string())
}

#[but_api]
#[instrument(err(Debug))]
pub fn undo_commit(project_id: ProjectId, stack_id: StackId, commit_id: String) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::undo_commit(&ctx, stack_id, commit_id)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn insert_blank_commit(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: Option<String>,
    offset: i32,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit_id = match commit_id {
        Some(oid) => git2::Oid::from_str(&oid).map_err(|e| anyhow!(e))?,
        None => {
            let state = VirtualBranchesHandle::new(ctx.project_data_dir());
            let stack = state.get_stack(stack_id)?;
            let gix_repo = ctx.repo.get()?;
            stack.head_oid(&gix_repo)?.to_git2()
        }
    };
    gitbutler_branch_actions::insert_blank_commit(&ctx, stack_id, commit_id, offset, None)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn reorder_stack(
    project_id: ProjectId,
    stack_id: StackId,
    stack_order: StackOrder,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::reorder_stack(&ctx, stack_id, stack_order)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn find_git_branches(
    project_id: ProjectId,
    branch_name: String,
) -> Result<Vec<RemoteBranchData>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let branches = gitbutler_branch_actions::find_git_branches(&ctx, &branch_name)?;
    Ok(branches)
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_branches(
    project_id: ProjectId,
    filter: Option<BranchListingFilter>,
) -> Result<Vec<BranchListing>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let branches = gitbutler_branch_actions::list_branches(&ctx, filter, None)?;
    Ok(branches)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_branch_listing_details(
    project_id: ProjectId,
    branch_names: Vec<String>,
) -> Result<Vec<BranchListingDetails>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let branches = gitbutler_branch_actions::get_branch_listing_details(&ctx, branch_names)?;
    Ok(branches)
}

#[but_api]
#[instrument(err(Debug))]
pub fn squash_commits(
    project_id: ProjectId,
    stack_id: StackId,
    source_commit_ids: Vec<String>,
    target_commit_id: String,
) -> Result<()> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let source_commit_ids: Vec<git2::Oid> = source_commit_ids
        .into_iter()
        .map(|oid| git2::Oid::from_str(&oid))
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow!(e))?;
    let destination_commit_id = git2::Oid::from_str(&target_commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::squash_commits(
        &ctx,
        stack_id,
        source_commit_ids,
        destination_commit_id,
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn fetch_from_remotes(project_id: ProjectId, action: Option<String>) -> Result<BaseBranch> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;

    let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
        &ctx,
        Some(action.unwrap_or_else(|| "unknown".to_string())),
    )?;

    // Updates the project controller with the last fetched timestamp
    //
    // TODO: This cross dependency likely indicates that last_fetched is stored in the wrong place - value is coupled with virtual branches state
    gitbutler_project::update(gitbutler_project::UpdateRequest {
        project_data_last_fetched: Some(project_data_last_fetched.clone()),
        ..gitbutler_project::UpdateRequest::default_with_id(ctx.legacy_project.id)
    })
    .context("failed to update project with last fetched timestamp")?;

    if let FetchResult::Error { error, .. } = project_data_last_fetched {
        return Err(anyhow!(error));
    }

    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    Ok(base_branch)
}

#[but_api]
#[instrument(err(Debug))]
pub fn move_commit(
    project_id: ProjectId,
    commit_id: String,
    target_stack_id: StackId,
    source_stack_id: StackId,
) -> Result<Option<MoveCommitIllegalAction>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::move_commit(&ctx, target_stack_id, commit_id, source_stack_id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn move_branch(
    project_id: ProjectId,
    target_stack_id: StackId,
    target_branch_name: String,
    source_stack_id: StackId,
    subject_branch_name: String,
) -> Result<MoveBranchResult> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::move_branch(
        &ctx,
        target_stack_id,
        target_branch_name.as_str(),
        source_stack_id,
        subject_branch_name.as_str(),
    )
}

#[but_api]
#[instrument(err(Debug))]
pub fn tear_off_branch(
    project_id: ProjectId,
    source_stack_id: StackId,
    subject_branch_name: String,
) -> Result<MoveBranchResult> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::tear_off_branch(&ctx, source_stack_id, subject_branch_name.as_str())
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_commit_message(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    message: String,
) -> Result<String> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let new_commit_id =
        gitbutler_branch_actions::update_commit_message(&ctx, stack_id, commit_id, &message)?;
    Ok(new_commit_id.to_string())
}

#[but_api]
#[instrument(err(Debug))]
pub fn find_commit(project_id: ProjectId, commit_id: String) -> Result<Option<RemoteCommit>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::find_commit(&ctx, commit_id)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn upstream_integration_statuses(
    project_id: ProjectId,
    target_commit_id: Option<String>,
) -> Result<StackStatuses> {
    let project = gitbutler_project::get(project_id)?;
    let (base_branch, commit_id, sync_ctx) = {
        let ctx = Context::new_from_legacy_project(project.clone())?;
        let commit_id = target_commit_id
            .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
            .transpose()?;

        // Get all the actively applied reviews
        (
            gitbutler_branch_actions::base::get_base_branch_data(&ctx)?,
            commit_id,
            ctx.into_sync(),
        )
    };

    let resolved_reviews = resolve_review_map(project, &base_branch).await?;
    let ctx = sync_ctx.into_thread_local();
    gitbutler_branch_actions::upstream_integration_statuses(&ctx, commit_id, &resolved_reviews)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn integrate_upstream(
    project_id: ProjectId,
    resolutions: Vec<Resolution>,
    base_branch_resolution: Option<BaseBranchResolution>,
) -> Result<IntegrationOutcome> {
    let project = gitbutler_project::get(project_id)?;
    let (base_branch, sync_ctx) = {
        let ctx = Context::new_from_legacy_project(project.clone())?;
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        (base_branch, ctx.into_sync())
    };
    let resolved_reviews = resolve_review_map(project, &base_branch).await?;
    let ctx = sync_ctx.into_thread_local();
    let outcome = gitbutler_branch_actions::integrate_upstream(
        &ctx,
        &resolutions,
        base_branch_resolution,
        &resolved_reviews,
    )?;

    Ok(outcome)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn resolve_upstream_integration(
    project_id: ProjectId,
    resolution_approach: BaseBranchResolutionApproach,
) -> Result<String> {
    let project = gitbutler_project::get(project_id)?;
    let (base_branch, sync_ctx) = {
        let ctx = Context::new_from_legacy_project(project.clone())?;

        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        (base_branch, ctx.into_sync())
    };
    let resolved_reviews = resolve_review_map(project, &base_branch).await?;
    let ctx = sync_ctx.into_thread_local();
    let new_target_id = gitbutler_branch_actions::resolve_upstream_integration(
        &ctx,
        resolution_approach,
        &resolved_reviews,
    )?;
    let commit_id = git2::Oid::to_string(&new_target_id);
    Ok(commit_id)
}

/// Resolve all actively applied reviews for the given project and command context
async fn resolve_review_map(
    project: gitbutler_project::Project,
    base_branch: &BaseBranch,
) -> Result<HashMap<String, but_forge::ForgeReview>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    let Some(forge_repo_info) = base_branch.forge_repo_info.as_ref() else {
        // No forge? No problem!
        // If there's no forge associated with the base branch, there can't be any reviews.
        // Return an empty map.
        return Ok(HashMap::new());
    };

    let filter = Some(BranchListingFilter {
        local: None,
        applied: Some(true),
    });
    let branches = list_branches(project.id, filter)?;
    let mut reviews = branches.iter().fold(HashMap::new(), |mut acc, branch| {
        if let Some(stack_ref) = &branch.stack {
            acc.extend(stack_ref.pull_requests.iter().map(|(k, v)| (k.clone(), *v)));
        }
        acc
    });
    let mut resolved_reviews = HashMap::new();
    for (key, pr_number) in reviews.drain() {
        if let Ok(resolved) = but_forge::get_forge_review(
            &project.preferred_forge_user,
            forge_repo_info,
            pr_number,
            &storage,
        )
        .await
        {
            resolved_reviews.insert(key, resolved);
        }
    }
    Ok(resolved_reviews)
}
