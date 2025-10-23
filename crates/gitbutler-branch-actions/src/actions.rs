use anyhow::{Context, Result};
use but_workspace::{DiffSpec, commit_engine, stack_heads_info, ui};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::{
    OplogExt, SnapshotExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_project::{FetchResult, access::WorktreeWritePermission};
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::RepositoryExt;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{BranchOwnershipClaims, StackId};
use tracing::instrument;

use super::r#virtual as vbranch;
use crate::{
    VirtualBranchesExt, base,
    base::BaseBranch,
    branch_manager::BranchManagerExt,
    branch_upstream_integration,
    branch_upstream_integration::IntegrationStrategy,
    file::RemoteBranchFile,
    move_branch::MoveBranchResult,
    move_commits::{self, MoveCommitIllegalAction},
    remote,
    remote::{RemoteBranchData, RemoteCommit},
    reorder::{self, StackOrder},
    upstream_integration::{
        self, BaseBranchResolution, BaseBranchResolutionApproach, IntegrationOutcome, Resolution,
        StackStatuses, UpstreamIntegrationContext,
    },
};

pub fn create_commit(
    ctx: &CommandContext,
    stack_id: StackId,
    message: &str,
    ownership: Option<&BranchOwnershipClaims>,
) -> Result<git2::Oid> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Creating a commit requires open workspace mode")?;
    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());
    let result = vbranch::commit(ctx, stack_id, message, ownership);

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_creation(
            snapshot_tree,
            result.as_ref().err(),
            message.to_owned(),
            None,
            guard.write_permission(),
        )
    });

    result
}

pub fn can_apply_remote_branch(ctx: &CommandContext, branch_name: &RemoteRefname) -> Result<bool> {
    ensure_open_workspace_mode(ctx)
        .context("Testing branch mergability requires open workspace mode")?;
    vbranch::is_remote_branch_mergeable(ctx, branch_name)
}

pub fn create_virtual_branch(
    ctx: &CommandContext,
    create: &BranchCreateRequest,
    perm: &mut WorktreeWritePermission,
) -> Result<ui::StackEntryNoOpt> {
    ctx.verify(perm)?;
    ensure_open_workspace_mode(ctx).context("Creating a branch requires open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    let stack = branch_manager.create_virtual_branch(create, perm)?;
    let repo = ctx.gix_repo()?;
    Ok(ui::StackEntryNoOpt {
        id: stack.id,
        heads: stack_heads_info(&stack, &repo)?,
        tip: stack.head_oid(&repo)?,
        order: Some(stack.order),
        is_checked_out: false,
    })
}

/// Deletes a local branch reference and it's associated virtual branch.
/// If there is a virtual branch and it is applied, this function will return an error.
/// If there is no such local reference, this function will return an error.
pub fn delete_local_branch(
    ctx: &CommandContext,
    refname: &Refname,
    given_name: String,
) -> Result<()> {
    ctx.verify(ctx.project().exclusive_worktree_access().write_permission())?;
    let repo = ctx.repo();
    let handle = ctx.project().virtual_branches();
    let stack = handle.list_all_stacks()?.into_iter().find(|stack| {
        stack
            .source_refname
            .as_ref()
            .is_some_and(|source_refname| source_refname == refname)
            || stack.heads(false).contains(&given_name)
    });

    if let Some(mut stack) = stack {
        // Disallow deletion of branches that are applied in workspace
        if stack.in_workspace {
            return Err(anyhow::anyhow!(
                "Cannot delete a branch that is applied in workspace"
            ));
        }
        // Delete the branch head or if it is the only one, delete the entire stack
        if stack.heads.len() > 1 {
            stack.remove_branch(ctx, given_name.clone())?;
        } else {
            handle.delete_branch_entry(&stack.id)?;
        }
    }

    // If a branch reference for this can be found, delete it
    if let Ok(mut branch) = repo.find_branch(&given_name, git2::BranchType::Local) {
        branch.delete()?;
    };
    Ok(())
}

pub fn list_commit_files(
    ctx: &CommandContext,
    commit_oid: git2::Oid,
) -> Result<Vec<RemoteBranchFile>> {
    crate::file::list_commit_files(ctx.repo(), commit_oid)
}

pub fn set_base_branch(
    ctx: &CommandContext,
    target_branch: &RemoteRefname,
    perm: &mut WorktreeWritePermission,
) -> Result<BaseBranch> {
    let _ = ctx.create_snapshot(SnapshotDetails::new(OperationKind::SetBaseBranch), perm);
    base::set_base_branch(ctx, target_branch)
}

pub fn set_target_push_remote(ctx: &CommandContext, push_remote: &str) -> Result<()> {
    base::set_target_push_remote(ctx, push_remote)
}

pub fn push_base_branch(ctx: &CommandContext, with_force: bool) -> Result<()> {
    base::push(ctx, with_force)
}

pub fn integrate_upstream_commits(
    ctx: &CommandContext,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<()> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx)
        .context("Integrating upstream commits requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::MergeUpstream),
        guard.write_permission(),
    );
    branch_upstream_integration::integrate_upstream_commits_for_series(
        ctx,
        stack_id,
        guard.write_permission(),
        series_name,
        integration_strategy,
    )
}

