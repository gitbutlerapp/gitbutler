use super::r#virtual as vbranch;
use crate::branch_upstream_integration;
use crate::branch_upstream_integration::IntegrationStrategy;
use crate::move_commits;
use crate::reorder::{self, StackOrder};
use crate::upstream_integration::{
    self, BaseBranchResolution, BaseBranchResolutionApproach, BranchStatuses, Resolution,
    UpstreamIntegrationContext,
};
use crate::VirtualBranchHunkRangeMap;
use crate::{
    base,
    base::BaseBranch,
    branch_manager::BranchManagerExt,
    file::RemoteBranchFile,
    remote,
    remote::{RemoteBranch, RemoteBranchData, RemoteCommit},
    VirtualBranchesExt,
};
use anyhow::{Context, Result};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_command_context::CommandContext;
use gitbutler_diff::DiffByPathMap;
use gitbutler_operating_modes::assure_open_workspace_mode;
use gitbutler_oplog::{
    entry::{OperationKind, SnapshotDetails},
    OplogExt, SnapshotExt,
};
use gitbutler_project::{FetchResult, Project};
use gitbutler_reference::{ReferenceName, Refname, RemoteRefname};
use gitbutler_repo::RepositoryExt;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{BranchOwnershipClaims, StackId};
use std::path::PathBuf;
use tracing::instrument;

pub fn create_commit(
    project: &Project,
    stack_id: StackId,
    message: &str,
    ownership: Option<&BranchOwnershipClaims>,
    run_hooks: bool,
) -> Result<git2::Oid> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Creating a commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = ctx.project().prepare_snapshot(guard.read_permission());
    let result = vbranch::commit(&ctx, stack_id, message, ownership, run_hooks).map_err(Into::into);
    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.project().snapshot_commit_creation(
            snapshot_tree,
            result.as_ref().err(),
            message.to_owned(),
            None,
            guard.write_permission(),
        )
    });
    result
}

pub fn can_apply_remote_branch(project: &Project, branch_name: &RemoteRefname) -> Result<bool> {
    let ctx = CommandContext::open(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Testing branch mergability requires open workspace mode")?;
    vbranch::is_remote_branch_mergeable(&ctx, branch_name).map_err(Into::into)
}

pub fn list_virtual_branches(
    project: &Project,
) -> Result<(Vec<vbranch::VirtualBranch>, Vec<gitbutler_diff::FileDiff>)> {
    let ctx = open_with_verify(project)?;

    assure_open_workspace_mode(&ctx)
        .context("Listing virtual branches requires open workspace mode")?;

    vbranch::list_virtual_branches(&ctx, project.exclusive_worktree_access().write_permission())
        .map_err(Into::into)
}

pub fn list_virtual_branches_cached(
    project: &Project,
    worktree_changes: Option<DiffByPathMap>,
) -> Result<(Vec<vbranch::VirtualBranch>, Vec<gitbutler_diff::FileDiff>)> {
    let ctx = open_with_verify(project)?;

    assure_open_workspace_mode(&ctx)
        .context("Listing virtual branches requires open workspace mode")?;

    vbranch::list_virtual_branches_cached(
        &ctx,
        project.exclusive_worktree_access().write_permission(),
        worktree_changes,
    )
    .map_err(Into::into)
}

pub fn create_virtual_branch(project: &Project, create: &BranchCreateRequest) -> Result<StackId> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Creating a branch requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let branch_manager = ctx.branch_manager();
    let stack_id = branch_manager
        .create_virtual_branch(create, guard.write_permission())?
        .id;
    Ok(stack_id)
}

