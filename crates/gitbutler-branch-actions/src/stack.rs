use anyhow::{Context as _, Result};
use but_core::RepositoryExt;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::{
    OplogExt, SnapshotExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_reference::normalize_branch_name;
use gitbutler_repo::hooks;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{PatchReferenceUpdate, StackBranch, StackId};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    VirtualBranchesExt,
    actions::Verify,
    r#virtual::{IsCommitIntegrated, PushResult},
};

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
pub fn create_branch(ctx: &Context, stack_id: StackId, req: CreateSeriesRequest) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.snapshot_create_dependent_branch(&req.name, guard.write_permission());
    ensure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.legacy_project.virtual_branches().get_stack(stack_id)?;
    let normalized_head_name = normalize_branch_name(&req.name)?;
    let repo = ctx.repo.get()?;
    // If target_patch is None, create a new head that points to the top of the stack (most recent patch)
    if let Some(target_patch) = req.target_patch {
        stack.add_series(
            ctx,
            StackBranch::new(target_patch, normalized_head_name, req.description, &repo)?,
            req.preceding_head,
        )
    } else {
        stack.add_series_top_of_stack(ctx, normalized_head_name, req.description)
    }
}

/// Request to create a new series in a stack
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateSeriesRequest {
    /// Name of the new series
    pub name: String,
    /// Description of the new series - can be markdown or anything really
    pub description: Option<String>,
    /// The target patch (head) to create these series for. If let None, the new series will be at the top of the stack
    pub target_patch: Option<gitbutler_stack::CommitOrChangeId>,
    /// The name of the series that preceded the newly created series.
    /// This is used to disambiguate the order when they point to the same patch
    pub preceding_head: Option<String>,
}

/// Removes series grouping from the Stack. This will not touch the patches / commits contained in the series.
/// The very last branch (reference) cannot be removed (A Stack must always contain at least one reference)
/// If there were commits/changes that were *only* referenced by the removed branch,
/// those commits are moved to the branch underneath it (or more accurately, the preceding it)
pub fn remove_branch(ctx: &Context, stack_id: StackId, branch_name: &str) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.snapshot_remove_dependent_branch(branch_name, guard.write_permission());
    ensure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.legacy_project.virtual_branches().get_stack(stack_id)?;
    stack.remove_branch(ctx, branch_name)
}

/// Updates the name an existing branch and resets the pr_number to None.
/// Same invariants as `create_branch` apply.
pub fn update_branch_name(
    ctx: &Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.snapshot_update_dependent_branch_name(&branch_name, guard.write_permission());
    ensure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.legacy_project.virtual_branches().get_stack(stack_id)?;
    let normalized_head_name = normalize_branch_name(&new_name)?;
    stack.update_branch(
        ctx,
        branch_name,
        &PatchReferenceUpdate {
            name: Some(normalized_head_name),
            ..Default::default()
        },
    )
}

/// Updates the description of an existing series in the stack.
/// The description can be set to `None` to remove it.
pub fn update_branch_description(
    ctx: &Context,
    stack_id: StackId,
    branch_name: String,
    description: Option<String>,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateDependentBranchDescription),
        guard.write_permission(),
    );
    ensure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.legacy_project.virtual_branches().get_stack(stack_id)?;
    stack.update_branch(
        ctx,
        branch_name,
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
///  - The stack can't be found
///  - The stack has not been initialized
///  - The project is not in workspace mode
///  - Persisting the changes failed
pub fn update_branch_pr_number(
    ctx: &Context,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateDependentBranchPrNumber),
        guard.write_permission(),
    );
    ensure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.legacy_project.virtual_branches().get_stack(stack_id)?;
    stack.set_pr_number(ctx, &branch_name, pr_number)
}