pub fn get_initial_integration_steps_for_branch(
    ctx: &CommandContext,
    stack_id: Option<StackId>,
    branch_name: String,
) -> Result<Vec<branch_upstream_integration::InteractiveIntegrationStep>> {
    ensure_open_workspace_mode(ctx)
        .context("Getting initial integration steps requires open workspace mode")?;
    branch_upstream_integration::get_initial_integration_steps_for_branch(
        ctx,
        stack_id,
        branch_name,
    )
}

pub fn integrate_branch_with_steps(
    ctx: &CommandContext,
    stack_id: StackId,
    branch_name: String,
    steps: Vec<branch_upstream_integration::InteractiveIntegrationStep>,
) -> Result<()> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx)
        .context("Integrating a branch with steps requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::MergeUpstream),
        guard.write_permission(),
    );
    branch_upstream_integration::integrate_branch_with_steps(
        ctx,
        stack_id,
        branch_name,
        steps,
        guard.write_permission(),
    )
}

pub fn update_virtual_branch(
    ctx: &CommandContext,
    branch_update: BranchUpdateRequest,
) -> Result<()> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Updating a branch requires open workspace mode")?;
    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());
    let old_branch = ctx
        .project()
        .virtual_branches()
        .get_stack_in_workspace(branch_update.id.context("BUG(opt-stack-id)")?)?;
    let result = vbranch::update_stack(ctx, &branch_update);
    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_branch_update(
            snapshot_tree,
            &old_branch,
            &branch_update,
            result.as_ref().err(),
            guard.write_permission(),
        )
    });
    result?;
    Ok(())
}

pub fn update_stack_order(ctx: &CommandContext, updates: Vec<BranchUpdateRequest>) -> Result<()> {
    ctx.verify(ctx.project().exclusive_worktree_access().write_permission())?;
    ensure_open_workspace_mode(ctx)
        .context("Updating branch order requires open workspace mode")?;
    for stack_update in updates {
        let stack = ctx
            .project()
            .virtual_branches()
            .get_stack_in_workspace(stack_update.id.context("BUG(opt-stack-id)")?)?;
        if stack_update.order != Some(stack.order) {
            vbranch::update_stack(ctx, &stack_update)?;
        }
    }
    Ok(())
}

/// Unapplies a virtual branch and deletes the branch entry from the virtual branch state.
pub fn unapply_stack(
    ctx: &CommandContext,
    stack_id: StackId,
    assigned_diffspec: Vec<but_workspace::DiffSpec>,
) -> Result<String> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx)
        .context("Deleting a branch order requires open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    // NB: unapply_without_saving is also called from save_and_unapply
    let branch_name = branch_manager.unapply(
        stack_id,
        guard.write_permission(),
        false,
        assigned_diffspec,
        ctx.app_settings().feature_flags.cv3,
    )?;
    Ok(branch_name)
}