/// Deletes a local branch reference and it's associated virtual branch.
/// If there is a virtual branch and it is applied, this function will return an error.
/// If there is no such local reference, this function will return an error.
pub fn delete_local_branch(project: &Project, refname: &Refname, given_name: String) -> Result<()> {
    let ctx = open_with_verify(project)?;
    let repo = ctx.repository();
    let handle = ctx.project().virtual_branches();
    let stack = handle.list_all_stacks()?.into_iter().find(|stack| {
        stack
            .source_refname
            .as_ref()
            .map_or(false, |source_refname| source_refname == refname)
    });

    if let Some(stack) = stack {
        // Disallow deletion of branches that are applied in workspace
        if stack.in_workspace {
            return Err(anyhow::anyhow!(
                "Cannot delete a branch that is applied in workspace"
            ));
        }
        // Deletes the virtual branch entry from the application state
        handle.delete_branch_entry(&stack.id)?;
    }

    // If a branch reference for this can be found, delete it
    if let Ok(mut branch) = repo.find_branch(&given_name, git2::BranchType::Local) {
        branch.delete()?;
    };
    Ok(())
}

#[instrument(skip(project), err(Debug))]
pub fn get_base_branch_data(project: &Project) -> Result<BaseBranch> {
    let ctx = CommandContext::open(project)?;
    base::get_base_branch_data(&ctx)
}

pub fn list_commit_files(
    project: &Project,
    commit_oid: git2::Oid,
) -> Result<Vec<RemoteBranchFile>> {
    let ctx = CommandContext::open(project)?;
    crate::file::list_commit_files(ctx.repository(), commit_oid).map_err(Into::into)
}

pub fn set_base_branch(project: &Project, target_branch: &RemoteRefname) -> Result<BaseBranch> {
    let ctx = CommandContext::open(project)?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::SetBaseBranch),
        guard.write_permission(),
    );
    base::set_base_branch(&ctx, target_branch)
}

pub fn set_target_push_remote(project: &Project, push_remote: &str) -> Result<()> {
    let ctx = CommandContext::open(project)?;
    base::set_target_push_remote(&ctx, push_remote)
}

pub fn push_base_branch(project: &Project, with_force: bool) -> Result<()> {
    let ctx = CommandContext::open(project)?;
    base::push(&ctx, with_force)
}

pub fn integrate_upstream_commits(
    project: &Project,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Integrating upstream commits requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::MergeUpstream),
        guard.write_permission(),
    );
    branch_upstream_integration::integrate_upstream_commits_for_series(
        &ctx,
        stack_id,
        guard.write_permission(),
        series_name,
        integration_strategy,
    )
    .map_err(Into::into)
}

pub fn update_virtual_branch(project: &Project, branch_update: BranchUpdateRequest) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Updating a branch requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = ctx.project().prepare_snapshot(guard.read_permission());
    let old_branch = ctx
        .project()
        .virtual_branches()
        .get_stack_in_workspace(branch_update.id)?;
    let result = vbranch::update_branch(&ctx, &branch_update);
    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.project().snapshot_branch_update(
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

pub fn update_branch_order(
    project: &Project,
    branch_updates: Vec<BranchUpdateRequest>,
) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Updating branch order requires open workspace mode")?;
    for branch_update in branch_updates {
        let stack = ctx
            .project()
            .virtual_branches()
            .get_stack_in_workspace(branch_update.id)?;
        if branch_update.order != Some(stack.order) {
            vbranch::update_branch(&ctx, &branch_update)?;
        }
    }
    Ok(())
}

/// Unapplies a virtual branch and deletes the branch entry from the virtual branch state.
pub fn unapply_without_saving_virtual_branch(project: &Project, stack_id: StackId) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Deleting a branch order requires open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let state = ctx.project().virtual_branches();
    let default_target = state.get_default_target()?;
    let target_commit = ctx.repository().find_commit(default_target.sha)?;
    // NB: unapply_without_saving is also called from save_and_unapply
    branch_manager.unapply(stack_id, guard.write_permission(), &target_commit, true)?;
    state.delete_branch_entry(&stack_id)
}

pub fn unapply_lines(
    project: &Project,
    ownership: &BranchOwnershipClaims,
    lines: VirtualBranchHunkRangeMap,
) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Unapply a patch requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardLines),
        guard.write_permission(),
    );

    vbranch::unapply_ownership(&ctx, ownership, Some(lines), guard.write_permission())
        .map_err(Into::into)
}

pub fn unapply_ownership(project: &Project, ownership: &BranchOwnershipClaims) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Unapply a patch requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardHunk),
        guard.write_permission(),
    );
    vbranch::unapply_ownership(&ctx, ownership, None, guard.write_permission()).map_err(Into::into)
}

