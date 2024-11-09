use std::collections::HashMap;

use anyhow::{Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project::Project;
use gitbutler_reference::normalize_branch_name;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{CommitOrChangeId, PatchReferenceUpdate, StackBranch};
use gitbutler_stack::{Stack, StackId, Target};
use serde::{Deserialize, Serialize};

use crate::{
    actions::open_with_verify,
    commit::{commit_to_vbranch_commit, VirtualBranchCommit},
    r#virtual::{CommitData, IsCommitIntegrated, PatchSeries},
    VirtualBranchesExt,
};
use gitbutler_operating_modes::assure_open_workspace_mode;

/// Adds a new "series/branch" to the Stack.
/// This is in fact just creating a new  GitButler patch reference (head) and associates it with the stack.
/// The name cannot be the same as existing git references or existing patch references.
/// The target must reference a commit (or change) that is part of the stack.
/// The branch name must be a valid reference name (i.e. can not contain spaces, special characters etc.)
///
/// When creating heads, it is possible to have multiple heads that point to the same patch/commit.
/// If this is the case, the order can be disambiguated by specifying the `preceding_head`.
/// If there are multiple heads pointing to the same patch and `preceding_head` is not specified,
/// that means the new head will be first in order for that patch.
/// The argument `preceding_head` is only used if there are multiple heads that point to the same patch, otherwise it is ignored.
pub fn create_series(project: &Project, stack_id: StackId, req: CreateSeriesRequest) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx
        .project()
        .snapshot_create_dependent_branch(&req.name, guard.write_permission());
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(stack_id)?;
    let normalized_head_name = normalize_branch_name(&req.name)?;
    // If target_patch is None, create a new head that points to the top of the stack (most recent patch)
    if let Some(target_patch) = req.target_patch {
        stack.add_series(
            ctx,
            StackBranch {
                head: target_patch,
                name: normalized_head_name,
                description: req.description,
                pr_number: Default::default(),
                archived: Default::default(),
            },
            req.preceding_head,
        )
    } else {
        stack.add_series_top_of_stack(ctx, req.name, req.description)
    }
}

/// Request to create a new series in a stack
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateSeriesRequest {
    /// Name of the new series
    name: String,
    /// Description of the new series - can be markdown or anything really
    description: Option<String>,
    /// The target patch (head) to create these series for. If let None, the new series will be at the top of the stack
    target_patch: Option<CommitOrChangeId>,
    /// The name of the series that preceded the newly created series.
    /// This is used to disambiguate the order when they point to the same patch
    preceding_head: Option<String>,
}

/// Removes series grouping from the Stack. This will not touch the patches / commits contained in the series.
/// The very last branch (reference) cannot be removed (A Stack must always contain at least one reference)
/// If there were commits/changes that were *only* referenced by the removed branch,
/// those commits are moved to the branch underneath it (or more accurately, the preceding it)
pub fn remove_series(project: &Project, stack_id: StackId, head_name: String) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx
        .project()
        .snapshot_remove_dependent_branch(&head_name, guard.write_permission());
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(stack_id)?;
    stack.remove_series(ctx, head_name)
}

/// Updates the name an existing series in the stack and resets the pr_number to None.
/// Same invariants as `create_series` apply.
/// If the series have been pushed to a remote, the name can not be changed as it corresponds to a remote ref.
pub fn update_series_name(
    project: &Project,
    stack_id: StackId,
    head_name: String,
    new_head_name: String,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx
        .project()
        .snapshot_update_dependent_branch_name(&head_name, guard.write_permission());
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(stack_id)?;
    let normalized_head_name = normalize_branch_name(&new_head_name)?;
    stack.update_series(
        ctx,
        head_name,
        &PatchReferenceUpdate {
            name: Some(normalized_head_name),
            ..Default::default()
        },
    )
}

/// Updates the description of an existing series in the stack.
/// The description can be set to `None` to remove it.
pub fn update_series_description(
    project: &Project,
    stack_id: StackId,
    head_name: String,
    description: Option<String>,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateDependentBranchDescription),
        guard.write_permission(),
    );
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(stack_id)?;
    stack.update_series(
        ctx,
        head_name,
        &PatchReferenceUpdate {
            description: Some(description),
            ..Default::default()
        },
    )
}

/// Sets the forge identifier for a given series/branch. Existing value is overwritten.
///
/// # Errors
/// This method will return an error if:
///  - The series does not exist
///  - The stack cant be found
///  - The stack has not been initialized
///  - The project is not in workspace mode
///  - Persisting the changes failed
pub fn update_series_pr_number(
    project: &Project,
    stack_id: StackId,
    head_name: String,
    pr_number: Option<usize>,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    let mut guard = project.exclusive_worktree_access();
    let _ = ctx.project().create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateDependentBranchPrNumber),
        guard.write_permission(),
    );
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(stack_id)?;
    stack.set_pr_number(ctx, &head_name, pr_number)
}

