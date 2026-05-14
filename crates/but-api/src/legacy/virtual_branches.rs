use std::collections::HashMap;

use anyhow::{Context as _, Result, anyhow};
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{
    DiffSpec, RefMetadata,
    ref_metadata::{StackId, StackKind, WorkspaceStack},
    sync::RepoExclusive,
};
use but_ctx::{Context, ThreadSafeContext};
use but_error::bail_precondition;
use but_workspace::legacy::ui::{StackEntryNoOpt, StackHeadInfo};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::{
    BaseBranch, BranchListing, BranchListingDetails, BranchListingFilter,
    branch_upstream_integration::IntegrationStrategy,
};

use crate::workspace::upstream_integration::StackStatuses;
use gitbutler_git::GitContextExt as _;
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_project::FetchResult;
use gitbutler_reference::{Refname, normalize_branch_name as normalize_name};
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
    let branch_refname = local_branch_refname(refname, &given_name)?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.legacy_meta_mut(guard.write_permission())?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;

    if ws
        .metadata
        .as_ref()
        .and_then(|metadata| {
            metadata.find_stack_with_branch(branch_refname.as_ref(), StackKind::AppliedAndUnapplied)
        })
        .is_some_and(|stack| stack.is_in_workspace())
    {
        bail_precondition!("Cannot delete a branch that is applied in workspace");
    }

    if let Some(new_ws) = but_workspace::branch::remove_reference(
        branch_refname.as_ref(),
        &repo,
        &ws,
        &mut meta,
        but_workspace::branch::remove_reference::Options {
            avoid_anonymous_stacks: false,
            keep_metadata: false,
        },
    )? {
        *ws = new_ws;
    } else {
        if let Some(reference) = repo.try_find_reference(branch_refname.as_ref())? {
            let safe_delete = but_core::branch::SafeDelete::new(&repo)?;
            let outcome = safe_delete.delete_reference(&reference)?;
            if let Some(paths) = outcome.checked_out_in_worktree_dirs {
                bail_precondition!(
                    "Refusing to delete a branch that is checked out. Worktrees are: {paths:?}"
                );
            }
        }
        meta.remove(branch_refname.as_ref())?;
        if let Some(metadata) = &mut ws.metadata {
            metadata.remove_segment(branch_refname.as_ref());
        }
    }

    Ok(())
}

fn local_branch_refname(refname: Refname, given_name: &str) -> Result<gix::refs::FullName> {
    if let Ok(refname) = gix::refs::FullName::try_from(refname.to_string())
        && refname.as_ref().category() == Some(Category::LocalBranch)
    {
        return Ok(refname);
    }

    Category::LocalBranch
        .to_full_name(given_name)
        .map_err(anyhow::Error::from)
}

/// Turn `branch` into an applied virtual branch, optionally associating
/// `remote` and `pr_number`.
///
/// This acquires exclusive worktree access from `ctx` before applying the
/// branch in the workspace.
///
/// See [`create_virtual_branch_from_branch_with_perm()`] for the underlying
/// mutation.
#[but_api]
#[instrument(err(Debug))]
pub fn create_virtual_branch_from_branch(
    ctx: &mut but_ctx::Context,
    branch: Refname,
    pr_number: Option<usize>,
) -> Result<gitbutler_branch_actions::CreateBranchFromBranchOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let outcome = create_virtual_branch_from_branch_with_perm(
        ctx,
        &branch,
        pr_number,
        guard.write_permission(),
    )?;
    Ok(outcome)
}

/// Turn `branch` into an applied virtual branch, optionally associating
/// `remote` and `pr_number`, while reusing caller-held exclusive access.
///
/// This delegates to
/// [`gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm()`].
pub fn create_virtual_branch_from_branch_with_perm(
    ctx: &mut but_ctx::Context,
    branch: &Refname,
    pr_number: Option<usize>,
    perm: &mut RepoExclusive,
) -> Result<gitbutler_branch_actions::CreateBranchFromBranchOutcome> {
    let outcome = gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
        ctx, branch, pr_number, perm,
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

/// Switch back to the workspace branch state.
///
/// This acquires exclusive worktree access from `ctx` before restoring the
/// workspace branch.
#[but_api]
#[instrument(err(Debug))]
pub fn switch_back_to_workspace(ctx: &mut but_ctx::Context) -> Result<BaseBranch> {
    let mut guard = ctx.exclusive_worktree_access();
    switch_back_to_workspace_with_perm(ctx, guard.write_permission())
}

#[instrument(skip(perm), err(Debug))]
/// Switch back to the workspace branch using an existing exclusive permission token.
///
/// This variant is more composable than [`switch_back_to_workspace`] when the caller already
/// holds a lock, as it reuses the provided permission token instead of obtaining exclusive access
/// itself.
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

/// Set the base branch to `branch`, optionally updating `push_remote` as well.
///
/// This acquires exclusive worktree access from `ctx` before updating the base
/// branch state.
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
/// Set the base branch using an existing exclusive permission token.
///
/// This variant is more composable than [`set_base_branch`] when the caller already holds a lock,
/// as it reuses the provided permission token instead of obtaining exclusive access itself.
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
    let mut guard = ctx.exclusive_worktree_access();
    update_stack_order_with_perm(ctx, stacks, guard.write_permission())
}