pub fn reset_files(project: &Project, stack_id: StackId, files: &[PathBuf]) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Resetting a file requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardFile),
        guard.write_permission(),
    );
    vbranch::reset_files(&ctx, stack_id, files, guard.write_permission()).map_err(Into::into)
}

pub fn amend(
    project: &Project,
    stack_id: StackId,
    commit_oid: git2::Oid,
    ownership: &BranchOwnershipClaims,
) -> Result<git2::Oid> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Amending a commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::AmendCommit),
        guard.write_permission(),
    );
    vbranch::amend(
        &ctx,
        stack_id,
        commit_oid,
        ownership,
        guard.write_permission(),
    )
}

pub fn move_commit_file(
    project: &Project,
    stack_id: StackId,
    from_commit_oid: git2::Oid,
    to_commit_oid: git2::Oid,
    ownership: &BranchOwnershipClaims,
) -> Result<git2::Oid> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Amending a commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::MoveCommitFile),
        guard.write_permission(),
    );
    vbranch::move_commit_file(&ctx, stack_id, from_commit_oid, to_commit_oid, ownership)
        .map_err(Into::into)
}

pub fn undo_commit(project: &Project, stack_id: StackId, commit_oid: git2::Oid) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Undoing a commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = ctx.project().prepare_snapshot(guard.read_permission());
    let result: Result<()> = crate::undo_commit::undo_commit(&ctx, stack_id, commit_oid)
        .map(|_| ())
        .map_err(Into::into);
    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.project().snapshot_commit_undo(
            snapshot_tree,
            result.as_ref(),
            commit_oid,
            guard.write_permission(),
        )
    });
    result
}

pub fn insert_blank_commit(
    project: &Project,
    stack_id: StackId,
    commit_oid: git2::Oid,
    offset: i32,
) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Inserting a blank commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
        guard.write_permission(),
    );
    vbranch::insert_blank_commit(&ctx, stack_id, commit_oid, offset).map_err(Into::into)
}

pub fn reorder_stack(project: &Project, stack_id: StackId, stack_order: StackOrder) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Reordering a commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::ReorderCommit),
        guard.write_permission(),
    );
    reorder::reorder_stack(&ctx, stack_id, stack_order, guard.write_permission())
}

pub fn reset_virtual_branch(
    project: &Project,
    stack_id: StackId,
    target_commit_oid: git2::Oid,
) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Resetting a branch requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::UndoCommit),
        guard.write_permission(),
    );
    vbranch::reset_branch(&ctx, stack_id, target_commit_oid).map_err(Into::into)
}

pub fn save_and_unapply_virutal_branch(
    project: &Project,
    stack_id: StackId,
) -> Result<ReferenceName> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Converting branch to a real branch requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = ctx.project().prepare_snapshot(guard.read_permission());
    let branch_manager = ctx.branch_manager();
    let result = branch_manager.save_and_unapply(stack_id, guard.write_permission());

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.project().snapshot_branch_unapplied(
            snapshot_tree,
            result.as_ref(),
            guard.write_permission(),
        )
    });

    result
}

pub fn push_virtual_branch(
    project: &Project,
    stack_id: StackId,
    with_force: bool,
    askpass: Option<Option<StackId>>,
) -> Result<vbranch::PushResult> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Pushing a branch requires open workspace mode")?;
    vbranch::push(&ctx, stack_id, with_force, askpass)
}

pub fn list_local_branches(project: Project) -> Result<Vec<RemoteBranch>> {
    let ctx = CommandContext::open(&project)?;
    remote::list_local_branches(&ctx)
}

pub fn get_remote_branch_data(project: &Project, refname: &Refname) -> Result<RemoteBranchData> {
    let ctx = CommandContext::open(project)?;
    remote::get_branch_data(&ctx, refname)
}

pub fn squash(project: &Project, stack_id: StackId, commit_oid: git2::Oid) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Squashing a commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::SquashCommit),
        guard.write_permission(),
    );
    vbranch::squash(&ctx, stack_id, commit_oid).map_err(Into::into)
}

