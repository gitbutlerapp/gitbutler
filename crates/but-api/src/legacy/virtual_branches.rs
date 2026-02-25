use std::collections::HashMap;

use anyhow::{Context as _, Result, anyhow};
use but_api_macros::but_api;
use but_core::{DiffSpec, sync::RepoExclusive};
use but_ctx::{Context, ThreadSafeContext};
use but_oxidize::ObjectIdExt;
use but_workspace::legacy::ui::{StackEntryNoOpt, StackHeadInfo};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::{
    BaseBranch, BranchListing, BranchListingDetails, BranchListingFilter, MoveBranchResult,
    MoveCommitIllegalAction, StackOrder,
    branch_upstream_integration::IntegrationStrategy,
    upstream_integration::{
        BaseBranchResolution, BaseBranchResolutionApproach, IntegrationOutcome, Resolution,
        StackStatuses,
    },
};
use gitbutler_project::FetchResult;
use gitbutler_reference::{Refname, RemoteRefname, normalize_branch_name as normalize_name};
use gitbutler_stack::StackId;
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
    ctx: &mut Context,
    branch: BranchCreateRequest,
) -> Result<StackEntryNoOpt> {
    let stack_entry = {
        let branch_name = match branch.name {
            Some(name) => normalize_name(&name)?,
            None => canned_branch_name(ctx)?,
        };
        let new_ref = Category::LocalBranch
            .to_full_name(branch_name.as_str())
            .map_err(anyhow::Error::from)?;

        let mut meta = ctx.meta()?;
        let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
        let new_ws = but_workspace::branch::create_reference(
            new_ref.as_ref(),
            None,
            &repo,
            &ws,
            &mut meta,
            |_| StackId::generate(),
            branch.order,
        )?;

        let (stack_idx, segment_idx) = new_ws
            .find_segment_owner_indexes_by_refname(new_ref.as_ref())
            .context("BUG: didn't find a stack that was just created")?;
        let stack = &new_ws.stacks[stack_idx];
        let tip = stack.segments[segment_idx]
            .tip()
            .unwrap_or(repo.object_hash().null());
        let review_id = stack.segments[segment_idx]
            .metadata
            .as_ref()
            .and_then(|meta| meta.review.pull_request);

        let out = StackEntryNoOpt {
            id: stack
                .id
                .context("BUG: all new stacks are created with an ID")?,
            heads: vec![StackHeadInfo {
                name: new_ref.shorten().into(),
                review_id,
                tip,
                is_checked_out: false,
            }],
            tip,
            order: Some(stack_idx),
            is_checked_out: false,
        };

        *ws = new_ws.into_owned();
        out
    };
    Ok(stack_entry)
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_local_branch(
    ctx: &mut but_ctx::Context,
    refname: Refname,
    given_name: String,
) -> Result<()> {
    gitbutler_branch_actions::delete_local_branch(ctx, &refname, given_name)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn create_virtual_branch_from_branch(
    ctx: &mut but_ctx::Context,
    branch: Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
) -> Result<gitbutler_branch_actions::CreateBranchFromBranchOutcome> {
    let outcome = gitbutler_branch_actions::create_virtual_branch_from_branch(
        ctx, &branch, remote, pr_number,
    )?;
    Ok(outcome.into())
}

#[but_api]
#[instrument(err(Debug))]
pub fn integrate_upstream_commits(
    ctx: &mut but_ctx::Context,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<()> {
    gitbutler_branch_actions::integrate_upstream_commits(
        ctx,
        stack_id,
        series_name,
        integration_strategy,
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_initial_integration_steps_for_branch(
    ctx: &mut but_ctx::Context,
    stack_id: Option<StackId>,
    branch_name: String,
) -> Result<
    Vec<gitbutler_branch_actions::branch_upstream_integration::InteractiveIntegrationStep>,
    Error,
> {
    let steps = gitbutler_branch_actions::branch_upstream_integration::get_initial_integration_steps_for_branch(
        ctx,
        stack_id,
        branch_name,
    )?;
    Ok(steps)
}

#[but_api]
#[instrument(err(Debug))]
pub fn integrate_branch_with_steps(
    ctx: &mut but_ctx::Context,
    stack_id: StackId,
    branch_name: String,
    steps: Vec<gitbutler_branch_actions::branch_upstream_integration::InteractiveIntegrationStep>,
) -> Result<()> {
    gitbutler_branch_actions::integrate_branch_with_steps(ctx, stack_id, branch_name, steps)
}

#[but_api]
#[instrument(err(Debug))]
pub fn switch_back_to_workspace(ctx: &mut but_ctx::Context) -> Result<BaseBranch> {
    let mut guard = ctx.exclusive_worktree_access();
    switch_back_to_workspace_with_perm(ctx, guard.write_permission())
}

#[instrument(skip(perm), err(Debug))]
pub fn switch_back_to_workspace_with_perm(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
) -> Result<BaseBranch> {
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)
        .context("Failed to get base branch data")?;

    let branch_name = format!("refs/remotes/{}", base_branch.branch_name)
        .parse()
        .context("Invalid branch name")?;

    gitbutler_branch_actions::set_base_branch(ctx, &branch_name, perm)?;
    crate::legacy::meta::reconcile_in_workspace_state_of_vb_toml(ctx, perm).ok();

    Ok(base_branch)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_base_branch_data(ctx: &but_ctx::Context) -> Result<Option<BaseBranch>> {
    if let Ok(base_branch) = gitbutler_branch_actions::base::get_base_branch_data(ctx) {
        Ok(Some(base_branch))
    } else {
        Ok(None)
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn set_base_branch(
    ctx: &mut but_ctx::Context,
    branch: String,
    push_remote: Option<String>,
) -> Result<BaseBranch> {
    let mut guard = ctx.exclusive_worktree_access();
    set_base_branch_with_perm(ctx, branch, push_remote, guard.write_permission())
}

#[instrument(skip(perm), err(Debug))]
pub fn set_base_branch_with_perm(
    ctx: &mut but_ctx::Context,
    branch: String,
    push_remote: Option<String>,
    perm: &mut RepoExclusive,
) -> Result<BaseBranch> {
    let branch_name = format!("refs/remotes/{branch}")
        .parse()
        .context("Invalid branch name")?;
    let base_branch = gitbutler_branch_actions::set_base_branch(ctx, &branch_name, perm)?;

    // if they also sent a different push remote, set that too
    if let Some(push_remote) = push_remote {
        gitbutler_branch_actions::set_target_push_remote(ctx, &push_remote)?;
    }
    {
        crate::legacy::meta::reconcile_in_workspace_state_of_vb_toml(ctx, perm).ok();
    }

    Ok(base_branch)
}

#[but_api]
#[instrument(err(Debug))]
pub fn push_base_branch(ctx: &mut but_ctx::Context, with_force: bool) -> Result<()> {
    gitbutler_branch_actions::push_base_branch(ctx, with_force)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_stack_order(
    ctx: &mut but_ctx::Context,
    stacks: Vec<BranchUpdateRequest>,
) -> Result<()> {
    gitbutler_branch_actions::update_stack_order(ctx, stacks)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn unapply_stack(ctx: &mut Context, stack_id: StackId) -> Result<()> {
    let context_lines = ctx.settings.context_lines;
    let (mut guard, repo, ws, mut db) = ctx.workspace_mut_and_db_mut()?;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        false,
        Some(but_core::diff::ui::worktree_changes(&repo)?.changes),
        None,
        context_lines,
    )?;
    let assigned_diffspec = but_workspace::flatten_diff_specs(
        assignments
            .into_iter()
            .filter(|a| a.stack_id == Some(stack_id))
            .map(|a| a.into())
            .collect::<Vec<DiffSpec>>(),
    );
    drop((repo, ws, db));
    gitbutler_branch_actions::unapply_stack(
        ctx,
        guard.write_permission(),
        stack_id,
        assigned_diffspec,
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn amend_virtual_branch(
    ctx: &mut but_ctx::Context,
    stack_id: StackId,
    commit_id: gix::ObjectId,
    worktree_changes: Vec<DiffSpec>,
) -> Result<String> {
    let oid =
        gitbutler_branch_actions::amend(ctx, stack_id, commit_id.to_git2(), worktree_changes)?;
    Ok(oid.to_string())
}

#[but_api]
#[instrument(err(Debug))]
pub fn undo_commit(
    ctx: &mut but_ctx::Context,
    stack_id: StackId,
    commit_id: gix::ObjectId,
) -> Result<()> {
    gitbutler_branch_actions::undo_commit(ctx, stack_id, commit_id.to_git2())?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn reorder_stack(
    ctx: &mut but_ctx::Context,
    stack_id: StackId,
    stack_order: StackOrder,
) -> Result<()> {
    gitbutler_branch_actions::reorder_stack(ctx, stack_id, stack_order)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_branches(
    ctx: &Context,
    filter: Option<BranchListingFilter>,
) -> Result<Vec<BranchListing>> {
    let branches = gitbutler_branch_actions::list_branches(ctx, filter, None)?;
    Ok(branches)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_branch_listing_details(
    ctx: &but_ctx::Context,
    branch_names: Vec<String>,
) -> Result<Vec<BranchListingDetails>> {
    let branches = gitbutler_branch_actions::get_branch_listing_details(ctx, branch_names)?;
    Ok(branches)
}

#[but_api]
#[instrument(err(Debug))]
pub fn squash_commits(
    ctx: &mut Context,
    stack_id: StackId,
    source_commit_ids: Vec<String>,
    target_commit_id: gix::ObjectId,
) -> Result<()> {
    let source_commit_ids: Vec<git2::Oid> = source_commit_ids
        .into_iter()
        .map(|oid| git2::Oid::from_str(&oid))
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::squash_commits(
        ctx,
        stack_id,
        source_commit_ids,
        target_commit_id.to_git2(),
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn fetch_from_remotes(ctx: &Context, action: Option<String>) -> Result<BaseBranch> {
    let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
        ctx,
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

    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
    Ok(base_branch)
}

#[but_api]
#[instrument(err(Debug))]
pub fn move_commit(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    target_stack_id: StackId,
    source_stack_id: StackId,
) -> Result<Option<MoveCommitIllegalAction>> {
    gitbutler_branch_actions::move_commit(
        ctx,
        target_stack_id,
        commit_id.to_git2(),
        source_stack_id,
    )
}

#[but_api]
#[instrument(err(Debug))]
pub fn move_branch(
    ctx: &mut but_ctx::Context,
    target_stack_id: StackId,
    target_branch_name: String,
    source_stack_id: StackId,
    subject_branch_name: String,
) -> Result<MoveBranchResult> {
    gitbutler_branch_actions::move_branch(
        ctx,
        target_stack_id,
        target_branch_name.as_str(),
        source_stack_id,
        subject_branch_name.as_str(),
    )
}

#[but_api]
#[instrument(err(Debug))]
pub fn tear_off_branch(
    ctx: &mut but_ctx::Context,
    source_stack_id: StackId,
    subject_branch_name: String,
) -> Result<MoveBranchResult> {
    gitbutler_branch_actions::tear_off_branch(ctx, source_stack_id, subject_branch_name.as_str())
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_commit_message(
    ctx: &mut but_ctx::Context,
    stack_id: StackId,
    commit_id: gix::ObjectId,
    message: String,
) -> Result<String> {
    let new_commit_id = gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_id,
        commit_id.to_git2(),
        &message,
    )?;
    Ok(new_commit_id.to_string())
}

#[but_api]
#[instrument(err(Debug))]
pub async fn upstream_integration_statuses(
    ctx: ThreadSafeContext,
    target_commit_id: Option<String>,
) -> Result<StackStatuses> {
    let (base_branch, commit_id, ctx) = {
        let commit_id = target_commit_id
            .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
            .transpose()?;
        let ctx = ctx.into_thread_local();

        // Get all the actively applied reviews
        (
            gitbutler_branch_actions::base::get_base_branch_data(&ctx)?,
            commit_id,
            ctx.into_sync(),
        )
    };

    let resolved_reviews = resolve_review_map(ctx.clone(), &base_branch).await?;
    let mut ctx = ctx.into_thread_local();
    gitbutler_branch_actions::upstream_integration_statuses(&mut ctx, commit_id, &resolved_reviews)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn integrate_upstream(
    ctx: ThreadSafeContext,
    resolutions: Vec<Resolution>,
    base_branch_resolution: Option<BaseBranchResolution>,
) -> Result<IntegrationOutcome> {
    let (base_branch, ctx) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        (base_branch, ctx.to_sync())
    };
    let resolved_reviews = resolve_review_map(ctx.clone(), &base_branch).await?;
    let mut ctx = ctx.into_thread_local();
    let outcome = gitbutler_branch_actions::integrate_upstream(
        &mut ctx,
        &resolutions,
        base_branch_resolution,
        &resolved_reviews,
    )?;

    Ok(outcome)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn resolve_upstream_integration(
    ctx: ThreadSafeContext,
    resolution_approach: BaseBranchResolutionApproach,
) -> Result<String> {
    let (base_branch, sync_ctx) = {
        let ctx = ctx.into_thread_local();
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        (base_branch, ctx.into_sync())
    };
    let resolved_reviews = resolve_review_map(sync_ctx.clone(), &base_branch).await?;
    let mut ctx = sync_ctx.into_thread_local();
    let new_target_id = gitbutler_branch_actions::resolve_upstream_integration(
        &mut ctx,
        resolution_approach,
        &resolved_reviews,
    )?;
    let commit_id = git2::Oid::to_string(&new_target_id);
    Ok(commit_id)
}

/// Resolve all actively applied reviews for the given project and command context
async fn resolve_review_map(
    ctx: ThreadSafeContext,
    base_branch: &BaseBranch,
) -> Result<HashMap<String, but_forge::ForgeReview>> {
    let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);
    let Some(forge_repo_info) = forge_repo_info.as_ref() else {
        // No forge? No problem!
        // If there's no forge associated with the base branch, there can't be any reviews.
        // Return an empty map.
        return Ok(HashMap::new());
    };

    let filter = Some(BranchListingFilter {
        local: None,
        applied: Some(true),
    });
    let ctx = ctx.into_thread_local();
    let (branches, preferred_forge_user) = {
        let preferred_forge_user = ctx.legacy_project.preferred_forge_user.clone();
        (list_branches(&ctx, filter)?, preferred_forge_user)
    };
    let mut reviews = branches.iter().fold(HashMap::new(), |mut acc, branch| {
        if let Some(stack_ref) = &branch.stack {
            acc.extend(stack_ref.pull_requests.iter().map(|(k, v)| (k.clone(), *v)));
        }
        acc
    });
    let mut ctx = ctx;
    let mut resolved_reviews = HashMap::new();
    let db = &mut *ctx.db.get_mut()?;
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    for (key, pr_number) in reviews.drain() {
        if let Ok(resolved) = but_forge::get_forge_review(
            &preferred_forge_user,
            forge_repo_info,
            pr_number,
            db,
            &storage,
        ) {
            resolved_reviews.insert(key, resolved);
        }
    }
    Ok(resolved_reviews)
}