/// Update stack order while reusing caller-held exclusive repository access.
///
/// This writes through workspace metadata directly instead of using the legacy
/// `VirtualBranchesHandle` path in `gitbutler-branch-actions`.
pub fn update_stack_order_with_perm(
    ctx: &mut but_ctx::Context,
    stacks: Vec<BranchUpdateRequest>,
    perm: &mut RepoExclusive,
) -> Result<()> {
    ensure_open_workspace_mode(ctx, perm.read_permission())
        .context("Updating branch order requires open workspace mode")?;

    let mut meta = ctx.legacy_meta_mut(perm)?;
    let (_repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let workspace_ref = ws
        .ref_name()
        .context("Updating stack order requires a managed workspace")?;
    let mut workspace_metadata = meta.workspace(workspace_ref)?;
    let changed = apply_stack_order_updates(&mut workspace_metadata, stacks)?;

    if changed {
        let updated_metadata = (*workspace_metadata).clone();
        meta.set_workspace(&workspace_metadata)?;
        meta.set_changed_to_necessitate_write();
        ws.metadata = Some(updated_metadata);
        sort_projected_stacks_like_metadata(&mut ws.stacks, &workspace_metadata.stacks);
    }

    Ok(())
}

fn apply_stack_order_updates(
    workspace: &mut but_core::ref_metadata::Workspace,
    updates: Vec<BranchUpdateRequest>,
) -> Result<bool> {
    let mut requested_orders = HashMap::new();

    for update in updates {
        let stack_id = update.id.context("BUG(opt-stack-id)")?;
        if !workspace
            .stacks
            .iter()
            .any(|stack| stack.id == stack_id && stack.is_in_workspace())
        {
            return Err(anyhow!("branch with ID {stack_id} not found")
                .context(but_error::Code::BranchNotFound));
        }

        if let Some(order) = update.order {
            requested_orders.insert(stack_id, order);
        }
    }

    if requested_orders.is_empty() {
        return Ok(false);
    }

    let original_stack_ids = workspace
        .stacks
        .iter()
        .map(|stack| stack.id)
        .collect::<Vec<_>>();
    let original_orders = original_stack_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(order, stack_id)| (stack_id, order))
        .collect::<HashMap<_, _>>();

    workspace.stacks.sort_by_cached_key(|stack| {
        let order = requested_orders
            .get(&stack.id)
            .copied()
            .unwrap_or_else(|| original_orders[&stack.id]);

        (order, stack.name().map(ToString::to_string), stack.id)
    });

    Ok(workspace
        .stacks
        .iter()
        .map(|stack| stack.id)
        .ne(original_stack_ids))
}

fn sort_projected_stacks_like_metadata(
    stacks: &mut [but_graph::projection::Stack],
    metadata_stacks: &[WorkspaceStack],
) {
    let stack_orders = metadata_stacks
        .iter()
        .enumerate()
        .map(|(order, stack)| (stack.id, order))
        .collect::<HashMap<_, _>>();

    stacks.sort_by_key(|stack| {
        stack
            .id
            .and_then(|stack_id| stack_orders.get(&stack_id).copied())
            .unwrap_or(usize::MAX)
    });
}

#[cfg(test)]
mod tests {
    use but_core::ref_metadata::{
        StackId, Workspace, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    };

    use super::*;

    #[test]
    fn stack_order_updates_reorder_workspace_metadata() -> Result<()> {
        let first = StackId::from_number_for_testing(1);
        let second = StackId::from_number_for_testing(2);
        let third = StackId::from_number_for_testing(3);
        let mut workspace = workspace_with_stacks([
            stack(first, "first", WorkspaceCommitRelation::Merged),
            stack(second, "second", WorkspaceCommitRelation::Merged),
            stack(third, "third", WorkspaceCommitRelation::Merged),
        ]);

        let changed = apply_stack_order_updates(
            &mut workspace,
            vec![
                BranchUpdateRequest {
                    id: Some(first),
                    order: Some(2),
                },
                BranchUpdateRequest {
                    id: Some(second),
                    order: Some(0),
                },
                BranchUpdateRequest {
                    id: Some(third),
                    order: Some(1),
                },
            ],
        )?;

        assert!(changed);
        assert_eq!(stack_ids(&workspace), vec![second, third, first]);
        Ok(())
    }

