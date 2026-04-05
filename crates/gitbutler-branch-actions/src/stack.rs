use anyhow::{Context as _, Result};
use but_core::RepositoryExt;
use but_ctx::Context;
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::{
    OplogExt, SnapshotExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_reference::normalize_branch_name;
use gitbutler_repo::hooks;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{PatchReferenceUpdate, StackBranch, StackId};
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
pub fn create_branch(ctx: &mut Context, stack_id: StackId, req: CreateSeriesRequest) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.snapshot_create_dependent_branch(&req.name, guard.write_permission());
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Requires an open workspace mode")?;
    let mut stack = ctx.virtual_branches().get_stack(stack_id)?;
    let normalized_head_name = normalize_branch_name(&req.name)?;
    let repo = ctx.repo.get()?;
    // If target_patch is None, create a new head that points to the top of the stack (most recent patch)
    if let Some(target_patch) = req.target_patch {
        let target_oid = gix::ObjectId::from_hex(target_patch.as_bytes())?;
        stack.add_series(
            ctx,
            StackBranch::new(target_oid, normalized_head_name, &repo)?,
            req.preceding_head,
        )
    } else {
        stack.add_series_top_of_stack(ctx, normalized_head_name)
    }
}

/// Request to create a new series in a stack
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateSeriesRequest {
    /// Name of the new series
    pub name: String,
    /// The target patch (head) to create these series for. If let None, the new series will be at the top of the stack
    pub target_patch: Option<String>,
    /// The name of the series that preceded the newly created series.
    /// This is used to disambiguate the order when they point to the same patch
    pub preceding_head: Option<String>,
}

/// Removes series grouping from the Stack. This will not touch the patches / commits contained in the series.
/// The very last branch (reference) cannot be removed (A Stack must always contain at least one reference)
/// If there were commits/changes that were *only* referenced by the removed branch,
/// those commits are moved to the branch underneath it (or more accurately, the preceding it)
pub fn remove_branch(ctx: &mut Context, stack_id: StackId, branch_name: &str) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    let _ = ctx.snapshot_remove_dependent_branch(branch_name, guard.write_permission());
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Requires an open workspace mode")?;
    let mut stack = ctx.virtual_branches().get_stack(stack_id)?;
    stack.remove_branch(ctx, branch_name)
}

/// Updates the name an existing branch and resets the pr_number to None.
/// Same invariants as `create_branch` apply.
///
/// Returns the new normalized name of the branch.
pub fn update_branch_name(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<String> {
    let mut guard = ctx.exclusive_worktree_access();
    update_branch_name_with_perm(
        ctx,
        stack_id,
        branch_name,
        new_name,
        guard.write_permission(),
    )
}

pub fn update_branch_name_with_perm(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
    perm: &mut but_core::sync::RepoExclusive,
) -> Result<String> {
    ctx.verify(perm)?;
    let _ = ctx.snapshot_update_dependent_branch_name(&branch_name, perm);
    ensure_open_workspace_mode(ctx, perm.read_permission())
        .context("Requires an open workspace mode")?;
    let mut stack = ctx.virtual_branches().get_stack(stack_id)?;
    let normalized_head_name = normalize_branch_name(&new_name)?;
    stack.update_branch(
        ctx,
        branch_name,
        &PatchReferenceUpdate {
            name: Some(normalized_head_name.clone()),
        },
    )?;
    Ok(normalized_head_name)
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
    ctx: &mut Context,
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
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Requires an open workspace mode")?;
    let mut stack = ctx.virtual_branches().get_stack(stack_id)?;
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
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Requires an open workspace mode")?;
    let state = ctx.virtual_branches();
    let stack = state.get_stack(stack_id)?;

    let default_target = state.get_default_target()?;
    let gix_repo = ctx.clone_repo_for_merging_non_persisting()?;
    let merge_base_id = gix_repo
        .merge_base(stack.head_oid(ctx)?, default_target.sha)?
        .detach();

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
        branch_sha_updates: vec![],
    };
    let gerrit_mode = gix_repo
        .git_settings()?
        .gitbutler_gerrit_mode
        .unwrap_or(false);

    let force_push_protection =
        !skip_force_push_protection && ctx.legacy_project.force_push_protection;

    for branch in stack_branches {
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
        let mut check_commit = IsCommitIntegrated::new(&default_target, &gix_repo, &mut graph)?;
        if branch_integrated(&mut check_commit, &branch, &gix_repo)? {
            // Already integrated, nothing to push
            tracing::debug!(branch = branch.name, "Skipping push for integrated branch");
            continue;
        }
        drop(graph);
        let push_details = stack.push_details(ctx, branch.name().to_owned())?;

        // Capture the SHA before push (remote ref if exists, otherwise zero)
        let before_sha = gix_repo
            .try_find_reference(&push_details.remote_refname.to_string())?
            .map(|mut reference| reference.peel_to_commit())
            .transpose()?
            .map(|commit| commit.id)
            .unwrap_or(gix_repo.object_hash().null());
        let local_sha = push_details.head;

        if run_hooks {
            let remote_name = default_target.push_remote_name();
            let remote = gix_repo.find_remote(remote_name.as_str())?;
            let url = remote
                .url(gix::remote::Direction::Push)
                .or_else(|| remote.url(gix::remote::Direction::Fetch))
                .map(|url| url.to_bstring().to_string())
                .with_context(|| format!("Remote named {remote_name} didn't have a URL"))?;
            match hooks::pre_push(
                &gix_repo,
                &remote_name,
                &url,
                push_details.head,
                &push_details.remote_refname,
                ctx.legacy_project.husky_hooks_enabled,
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

        if gerrit_mode {
            let push_output = but_gerrit::parse::push_output(&out)?;
            let stacks = stack.commits(ctx)?;
            but_gerrit::record_push_metadata(ctx, stacks, push_output)?;
        }

        result.branch_to_remote.push((
            branch.name().to_owned(),
            push_details.remote_refname.to_owned().into(),
        ));

        // Record the SHA update (before -> after)
        result.branch_sha_updates.push((
            branch.name().to_owned(),
            before_sha.to_string(),
            local_sha.to_string(),
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
    gix_repo: &gix::Repository,
) -> Result<bool> {
    if branch.archived {
        return Ok(true);
    }
    let oid = branch.head_oid(gix_repo)?;
    check_commit.is_integrated(oid)
}
