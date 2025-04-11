use super::r#virtual as vbranch;
use crate::branch_upstream_integration;
use crate::branch_upstream_integration::IntegrationStrategy;
use crate::move_commits;
use crate::r#virtual::StackListResult;
use crate::reorder::{self, StackOrder};
use crate::squash::squash_tree;
use crate::upstream_integration::{
    self, BaseBranchResolution, BaseBranchResolutionApproach, IntegrationOutcome, Resolution,
    StackStatuses, UpstreamIntegrationContext,
};
use crate::VirtualBranchHunkRangeMap;
use crate::{
    base,
    base::BaseBranch,
    branch_manager::BranchManagerExt,
    file::RemoteBranchFile,
    remote,
    remote::{RemoteBranchData, RemoteCommit},
    VirtualBranchesExt,
};
use anyhow::{bail, Context, Result};
use but_rebase::RebaseStep;
use but_workspace::commit_engine::DiffSpec;
use but_workspace::stack_ext::StackExt;
use but_workspace::{commit_engine, StackEntry};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_headers::HasCommitHeaders;
use gitbutler_diff::DiffByPathMap;
use gitbutler_operating_modes::assure_open_workspace_mode;
use gitbutler_oplog::{
    entry::{OperationKind, SnapshotDetails},
    OplogExt, SnapshotExt,
};
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_project::FetchResult;
use gitbutler_reference::{ReferenceName, Refname, RemoteRefname};
use gitbutler_repo::RepositoryExt;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{BranchOwnershipClaims, StackId};
use gitbutler_workspace::branch_trees::{update_uncommited_changes, WorkspaceState};

use std::path::PathBuf;
use tracing::instrument;

pub fn create_commit(
    ctx: &CommandContext,
    stack_id: StackId,
    message: &str,
    ownership: Option<&BranchOwnershipClaims>,
) -> Result<git2::Oid> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Creating a commit requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
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
    assure_open_workspace_mode(ctx)
        .context("Testing branch mergability requires open workspace mode")?;
    vbranch::is_remote_branch_mergeable(ctx, branch_name)
}

pub fn list_virtual_branches(ctx: &CommandContext) -> Result<StackListResult> {
    ctx.verify()?;

    assure_open_workspace_mode(ctx)
        .context("Listing virtual branches requires open workspace mode")?;

    vbranch::list_virtual_branches(
        ctx,
        ctx.project().exclusive_worktree_access().write_permission(),
    )
}

pub fn list_virtual_branches_cached(
    ctx: &CommandContext,
    worktree_changes: DiffByPathMap,
) -> Result<StackListResult> {
    ctx.verify()?;

    assure_open_workspace_mode(ctx)
        .context("Listing virtual branches requires open workspace mode")?;

    vbranch::list_virtual_branches_cached(
        ctx,
        ctx.project().exclusive_worktree_access().write_permission(),
        &worktree_changes,
    )
}

pub fn create_virtual_branch(
    ctx: &CommandContext,
    create: &BranchCreateRequest,
) -> Result<StackEntry> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Creating a branch requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let branch_manager = ctx.branch_manager();
    let stack = branch_manager.create_virtual_branch(create, guard.write_permission())?;
    let repo = ctx.gix_repo()?;
    Ok(StackEntry {
        id: stack.id,
        branch_names: stack.heads(false).into_iter().map(Into::into).collect(),
        tip: stack.head(&repo)?.to_gix(),
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
    ctx.verify()?;
    let repo = ctx.repo();
    let handle = ctx.project().virtual_branches();
    let stack = handle.list_all_stacks()?.into_iter().find(|stack| {
        stack
            .source_refname
            .as_ref()
            .is_some_and(|source_refname| source_refname == refname)
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

pub fn list_commit_files(
    ctx: &CommandContext,
    commit_oid: git2::Oid,
) -> Result<Vec<RemoteBranchFile>> {
    crate::file::list_commit_files(ctx.repo(), commit_oid)
}

pub fn set_base_branch(ctx: &CommandContext, target_branch: &RemoteRefname) -> Result<BaseBranch> {
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SetBaseBranch),
        guard.write_permission(),
    );
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
    ctx.verify()?;
    assure_open_workspace_mode(ctx)
        .context("Integrating upstream commits requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
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

pub fn update_virtual_branch(
    ctx: &CommandContext,
    branch_update: BranchUpdateRequest,
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Updating a branch requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());
    let old_branch = ctx
        .project()
        .virtual_branches()
        .get_stack_in_workspace(branch_update.id)?;
    let result = vbranch::update_branch(ctx, &branch_update);
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

pub fn update_branch_order(
    ctx: &CommandContext,
    branch_updates: Vec<BranchUpdateRequest>,
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx)
        .context("Updating branch order requires open workspace mode")?;
    for branch_update in branch_updates {
        let stack = ctx
            .project()
            .virtual_branches()
            .get_stack_in_workspace(branch_update.id)?;
        if branch_update.order != Some(stack.order) {
            vbranch::update_branch(ctx, &branch_update)?;
        }
    }
    Ok(())
}

/// Unapplies a virtual branch and deletes the branch entry from the virtual branch state.
pub fn unapply_without_saving_virtual_branch(
    ctx: &CommandContext,
    stack_id: StackId,
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx)
        .context("Deleting a branch order requires open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    let mut guard = ctx.project().exclusive_worktree_access();
    let state = ctx.project().virtual_branches();
    let default_target = state.get_default_target()?;
    let target_commit = ctx.repo().find_commit(default_target.sha)?;
    // NB: unapply_without_saving is also called from save_and_unapply
    branch_manager.unapply(stack_id, guard.write_permission(), &target_commit, true)?;
    state.delete_branch_entry(&stack_id)
}

pub fn unapply_lines(
    ctx: &CommandContext,
    ownership: &BranchOwnershipClaims,
    lines: VirtualBranchHunkRangeMap,
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Unapply a patch requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardLines),
        guard.write_permission(),
    );

    vbranch::unapply_ownership(ctx, ownership, Some(lines), guard.write_permission())
}

pub fn unapply_ownership(ctx: &CommandContext, ownership: &BranchOwnershipClaims) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Unapply a patch requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardHunk),
        guard.write_permission(),
    );
    vbranch::unapply_ownership(ctx, ownership, None, guard.write_permission())
}