pub fn amend(
    ctx: &CommandContext,
    stack_id: StackId,
    commit_oid: git2::Oid,
    worktree_changes: Vec<DiffSpec>,
) -> Result<git2::Oid> {
    ctx.verify(ctx.project().exclusive_worktree_access().write_permission())?;
    ensure_open_workspace_mode(ctx).context("Amending a commit requires open workspace mode")?;
    {
        // commit_engine::create_commit_and_update_refs_with_project is also doing a write lock, so we want to allow this gurd to be dropped first
        let mut guard = ctx.project().exclusive_worktree_access();
        let _ = ctx.create_snapshot(
            SnapshotDetails::new(OperationKind::AmendCommit),
            guard.write_permission(),
        );
    }
    amend_with_commit_engine(ctx, stack_id, commit_oid, worktree_changes)
}

/// This is backported version of amending using the new commit engine, in the old API
fn amend_with_commit_engine(
    ctx: &CommandContext,
    stack_id: StackId,
    commit_oid: git2::Oid,
    worktree_changes: Vec<DiffSpec>,
) -> Result<git2::Oid> {
    let mut guard = ctx.project().exclusive_worktree_access();

    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &ctx.gix_repo()?,
        ctx.project(),
        Some(stack_id),
        commit_engine::Destination::AmendCommit {
            commit_id: commit_oid.to_gix(),
            new_message: None,
        },
        None,
        worktree_changes,
        3, // for the old API this is hardcoded
        guard.write_permission(),
    )?;
    let new_commit = outcome.new_commit.ok_or(anyhow::anyhow!(
        "Failed to amend with commit engine. Rejected specs: {:?}",
        outcome.rejected_specs
    ))?;
    Ok(new_commit.to_git2())
}

pub fn undo_commit(ctx: &CommandContext, stack_id: StackId, commit_oid: git2::Oid) -> Result<()> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Undoing a commit requires open workspace mode")?;
    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());
    let result: Result<()> =
        crate::undo_commit::undo_commit(ctx, stack_id, commit_oid, guard.write_permission())
            .map(|_| ());
    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_undo(
            snapshot_tree,
            result.as_ref(),
            commit_oid,
            guard.write_permission(),
        )
    });
    result
}

pub fn insert_blank_commit(
    ctx: &CommandContext,
    stack_id: StackId,
    commit_oid: git2::Oid,
    offset: i32,
    message: Option<&str>,
) -> Result<(gix::ObjectId, Vec<(gix::ObjectId, gix::ObjectId)>)> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx)
        .context("Inserting a blank commit requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
        guard.write_permission(),
    );
    vbranch::insert_blank_commit(ctx, stack_id, commit_oid, offset, message)
}

pub fn reorder_stack(
    ctx: &CommandContext,
    stack_id: StackId,
    stack_order: StackOrder,
) -> Result<()> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Reordering a commit requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::ReorderCommit),
        guard.write_permission(),
    );
    reorder::reorder_stack(ctx, stack_id, stack_order, guard.write_permission())?;
    Ok(())
}

pub fn find_git_branches(ctx: &CommandContext, branch_name: &str) -> Result<Vec<RemoteBranchData>> {
    remote::find_git_branches(ctx, branch_name)
}

pub fn squash_commits(
    ctx: &CommandContext,
    stack_id: StackId,
    source_ids: Vec<git2::Oid>,
    destination_id: git2::Oid,
) -> Result<git2::Oid> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Squashing a commit requires open workspace mode")?;
    crate::squash::squash_commits(
        ctx,
        stack_id,
        source_ids,
        destination_id,
        guard.write_permission(),
    )
}

pub fn update_commit_message(
    ctx: &CommandContext,
    stack_id: StackId,
    commit_oid: git2::Oid,
    message: &str,
) -> Result<git2::Oid> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx)
        .context("Updating a commit message requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateCommitMessage),
        guard.write_permission(),
    );
    vbranch::update_commit_message(ctx, stack_id, commit_oid, message)
}

pub fn find_commit(ctx: &CommandContext, commit_oid: git2::Oid) -> Result<Option<RemoteCommit>> {
    remote::get_commit_data(ctx, commit_oid)
}