/// Pushes all series in the stack to the remote.
/// This operation will error out if the target has no push remote configured.
pub fn push_stack(
    ctx: &mut Context,
    stack_id: StackId,
    with_force: bool,
    skip_force_push_protection: bool,
    branch_limit: String,
    run_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
) -> Result<PushResult> {
    ctx.verify(ctx.exclusive_worktree_access().write_permission())?;
    ensure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let state = ctx.legacy_project.virtual_branches();
    let stack = state.get_stack(stack_id)?;

    let git2_repo = ctx.git2_repo.get()?;
    let default_target = state.get_default_target()?;
    let gix_repo = ctx.clone_repo_for_merging_non_persisting()?;
    let merge_base_id = git2_repo
        .find_commit(
            git2_repo.merge_base(stack.head_oid(&gix_repo)?.to_git2(), default_target.sha)?,
        )?
        .id()
        .to_gix();

    // First fetch, because we dont want to push integrated series
    ctx.fetch(
        &default_target.push_remote_name(),
        Some("push_stack".into()),
    )?;
    let cache = gix_repo.commit_graph_if_enabled()?;
    let stack_branches = stack.branches();
    let mut result = PushResult {
        remote: default_target.push_remote_name(),
        branch_to_remote: vec![],
    };
    let gerrit_mode = gix_repo
        .git_settings()?
        .gitbutler_gerrit_mode
        .unwrap_or(false);

    let force_push_protection =
        !skip_force_push_protection && ctx.legacy_project.force_push_protection;

    drop(git2_repo);
    for branch in stack_branches {
        let git2_repo = ctx.git2_repo.get()?;
        if branch.archived {
            // Nothing to push for this one
            tracing::debug!(branch = branch.name, "skipping archived branch for pushing");
            continue;
        }
        if branch.head_oid(&gix_repo)? == merge_base_id {
            // Nothing to push for this one
            tracing::debug!(
                branch = branch.name,
                "nothing to push as head_oid == merge_base"
            );
            continue;
        }
        let mut graph = gix_repo.revision_graph(cache.as_ref());
        let mut check_commit =
            IsCommitIntegrated::new(ctx, &default_target, &gix_repo, &mut graph)?;
        if branch_integrated(&mut check_commit, &branch, &git2_repo, &gix_repo)? {
            // Already integrated, nothing to push
            tracing::debug!(branch = branch.name, "Skipping push for integrated branch");
            continue;
        }
        drop(graph);
        let push_details = stack.push_details(ctx, branch.name().to_owned())?;

        if run_hooks {
            let remote_name = default_target.push_remote_name();
            let remote = git2_repo.find_remote(&remote_name)?;
            let url = &remote
                .url()
                .with_context(|| format!("Remote named {remote_name} didn't have a URL"))?;
            match hooks::pre_push(
                &git2_repo,
                &remote_name,
                url,
                push_details.head,
                &push_details.remote_refname,
            )? {
                hooks::HookResult::Success | hooks::HookResult::NotConfigured => {}
                hooks::HookResult::Failure(error_data) => {
                    return Err(anyhow::anyhow!(
                        "pre-push hook failed: {}",
                        error_data.error
                    ));
                }
            }
        }

        let refspec = if gerrit_mode {
            Some(format!(
                "{}:refs/for/{}",
                push_details.head,
                default_target.branch.branch(),
            ))
        } else {
            None
        };

        let push_opts = if gerrit_mode {
            push_opts.iter().map(|o| o.to_string()).collect()
        } else {
            vec![]
        };

        let out = ctx.push(
            push_details.head,
            &push_details.remote_refname,
            with_force,
            force_push_protection,
            refspec,
            Some(Some(stack.id)),
            push_opts,
        )?;

        drop(git2_repo);
        if gerrit_mode {
            let push_output = but_gerrit::parse::push_output(&out)?;
            let stacks = stack
                .commits(ctx)?
                .iter()
                .map(|id| id.to_gix())
                .collect_vec();
            but_gerrit::record_push_metadata(ctx, &gix_repo, stacks, push_output)?;
        }

        result.branch_to_remote.push((
            branch.name().to_owned(),
            push_details.remote_refname.to_owned().into(),
        ));

        if branch.name().eq(&branch_limit) {
            break;
        }
    }

    Ok(result)
}

pub(crate) fn branch_integrated(
    check_commit: &mut IsCommitIntegrated,
    branch: &StackBranch,
    repo: &git2::Repository,
    gix_repo: &gix::Repository,
) -> Result<bool> {
    if branch.archived {
        return Ok(true);
    }
    let oid = branch.head_oid(gix_repo)?;
    let branch_head = repo.find_commit(oid.to_git2())?;
    check_commit.is_integrated(&branch_head)
}