pub fn reset_files(ctx: &CommandContext, stack_id: StackId, files: &[PathBuf]) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Resetting a file requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardFile),
        guard.write_permission(),
    );
    vbranch::reset_files(ctx, stack_id, files, guard.write_permission())
}

pub fn amend(
    ctx: &CommandContext,
    stack_id: StackId,
    commit_oid: git2::Oid,
    worktree_changes: Vec<DiffSpec>,
) -> Result<git2::Oid> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Amending a commit requires open workspace mode")?;
    {
        // commit_engine::create_commit_and_update_refs_with_project is also doing a write lock, so we want to allow this gurd to be dropped first
        let mut guard = ctx.project().exclusive_worktree_access();
        let _ = ctx.create_snapshot(
            SnapshotDetails::new(OperationKind::AmendCommit),
            guard.write_permission(),
        );
    }
    let mut guard = ctx.project().exclusive_worktree_access();
    amend_with_commit_engine(
        ctx,
        stack_id,
        commit_oid,
        worktree_changes,
        guard.write_permission(),
    )
}

/// This is backported version of amending using the new commit engine, in the old API
fn amend_with_commit_engine(
    ctx: &CommandContext,
    stack_id: StackId,
    target_commit_id: git2::Oid,
    worktree_changes: Vec<DiffSpec>,
    perm: &mut WorktreeWritePermission,
) -> Result<git2::Oid> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;

    let vb_state = ctx.project().virtual_branches();
    let mut stack = vb_state.get_stack(stack_id)?;

    if stack.upstream.is_some() && !stack.allow_rebasing {
        // amending to a pushed head commit will cause a force push that is not allowed
        bail!("force-push is not allowed");
    }

    let repo = ctx.gix_repo()?;
    let stack_head = repo.find_commit(stack.head(&repo)?.to_gix())?;

    let temp_commit_id = commit_engine::create_commit(
        &repo,
        commit_engine::Destination::NewCommit {
            parent_commit_id: Some(stack_head.id),
            stack_segment: None,
            message: String::default(),
        },
        None,
        worktree_changes,
        3,
    )?
    .new_commit
    .ok_or_else(|| anyhow::anyhow!("Failed to create temp commit"))?;

    // SquashIntoPreceding doesnt seem to work correclty, so replacing the commit with a squash
    let destination_commit = ctx.repo().find_commit(target_commit_id)?;
    let final_tree = squash_tree(
        ctx,
        &[ctx.repo().find_commit(temp_commit_id.to_git2())?],
        &destination_commit,
    )?;

    let parents: Vec<_> = destination_commit.parents().collect();
    // Create a new commit with the final tree
    let new_commit_oid = ctx
        .repo()
        .commit_with_signature(
            None,
            &destination_commit.author(),
            &destination_commit.author(),
            destination_commit.message().unwrap_or_default(),
            &final_tree,
            &parents.iter().collect::<Vec<_>>(),
            destination_commit.gitbutler_headers(),
        )
        .context("Failed to create a squash commit")?;

    let mut steps = stack.as_rebase_steps(ctx, &repo)?;
    // Replace the commit with the new commit
    for step in &mut steps {
        if let RebaseStep::Pick { commit_id, .. } = step {
            if *commit_id == target_commit_id.to_gix() {
                *commit_id = new_commit_oid.to_gix();
            }
        }
    }

    let default_target = vb_state.get_default_target()?;
    let merge_base = ctx
        .repo()
        .merge_base(stack.head(&repo)?, default_target.sha)?;

    let mut rebase = but_rebase::Rebase::new(&repo, Some(merge_base.to_gix()), None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let output = rebase.rebase()?;

    let new_stack_head = output.top_commit.to_git2();

    let (new_head_oid, new_tree_oid) = if ctx.app_settings().feature_flags.v3 {
        (new_stack_head, None)
    } else {
        #[allow(deprecated)]
        let res = gitbutler_workspace::compute_updated_branch_head(
            ctx.repo(),
            &repo,
            &stack,
            new_stack_head,
            ctx,
        )?;
        (res.head, Some(res.tree))
    };
    stack.set_stack_head(&vb_state, &repo, new_head_oid, new_tree_oid)?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    if ctx.app_settings().feature_flags.v3 {
        update_uncommited_changes(ctx, old_workspace, new_workspace, perm)?;
    } else {
        #[allow(deprecated)]
        gitbutler_workspace::checkout_branch_trees(ctx, perm)?;
    }

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    output
        .commit_mapping
        .iter()
        .find(|(_base, old, _new)| *old == target_commit_id.to_gix())
        .map(|(_base, _old, new)| new.to_git2())
        .ok_or_else(|| anyhow::anyhow!("Failed to find new commit id"))
}