pub fn fetch_from_remotes(ctx: &CommandContext, askpass: Option<String>) -> Result<FetchResult> {
    let remotes = ctx.repo().remotes_as_string()?;
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
    let state = ctx.project().virtual_branches();

    state.garbage_collect(ctx.repo())?;

    Ok(project_data_last_fetched)
}

pub fn move_commit(
    ctx: &CommandContext,
    target_stack_id: StackId,
    commit_oid: git2::Oid,
    source_stack_id: StackId,
) -> Result<Option<MoveCommitIllegalAction>> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Moving a commit requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::MoveCommit),
        guard.write_permission(),
    );
    move_commits::move_commit(
        ctx,
        target_stack_id,
        commit_oid,
        guard.write_permission(),
        source_stack_id,
    )
}

pub fn move_branch(
    ctx: &CommandContext,
    target_stack_id: StackId,
    target_branch_name: &str,
    source_stack_id: StackId,
    subject_branch_name: &str,
) -> Result<MoveBranchResult> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Moving a branch requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::MoveBranch),
        guard.write_permission(),
    );
    crate::move_branch::move_branch(
        ctx,
        target_stack_id,
        target_branch_name,
        source_stack_id,
        subject_branch_name,
        guard.write_permission(),
    )
}

pub fn tear_off_branch(
    ctx: &CommandContext,
    source_stack_id: StackId,
    subject_branch_name: &str,
) -> Result<MoveBranchResult> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx).context("Moving a branch requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::TearOffBranch),
        guard.write_permission(),
    );
    crate::move_branch::tear_off_branch(
        ctx,
        source_stack_id,
        subject_branch_name,
        guard.write_permission(),
    )
}

#[instrument(level = tracing::Level::DEBUG, skip(ctx), err(Debug))]
pub fn create_virtual_branch_from_branch(
    ctx: &CommandContext,
    branch: &Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
) -> Result<(StackId, Vec<StackId>)> {
    let mut guard = ctx.project().exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx)
        .context("Creating a virtual branch from a branch open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    branch_manager.create_virtual_branch_from_branch(
        branch,
        remote,
        pr_number,
        guard.write_permission(),
    )
}

pub fn get_uncommited_files(ctx: &CommandContext) -> Result<Vec<RemoteBranchFile>> {
    let guard = ctx.project().exclusive_worktree_access();
    crate::branch::get_uncommited_files(ctx, guard.read_permission())
}

pub fn upstream_integration_statuses(
    ctx: &CommandContext,
    target_commit_oid: Option<git2::Oid>,
) -> Result<StackStatuses> {
    let mut guard = ctx.project().exclusive_worktree_access();

    let gix_repo = ctx.gix_repo()?;
    let context = UpstreamIntegrationContext::open(
        ctx,
        target_commit_oid,
        guard.write_permission(),
        &gix_repo,
    )?;

    upstream_integration::upstream_integration_statuses(&context)
}

pub fn integrate_upstream(
    ctx: &CommandContext,
    resolutions: &[Resolution],
    base_branch_resolution: Option<BaseBranchResolution>,
) -> Result<IntegrationOutcome> {
    let mut guard = ctx.project().exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateWorkspaceBase),
        guard.write_permission(),
    );

    upstream_integration::integrate_upstream(
        ctx,
        resolutions,
        base_branch_resolution,
        guard.write_permission(),
    )
}

pub fn resolve_upstream_integration(
    ctx: &CommandContext,
    resolution_approach: BaseBranchResolutionApproach,
) -> Result<git2::Oid> {
    let mut guard = ctx.project().exclusive_worktree_access();

    upstream_integration::resolve_upstream_integration(
        ctx,
        resolution_approach,
        guard.write_permission(),
    )
}

pub(crate) trait Verify {
    fn verify(&self, perm: &mut WorktreeWritePermission) -> Result<()>;
}

impl Verify for CommandContext {
    fn verify(&self, perm: &mut WorktreeWritePermission) -> Result<()> {
        crate::integration::verify_branch(self, perm)
    }
}