    #[test]
    fn stack_order_updates_reject_unapplied_stacks() {
        let applied = StackId::from_number_for_testing(1);
        let unapplied = StackId::from_number_for_testing(2);
        let mut workspace = workspace_with_stacks([
            stack(applied, "applied", WorkspaceCommitRelation::Merged),
            stack(unapplied, "unapplied", WorkspaceCommitRelation::Outside),
        ]);

        let result = apply_stack_order_updates(
            &mut workspace,
            vec![BranchUpdateRequest {
                id: Some(unapplied),
                order: Some(0),
            }],
        );

        assert!(result.is_err());
        assert_eq!(stack_ids(&workspace), vec![applied, unapplied]);
    }

    fn workspace_with_stacks<const N: usize>(stacks: [WorkspaceStack; N]) -> Workspace {
        Workspace {
            stacks: stacks.into(),
            ..Default::default()
        }
    }

    fn stack(
        id: StackId,
        name: &str,
        workspacecommit_relation: WorkspaceCommitRelation,
    ) -> WorkspaceStack {
        WorkspaceStack {
            id,
            workspacecommit_relation,
            branches: vec![WorkspaceStackBranch {
                ref_name: gix::refs::FullName::try_from(format!("refs/heads/{name}"))
                    .expect("valid test ref name"),
                archived: false,
            }],
        }
    }

    fn stack_ids(workspace: &Workspace) -> Vec<StackId> {
        workspace.stacks.iter().map(|stack| stack.id).collect()
    }
}

#[but_api(napi)]
#[instrument(err(Debug))]
/// Take the stack identified by `stack_id` out of the workspace.
///
/// This acquires exclusive worktree access from `ctx` before collecting the
/// assigned changes and unapplying the stack.
///
/// See [`unapply_stack_with_perm()`] for how assigned changes are collected before
/// delegating to the underlying mutation.
pub fn unapply_stack(ctx: &mut Context, stack_id: StackId) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    unapply_stack_with_perm(ctx, stack_id, guard.write_permission())
}

/// Take the stack identified by `stack_id` out of the workspace while reusing
/// caller-held exclusive access.
///
/// This computes the currently assigned diffspecs for `stack_id` from the
/// workspace and then delegates to [`gitbutler_branch_actions::unapply_stack()`].
pub fn unapply_stack_with_perm(
    ctx: &mut Context,
    stack_id: StackId,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let context_lines = ctx.settings.context_lines;
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        Some(but_core::diff::ui::worktree_changes(&repo)?.changes),
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
    gitbutler_branch_actions::unapply_stack(ctx, perm, stack_id, assigned_diffspec)?;
    Ok(())
}

#[but_api(napi)]
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
pub fn fetch_from_remotes(ctx: &Context, action: Option<String>) -> Result<BaseBranch> {
    let remotes = {
        let repo = ctx.repo.get()?;
        repo.remote_names()
            .iter()
            .map(|name| name.to_str().map(str::to_owned))
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let askpass = Some(action.unwrap_or_else(|| "unknown".to_string()));
    let fetch_errors: Vec<_> = remotes
        .iter()
        .filter_map(|remote| {
            ctx.fetch(remote, askpass.clone())
                .err()
                .map(|err| err.to_string())
        })
        .collect();

    let timestamp = std::time::SystemTime::now();
    let project_data_last_fetched = if fetch_errors.is_empty() {
        FetchResult::Fetched { timestamp }
    } else {
        FetchResult::Error {
            timestamp,
            error: fetch_errors.join("\n"),
        }
    };
    let mut meta = ctx.legacy_meta()?;
    meta.garbage_collect(&*ctx.repo.get()?)?;

    // Updates the project controller with the last fetched timestamp
    //
    // TODO: This cross dependency likely indicates that last_fetched is stored in the wrong place - value is coupled with virtual branches state
    gitbutler_project::update(gitbutler_project::UpdateRequest {
        project_data_last_fetched: Some(project_data_last_fetched.clone()),
        ..gitbutler_project::UpdateRequest::default_with_id(ctx.legacy_project.id.clone())
    })
    .context("failed to update project with last fetched timestamp")?;

    if let FetchResult::Error { error, .. } = project_data_last_fetched {
        return Err(anyhow!(error));
    }

    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
    Ok(base_branch)
}

/// Compute upstream integration statuses, optionally scoped to `target_commit_id`.
#[but_api]
#[instrument(err(Debug))]
pub async fn upstream_integration_statuses(
    ctx: ThreadSafeContext,
    target_commit_id: Option<gix::ObjectId>,
) -> Result<StackStatuses> {
    let (base_branch, commit_id, ctx) = {
        let ctx = ctx.into_thread_local();
        (
            gitbutler_branch_actions::base::get_base_branch_data(&ctx)?,
            target_commit_id,
            ctx.into_sync(),
        )
    };

    let resolved_reviews = resolve_review_map(ctx.clone(), &base_branch).await?;
    let mut ctx = ctx.into_thread_local();
    crate::workspace::workspace_upstream_integration_statuses(
        &mut ctx,
        commit_id,
        &resolved_reviews,
    )
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
    let ctx = ctx;
    let mut resolved_reviews = HashMap::new();
    let db = &mut *ctx.db.get_cache_mut()?;
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