pub fn move_commit_file(
    ctx: &CommandContext,
    stack_id: StackId,
    from_commit_oid: git2::Oid,
    to_commit_oid: git2::Oid,
    ownership: &BranchOwnershipClaims,
) -> Result<git2::Oid> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Amending a commit requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::MoveCommitFile),
        guard.write_permission(),
    );
    vbranch::move_commit_file(ctx, stack_id, from_commit_oid, to_commit_oid, ownership)
}

pub fn undo_commit(ctx: &CommandContext, stack_id: StackId, commit_oid: git2::Oid) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Undoing a commit requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
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
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx)
        .context("Inserting a blank commit requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
        guard.write_permission(),
    );
    vbranch::insert_blank_commit(ctx, stack_id, commit_oid, offset)
}

pub fn reorder_stack(
    ctx: &CommandContext,
    stack_id: StackId,
    stack_order: StackOrder,
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Reordering a commit requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::ReorderCommit),
        guard.write_permission(),
    );
    reorder::reorder_stack(ctx, stack_id, stack_order, guard.write_permission())?;
    Ok(())
}

pub fn reset_virtual_branch(
    ctx: &CommandContext,
    stack_id: StackId,
    target_commit_oid: git2::Oid,
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Resetting a branch requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::UndoCommit),
        guard.write_permission(),
    );
    vbranch::reset_branch(ctx, stack_id, target_commit_oid)
}

pub fn save_and_unapply_virutal_branch(
    ctx: &CommandContext,
    stack_id: StackId,
) -> Result<ReferenceName> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx)
        .context("Converting branch to a real branch requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());
    let branch_manager = ctx.branch_manager();
    let result = branch_manager.save_and_unapply(stack_id, guard.write_permission());

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_branch_unapplied(snapshot_tree, result.as_ref(), guard.write_permission())
    });

    result
}

#[deprecated(note = "use gitbutler_branch_actions::stack::push_stack instead")]
pub fn push_virtual_branch(
    ctx: &CommandContext,
    stack_id: StackId,
    with_force: bool,
    askpass: Option<Option<StackId>>,
) -> Result<vbranch::PushResult> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Pushing a branch requires open workspace mode")?;
    vbranch::push(ctx, stack_id, with_force, askpass)
}

pub fn find_git_branches(ctx: &CommandContext, branch_name: &str) -> Result<Vec<RemoteBranchData>> {
    remote::find_git_branches(ctx, branch_name)
}

pub fn squash_commits(
    ctx: &CommandContext,
    stack_id: StackId,
    source_ids: Vec<git2::Oid>,
    destination_id: git2::Oid,
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Squashing a commit requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
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
    ctx.verify()?;
    assure_open_workspace_mode(ctx)
        .context("Updating a commit message requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
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
) -> Result<()> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx).context("Moving a commit requires open workspace mode")?;
    let mut guard = ctx.project().exclusive_worktree_access();
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

#[instrument(level = tracing::Level::DEBUG, skip(ctx), err(Debug))]
pub fn create_virtual_branch_from_branch(
    ctx: &CommandContext,
    branch: &Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
) -> Result<StackId> {
    ctx.verify()?;
    assure_open_workspace_mode(ctx)
        .context("Creating a virtual branch from a branch open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    let mut guard = ctx.project().exclusive_worktree_access();
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

/// Like [`get_uncommited_files()`], but returns a type that can be re-used with
/// [`crate::list_virtual_branches()`].
pub fn get_uncommited_files_reusable(ctx: &CommandContext) -> Result<DiffByPathMap> {
    let guard = ctx.project().exclusive_worktree_access();
    crate::branch::get_uncommited_files_raw(ctx, guard.read_permission())
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
    fn verify(&self) -> Result<()>;
}

impl Verify for CommandContext {
    fn verify(&self) -> Result<()> {
        let mut guard = self.project().exclusive_worktree_access();
        crate::integration::verify_branch(self, guard.write_permission())
    }
}