/// Pushes all series in the stack to the remote.
/// This operation will error out if the target has no push remote configured.
pub fn push_stack(project: &Project, stack_id: StackId, with_force: bool) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let state = ctx.project().virtual_branches();
    let stack = state.get_branch(stack_id)?;

    let repo = ctx.repository();
    let default_target = state.get_default_target()?;
    let merge_base = repo.find_commit(repo.merge_base(stack.head(), default_target.sha)?)?;
    let merge_base = if let Some(change_id) = merge_base.change_id() {
        CommitOrChangeId::ChangeId(change_id)
    } else {
        CommitOrChangeId::CommitId(merge_base.id().to_string())
    };

    // First fetch, because we dont want to push integrated series
    ctx.fetch(&default_target.push_remote_name(), None)?;
    let gix_repo = ctx.gix_repository_for_merging_non_persisting()?;
    let cache = gix_repo.commit_graph_if_enabled()?;
    let mut graph = gix_repo.revision_graph(cache.as_ref());
    let mut check_commit = IsCommitIntegrated::new(ctx, &default_target, &gix_repo, &mut graph)?;
    let stack_branches = stack.branches();
    for branch in stack_branches {
        if branch.archived {
            // Nothing to push for this one
            continue;
        }
        if branch.head == merge_base {
            // Nothing to push for this one
            continue;
        }
        if branch_integrated(&mut check_commit, &branch, ctx, &stack)? {
            // Already integrated, nothing to push
            continue;
        }
        let push_details = stack.push_details(ctx, branch.name)?;
        ctx.push(
            push_details.head,
            &push_details.remote_refname,
            with_force,
            None,
            Some(Some(stack.id)),
        )?
    }
    Ok(())
}

fn branch_integrated(
    check_commit: &mut IsCommitIntegrated,
    branch: &StackBranch,
    ctx: &CommandContext,
    stack: &Stack,
) -> Result<bool> {
    if branch.archived {
        return Ok(true);
    }
    let branch_head = ctx.repository().find_commit(branch.head_oid(ctx, stack)?)?;
    check_commit.is_integrated(&branch_head)
}

/// Returns the stack series for the API.
/// Newest first, oldest last in the list
/// `commits` is used to accelerate the is-integrated check.
pub(crate) fn stack_series(
    ctx: &CommandContext,
    stack: &mut Stack,
    default_target: &Target,
    check_commit: &mut IsCommitIntegrated,
    remote_commit_data: HashMap<CommitData, git2::Oid>,
    commits: &[VirtualBranchCommit],
) -> Result<(Vec<PatchSeries>, bool)> {
    let mut requires_force = false;
    let mut api_series: Vec<PatchSeries> = vec![];
    for stack_branch in stack.branches() {
        let branch_commits = stack_branch.commits(ctx, stack)?;
        let remote = default_target.push_remote_name();
        let upstream_reference = if stack_branch.pushed(remote.as_str(), ctx)? {
            stack_branch.remote_reference(remote.as_str()).ok()
        } else {
            None
        };
        let mut patches: Vec<VirtualBranchCommit> = vec![];
        let mut is_integrated = false;
        // Reverse first instead of later, so that we catch the first integrated commit
        for commit in branch_commits.clone().local_commits.iter().rev() {
            if !is_integrated {
                is_integrated = commits
                    .iter()
                    .find_map(|c| (c.id == commit.id()).then_some(Ok(c.is_integrated)))
                    .unwrap_or_else(|| check_commit.is_integrated(commit))?;
            }
            let copied_from_remote_id = CommitData::try_from(commit)
                .ok()
                .and_then(|data| remote_commit_data.get(&data).copied());
            let remote_commit_id = commit
                .change_id()
                .and_then(|change_id| {
                    branch_commits.remote_commits.iter().find_map(|c| {
                        (c.change_id().as_deref() == Some(&change_id)).then(|| c.id())
                    })
                })
                .or(copied_from_remote_id)
                .or(if branch_commits.remote(commit) {
                    Some(commit.id())
                } else {
                    None
                });
            if remote_commit_id.map_or(false, |id| commit.id() != id) {
                requires_force = true;
            }
            let vcommit = commit_to_vbranch_commit(
                ctx,
                stack,
                commit,
                is_integrated,
                branch_commits.remote(commit),
                copied_from_remote_id,
                remote_commit_id,
            )?;
            patches.push(vcommit);
        }
        // There should be no duplicates, but dedup because the UI cant handle duplicates
        patches.dedup_by(|a, b| a.id == b.id);

        let mut upstream_patches = vec![];
        for commit in branch_commits.upstream_only_commits {
            let is_integrated = check_commit.is_integrated(&commit)?;
            let vcommit = commit_to_vbranch_commit(
                ctx,
                stack,
                &commit,
                is_integrated,
                true, // per definition
                None, // per definition
                Some(commit.id()),
            )?;
            upstream_patches.push(vcommit);
        }
        upstream_patches.reverse();
        // There should be no duplicates, but dedup because the UI cant handle duplicates
        upstream_patches.dedup_by(|a, b| a.id == b.id);

        if !upstream_patches.is_empty() {
            requires_force = true;
        }
        api_series.push(PatchSeries {
            name: stack_branch.name,
            description: stack_branch.description,
            upstream_reference,
            patches,
            upstream_patches,
            pr_number: stack_branch.pr_number,
            archived: stack_branch.archived,
        });
    }
    api_series.reverse();

    // This is done for compatibility with the legacy flow.
    // After a couple of weeks we can get rid of this.
    if let Err(e) = stack.set_legacy_compatible_stack_reference(ctx) {
        tracing::warn!("failed to set legacy compatible stack reference: {:?}", e);
    }

    Ok((api_series, requires_force))
}
