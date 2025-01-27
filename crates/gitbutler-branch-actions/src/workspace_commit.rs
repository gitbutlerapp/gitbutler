use std::{cmp::Ordering, collections::HashMap};

use anyhow::{Context as _, Result};
use but_rebase::{Rebase, RebaseStep};
use but_workspace::StackId;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_diff::diff_files_into_hunks;
use gitbutler_operating_modes::{assure_open_workspace_mode, WORKSPACE_BRANCH_REF};
use gitbutler_oxidize::{ObjectIdExt as _, OidExt as _};
use gitbutler_project::access::{WorktreeReadPermission, WorktreeWritePermission};
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gitbutler_stack::VirtualBranchesHandle;

use crate::{
    compute_workspace_dependencies, integration::GITBUTLER_INTEGRATION_COMMIT_TITLE,
    update_workspace_commit, BranchManagerExt as _, GITBUTLER_WORKSPACE_COMMIT_TITLE,
};

#[derive(Debug)]
pub enum WorkspaceCommitStatus {
    /// gitbutler/workspace has a workspace commit, but it has extra commits
    /// above it.
    WorkspaceCommitBehind {
        workspace_commit: git2::Oid,
        extra_commits: Vec<git2::Oid>,
    },
    /// gitbutler/workspace has a workspace commit, and the workspace commit is
    /// the head of gitbutler/workspace
    OnWorkspaceCommit { workspace_commit: git2::Oid },
    /// gitbutler/workspace does not have workspace_commit
    NoWorkspaceCommit,
}

/// Returns the current state of the workspace commit, whether it's non-existant
/// the head of gitbutler/workspace, or has commits above
pub fn workspace_commit_status(
    ctx: &CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<WorkspaceCommitStatus> {
    assure_open_workspace_mode(ctx)?;
    let repository = ctx.repo();
    let vb_handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let default_target = vb_handle.get_default_target()?;

    let head_commit = repository
        .find_reference(WORKSPACE_BRANCH_REF)?
        .peel_to_commit()?;

    let mut revwalk = repository.revwalk()?;
    revwalk.push(head_commit.id())?;
    revwalk.hide(default_target.sha)?;

    let mut extra_commits = vec![];
    let mut workspace_commit = None;

    for oid in revwalk {
        let commit = repository.find_commit(oid?)?;

        let is_workspace_commit = commit.message().is_some_and(|message| {
            message.starts_with(GITBUTLER_WORKSPACE_COMMIT_TITLE)
                || message.starts_with(GITBUTLER_INTEGRATION_COMMIT_TITLE)
        });

        if is_workspace_commit {
            workspace_commit = Some(commit.id());
            break;
        } else {
            extra_commits.push(commit.id());
        }
    }

    let Some(workspace_commit) = workspace_commit else {
        return Ok(WorkspaceCommitStatus::NoWorkspaceCommit);
    };

    if extra_commits.is_empty() {
        // no extra commits found, so we're good
        return Ok(WorkspaceCommitStatus::OnWorkspaceCommit { workspace_commit });
    }

    Ok(WorkspaceCommitStatus::WorkspaceCommitBehind {
        workspace_commit,
        extra_commits,
    })
}

/// Resolves the situation if there are commits above the workspace merge commit.
///
/// This function should only be run in open workspace mode.
///
/// This function tries to move the commits into a branch into the workspace if
/// possible, or will remove the commits, leaving the changes in the working
/// directory.
///
/// If there are no branches in the workspace this function will create a new
/// banch for them, rather than simply dropping them.
pub fn resolve_commits_above(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    assure_open_workspace_mode(ctx)?;
    let repository = ctx.repo();
    let gix_repository = ctx.gix_repo()?;
    let head_commit = repository.head()?.peel_to_commit()?;

    let WorkspaceCommitStatus::WorkspaceCommitBehind {
        workspace_commit,
        extra_commits,
    } = workspace_commit_status(ctx, perm.read_permission())?
    else {
        return Ok(());
    };

    let best_stack_id =
        find_or_create_branch_for_commit(ctx, perm, head_commit.id(), workspace_commit)?;

    if let Some(best_stack_id) = best_stack_id {
        let vb_handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
        let mut stack = vb_handle.get_stack_in_workspace(best_stack_id)?;

        let mut rebase = Rebase::new(&gix_repository, stack.head(&gix_repository)?.to_gix(), None)?;
        let mut steps = vec![];
        for commit in &extra_commits {
            steps.push(RebaseStep::Pick {
                commit_id: commit.to_gix(),
                new_message: None,
            })
        }
        rebase.steps(steps)?;
        let outcome = rebase.rebase()?;

        let new_head = repository.find_commit(outcome.top_commit.to_git2())?;

        stack.set_stack_head(
            &vb_handle,
            &gix_repository,
            new_head.id(),
            Some(new_head.tree_id()),
        )?;

        update_workspace_commit(&vb_handle, ctx)?;
    } else {
        // There is no stack which can hold the commits so we should just unroll those changes
        repository.reference(WORKSPACE_BRANCH_REF, workspace_commit, true, "")?;
        repository.set_head(WORKSPACE_BRANCH_REF)?;
    }

    Ok(())
}

/// Tries to find a branch or create a branch that the commits can be moved into.
///
/// Uses the following logic:
/// if there are no branches applied:
///     create a new branch
/// otherwise:
///     if the changes lock to 2 or more branches
///         there is no branch that can accept these commits
///     if the chances lock to 1 branch
///         return that branch
///     otherwise:
///         return the branch currently selected for changes
fn find_or_create_branch_for_commit(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
    head_commit: git2::Oid,
    workspace_commit: git2::Oid,
) -> Result<Option<StackId>> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let default_target = vb_state.get_default_target()?;
    let repository = ctx.repo();
    let stacks = vb_state.list_stacks_in_workspace()?;

    let head_commit = repository.find_commit(head_commit)?;

    let diffs = gitbutler_diff::trees(
        ctx.repo(),
        &repository.find_commit(workspace_commit)?.tree()?,
        &head_commit.tree()?,
        true,
    )?;
    let base_diffs: HashMap<_, _> = diff_files_into_hunks(&diffs).collect();
    let workspace_dependencies =
        compute_workspace_dependencies(ctx, &default_target.sha, &base_diffs, &stacks)?;

    match workspace_dependencies.commit_dependent_diffs.len().cmp(&1) {
        Ordering::Greater => {
            // The commits are locked to multiple stacks. We can't correctly assign it
            // to any one stack, so the commits should be undone.
            Ok(None)
        }
        Ordering::Equal => {
            // There is one stack which the commits are locked to, so the commits
            // should be added to that particular stack.
            let stack_id = workspace_dependencies
                .commit_dependent_diffs
                .keys()
                .next()
                .expect("Values was asserted length 1 above");
            Ok(Some(*stack_id))
        }
        Ordering::Less => {
            // We should return the branch selected for changes, or create a new default branch.
            let mut stacks = vb_state.list_stacks_in_workspace()?;
            stacks.sort_by_key(|stack| stack.selected_for_changes.unwrap_or(0));

            if let Some(stack) = stacks.last() {
                return Ok(Some(stack.id));
            }

            let branch_manager = ctx.branch_manager();
            let new_stack = branch_manager
                .create_virtual_branch(
                    &BranchCreateRequest {
                        name: Some(head_commit.message_bstr().to_string()),
                        ..Default::default()
                    },
                    perm,
                )
                .context("failed to create virtual branch")?;

            Ok(Some(new_stack.id))
        }
    }
}
