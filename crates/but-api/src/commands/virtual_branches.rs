use anyhow::{Context, Result, anyhow};
use but_api_macros::api_cmd;
use but_core::DiffSpec;
use but_oxidize::ObjectIdExt;
use but_settings::AppSettings;
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
use gitbutler_command_context::CommandContext;
use gitbutler_project::{FetchResult, ProjectId};
use gitbutler_reference::{Refname, RemoteRefname, normalize_branch_name as normalize_name};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gix::reference::Category;
use tracing::instrument;

use crate::{commands::workspace::canned_branch_name, json::Error};
// Parameter structs for all functions

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn normalize_branch_name(name: String) -> Result<String, Error> {
    Ok(normalize_name(&name)?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn create_virtual_branch(
    project_id: ProjectId,
    branch: BranchCreateRequest,
) -> Result<StackEntryNoOpt, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let ws3_enabled = ctx.app_settings().feature_flags.ws3;
    let stack_entry = if ws3_enabled {
        let mut guard = project.exclusive_worktree_access();
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
            &mut *meta,
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
            ctx.project().exclusive_worktree_access().write_permission(),
        )?
    };
    Ok(stack_entry)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn delete_local_branch(
    project_id: ProjectId,
    refname: Refname,
    given_name: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::delete_local_branch(&ctx, &refname, given_name)?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn create_virtual_branch_from_branch(
    project_id: ProjectId,
    branch: Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
) -> Result<gitbutler_branch_actions::CreateBranchFromBranchOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let outcome = gitbutler_branch_actions::create_virtual_branch_from_branch(
        &ctx, &branch, remote, pr_number,
    )?;
    Ok(outcome.into())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn integrate_upstream_commits(
    project_id: ProjectId,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::integrate_upstream_commits(
        &ctx,
        stack_id,
        series_name,
        integration_strategy,
    )?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_initial_integration_steps_for_branch(
    project_id: ProjectId,
    stack_id: Option<StackId>,
    branch_name: String,
) -> Result<
    Vec<gitbutler_branch_actions::branch_upstream_integration::InteractiveIntegrationStep>,
    Error,
> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let steps = gitbutler_branch_actions::branch_upstream_integration::get_initial_integration_steps_for_branch(
        &ctx,
        stack_id,
        branch_name,
    )?;
    Ok(steps)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn integrate_branch_with_steps(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    steps: Vec<gitbutler_branch_actions::branch_upstream_integration::InteractiveIntegrationStep>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::integrate_branch_with_steps(&ctx, stack_id, branch_name, steps)
        .map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_base_branch_data(project_id: ProjectId) -> Result<Option<BaseBranch>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    if let Ok(base_branch) = gitbutler_branch_actions::base::get_base_branch_data(&ctx) {
        Ok(Some(base_branch))
    } else {
        Ok(None)
    }
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn set_base_branch(
    project_id: ProjectId,
    branch: String,
    push_remote: Option<String>,
) -> Result<BaseBranch, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let branch_name = format!("refs/remotes/{branch}")
        .parse()
        .context("Invalid branch name")?;
    let base_branch = gitbutler_branch_actions::set_base_branch(
        &ctx,
        &branch_name,
        ctx.project().exclusive_worktree_access().write_permission(),
    )?;

    // if they also sent a different push remote, set that too
    if let Some(push_remote) = push_remote {
        gitbutler_branch_actions::set_target_push_remote(&ctx, &push_remote)?;
    }
    Ok(base_branch)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn push_base_branch(project_id: ProjectId, with_force: bool) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::push_base_branch(&ctx, with_force)?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn update_stack_order(
    project_id: ProjectId,
    stacks: Vec<BranchUpdateRequest>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::update_stack_order(&ctx, stacks)?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn unapply_stack(project_id: ProjectId, stack_id: StackId) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
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

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn can_apply_remote_branch(
    project_id: ProjectId,
    branch: RemoteRefname,
) -> Result<bool, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(gitbutler_branch_actions::can_apply_remote_branch(
        &ctx, &branch,
    )?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn list_commit_files(
    project_id: ProjectId,
    commit_id: String,
) -> Result<Vec<RemoteBranchFile>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::list_commit_files(&ctx, commit_id).map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn amend_virtual_branch(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    worktree_changes: Vec<DiffSpec>,
) -> Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let oid = gitbutler_branch_actions::amend(&ctx, stack_id, commit_id, worktree_changes)?;
    Ok(oid.to_string())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn undo_commit(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::undo_commit(&ctx, stack_id, commit_id)?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn insert_blank_commit(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: Option<String>,
    offset: i32,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = match commit_id {
        Some(oid) => git2::Oid::from_str(&oid).map_err(|e| anyhow!(e))?,
        None => {
            let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
            let stack = state.get_stack(stack_id)?;
            let gix_repo = ctx.gix_repo()?;
            stack.head_oid(&gix_repo)?.to_git2()
        }
    };
    gitbutler_branch_actions::insert_blank_commit(&ctx, stack_id, commit_id, offset, None)?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn reorder_stack(
    project_id: ProjectId,
    stack_id: StackId,
    stack_order: StackOrder,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::reorder_stack(&ctx, stack_id, stack_order)?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn find_git_branches(
    project_id: ProjectId,
    branch_name: String,
) -> Result<Vec<RemoteBranchData>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let branches = gitbutler_branch_actions::find_git_branches(&ctx, &branch_name)?;
    Ok(branches)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn list_branches(
    project_id: ProjectId,
    filter: Option<BranchListingFilter>,
) -> Result<Vec<BranchListing>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let branches = gitbutler_branch_actions::list_branches(&ctx, filter, None)?;
    Ok(branches)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_branch_listing_details(
    project_id: ProjectId,
    branch_names: Vec<String>,
) -> Result<Vec<BranchListingDetails>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let branches = gitbutler_branch_actions::get_branch_listing_details(&ctx, branch_names)?;
    Ok(branches)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn squash_commits(
    project_id: ProjectId,
    stack_id: StackId,
    source_commit_ids: Vec<String>,
    target_commit_id: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
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

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn fetch_from_remotes(
    project_id: ProjectId,
    action: Option<String>,
) -> Result<BaseBranch, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
        &ctx,
        Some(action.unwrap_or_else(|| "unknown".to_string())),
    )?;

    // Updates the project controller with the last fetched timestamp
    //
    // TODO: This cross dependency likely indicates that last_fetched is stored in the wrong place - value is coupled with virtual branches state
    gitbutler_project::update(gitbutler_project::UpdateRequest {
        project_data_last_fetched: Some(project_data_last_fetched.clone()),
        ..gitbutler_project::UpdateRequest::default_with_id(project.id)
    })
    .context("failed to update project with last fetched timestamp")?;

    if let FetchResult::Error { error, .. } = project_data_last_fetched {
        return Err(anyhow!(error).into());
    }

    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    Ok(base_branch)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn move_commit(
    project_id: ProjectId,
    commit_id: String,
    target_stack_id: StackId,
    source_stack_id: StackId,
) -> Result<Option<MoveCommitIllegalAction>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::move_commit(&ctx, target_stack_id, commit_id, source_stack_id)
        .map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn move_branch(
    project_id: ProjectId,
    target_stack_id: StackId,
    target_branch_name: String,
    source_stack_id: StackId,
    subject_branch_name: String,
) -> Result<MoveBranchResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::move_branch(
        &ctx,
        target_stack_id,
        target_branch_name.as_str(),
        source_stack_id,
        subject_branch_name.as_str(),
    )
    .map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn tear_off_branch(
    project_id: ProjectId,
    source_stack_id: StackId,
    subject_branch_name: String,
) -> Result<MoveBranchResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::tear_off_branch(&ctx, source_stack_id, subject_branch_name.as_str())
        .map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn update_commit_message(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    message: String,
) -> Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    let new_commit_id =
        gitbutler_branch_actions::update_commit_message(&ctx, stack_id, commit_id, &message)?;
    Ok(new_commit_id.to_string())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn find_commit(
    project_id: ProjectId,
    commit_id: String,
) -> Result<Option<RemoteCommit>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
    gitbutler_branch_actions::find_commit(&ctx, commit_id).map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn upstream_integration_statuses(
    project_id: ProjectId,
    target_commit_id: Option<String>,
) -> Result<StackStatuses, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let commit_id = target_commit_id
        .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
        .transpose()?;
    Ok(gitbutler_branch_actions::upstream_integration_statuses(
        &ctx, commit_id,
    )?)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn integrate_upstream(
    project_id: ProjectId,
    resolutions: Vec<Resolution>,
    base_branch_resolution: Option<BaseBranchResolution>,
) -> Result<IntegrationOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let outcome =
        gitbutler_branch_actions::integrate_upstream(&ctx, &resolutions, base_branch_resolution)?;

    Ok(outcome)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn resolve_upstream_integration(
    project_id: ProjectId,
    resolution_approach: BaseBranchResolutionApproach,
) -> Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let new_target_id =
        gitbutler_branch_actions::resolve_upstream_integration(&ctx, resolution_approach)?;
    let commit_id = git2::Oid::to_string(&new_target_id);
    Ok(commit_id)
}