pub fn update_commit_message(
    project: &Project,
    stack_id: StackId,
    commit_oid: git2::Oid,
    message: &str,
) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Updating a commit message requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateCommitMessage),
        guard.write_permission(),
    );
    vbranch::update_commit_message(&ctx, stack_id, commit_oid, message).map_err(Into::into)
}

pub fn find_commit(project: &Project, commit_oid: git2::Oid) -> Result<Option<RemoteCommit>> {
    let ctx = CommandContext::open(project)?;
    remote::get_commit_data(&ctx, commit_oid)
}

pub fn fetch_from_remotes(project: &Project, askpass: Option<String>) -> Result<FetchResult> {
    let ctx = CommandContext::open(project)?;

    let remotes = ctx.repository().remotes_as_string()?;
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

    state.garbage_collect(ctx.repository())?;

    Ok(project_data_last_fetched)
}

pub fn move_commit(
    project: &Project,
    target_stack_id: StackId,
    commit_oid: git2::Oid,
    source_stack_id: StackId,
) -> Result<()> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx).context("Moving a commit requires open workspace mode")?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::MoveCommit),
        guard.write_permission(),
    );
    move_commits::move_commit(
        &ctx,
        target_stack_id,
        commit_oid,
        guard.write_permission(),
        source_stack_id,
    )
    .map_err(Into::into)
}

#[instrument(level = tracing::Level::DEBUG, skip(project), err(Debug))]
pub fn create_virtual_branch_from_branch(
    project: &Project,
    branch: &Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
) -> Result<StackId> {
    let ctx = open_with_verify(project)?;
    assure_open_workspace_mode(&ctx)
        .context("Creating a virtual branch from a branch open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    branch_manager
        .create_virtual_branch_from_branch(branch, remote, pr_number, guard.write_permission())
        .map_err(Into::into)
}

pub fn get_uncommited_files(project: &Project) -> Result<Vec<RemoteBranchFile>> {
    let context = CommandContext::open(project)?;
    let guard = project.exclusive_worktree_access();
    crate::branch::get_uncommited_files(&context, guard.read_permission())
}

/// Like [`get_uncommited_files()`], but returns a type that can be re-used with
/// [`crate::list_virtual_branches()`].
pub fn get_uncommited_files_reusable(project: &Project) -> Result<DiffByPathMap> {
    let context = CommandContext::open(project)?;
    let guard = project.exclusive_worktree_access();
    crate::branch::get_uncommited_files_raw(&context, guard.read_permission())
}

pub fn upstream_integration_statuses(
    project: &Project,
    target_commit_oid: Option<git2::Oid>,
) -> Result<BranchStatuses> {
    let command_context = CommandContext::open(project)?;
    let mut guard = project.exclusive_worktree_access();

    let context = UpstreamIntegrationContext::open(
        &command_context,
        target_commit_oid,
        guard.write_permission(),
    )?;

    upstream_integration::upstream_integration_statuses(&context)
}

pub fn integrate_upstream(
    project: &Project,
    resolutions: &[Resolution],
    base_branch_resolution: Option<BaseBranchResolution>,
) -> Result<()> {
    let command_context = CommandContext::open(project)?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_context.project().create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateWorkspaceBase),
        guard.write_permission(),
    );

    upstream_integration::integrate_upstream(
        &command_context,
        resolutions,
        base_branch_resolution,
        guard.write_permission(),
    )
}

pub fn resolve_upstream_integration(
    project: &Project,
    resolution_approach: BaseBranchResolutionApproach,
) -> Result<git2::Oid> {
    let command_context = CommandContext::open(project)?;
    let mut guard = project.exclusive_worktree_access();

    upstream_integration::resolve_upstream_integration(
        &command_context,
        resolution_approach,
        guard.write_permission(),
    )
}

pub(crate) fn open_with_verify(project: &Project) -> Result<CommandContext> {
    let ctx = CommandContext::open(project)?;
    let mut guard = project.exclusive_worktree_access();

    crate::integration::verify_branch(&ctx, guard.write_permission())?;
    Ok(ctx)
}
