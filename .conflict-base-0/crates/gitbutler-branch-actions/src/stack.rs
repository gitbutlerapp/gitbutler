use anyhow::{Context as _, Result};
use but_core::{RepositoryExt, extract_remote_name_and_short_name, ref_metadata::StackId};
use but_ctx::{Context, access::RepoShared};
use gitbutler_git::{GitContextExt as _, PushResult};
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::{
    OplogExt, SnapshotExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_reference::{RemoteRefname, normalize_branch_name};
use gitbutler_repo::hooks;
use gitbutler_stack::{PatchReferenceUpdate, Stack, StackBranch};
use gix::reference::Category;
use serde::{Deserialize, Serialize};

use crate::{VirtualBranchesExt, actions::Verify, r#virtual::IsCommitIntegrated};

/// Return the legacy stack identified by `stack_id`.
///
/// This keeps legacy virtual-branches access encapsulated within
/// `gitbutler-branch-actions` for callers that still operate on
/// `gitbutler_stack::Stack`.
pub fn get_stack(ctx: &Context, stack_id: StackId) -> Result<Stack> {
    ctx.virtual_branches().get_stack(stack_id)
}

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
    // Pure metadata write — skip verify so background syncs aren't
    // blocked when HEAD is off the workspace ref (e.g. edit mode).
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
    push_stack_with_perm(
        ctx,
        stack_id,
        with_force,
        skip_force_push_protection,
        branch_limit,
        run_hooks,
        push_opts,
        guard.read_permission(),
    )
}

/// Pushes all series in the stack to the remote using an existing shared repository permission.
/// This operation will error out if the target has no push remote configured.
#[expect(clippy::too_many_arguments)]
pub fn push_stack_with_perm(
    ctx: &mut Context,
    stack_id: StackId,
    with_force: bool,
    skip_force_push_protection: bool,
    branch_limit: String,
    run_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
    perm: &RepoShared,
) -> Result<PushResult> {
    ensure_open_workspace_mode(ctx, perm).context("Requires an open workspace mode")?;
    let virtual_branches = ctx.virtual_branches();
    let stack = virtual_branches.get_stack(stack_id)?;
    let (target_ref_name, target_base_oid, target_push_remote_name, target_branch_name) = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
        let target_ref_name = ws
            .target_ref_name()
            .context("failed to get target reference name")?
            .to_owned();
        let remote_names = repo.remote_names();
        let target_branch_name =
            target_branch_name_from_ref_name(target_ref_name.as_ref(), &remote_names)?;
        let target_push_remote_name = match ctx.project_meta()?.push_remote {
            Some(push_remote) => push_remote,
            None => extract_remote_name_and_short_name(target_ref_name.as_ref(), &remote_names)
                .map(|(remote, _)| remote)
                .context("failed to get target push remote name")?,
        };
        (
            target_ref_name,
            ws.stored_target_commit_id()
                .context("failed to get target base oid")?,
            target_push_remote_name,
            target_branch_name.to_string(),
        )
    };
    let push_env = PushStackEnv::new(
        ctx,
        &stack,
        target_ref_name,
        target_base_oid,
        target_push_remote_name,
        target_branch_name,
        skip_force_push_protection,
    )?;

    // First fetch, because we dont want to push integrated series
    ctx.fetch(&push_env.remote_name, Some("push_stack".into()))?;
    let mut result = PushResult {
        remote: push_env.remote_name.clone(),
        branch_to_remote: vec![],
        branch_sha_updates: vec![],
    };
    let stop_after_branch = branch_limit;

    for branch in stack.branches() {
        let Some(prepared_branch) = prepare_branch_push(&branch, &push_env)? else {
            continue;
        };

        let should_stop = prepared_branch.branch_name == stop_after_branch;
        let pushed_branch = execute_branch_push(
            ctx,
            &stack,
            prepared_branch,
            &push_env,
            with_force,
            run_hooks,
            &push_opts,
        )?;
        append_push_result(&mut result, pushed_branch);
        if should_stop {
            break;
        }
    }

    Ok(result)
}

struct PushStackEnv {
    target_ref_name: gix::refs::FullName,
    target_branch_name: String,
    gix_repo: gix::Repository,
    commit_graph_cache: Option<gix::commitgraph::Graph>,
    merge_base_id: gix::ObjectId,
    target_base_oid: gix::ObjectId,
    remote_name: String,
    gerrit_mode: bool,
    force_push_protection: bool,
    run_husky_hooks: bool,
}

impl PushStackEnv {
    fn new(
        ctx: &Context,
        stack: &Stack,
        target_ref_name: gix::refs::FullName,
        target_base_oid: gix::ObjectId,
        remote_name: String,
        target_branch_name: String,
        skip_force_push_protection: bool,
    ) -> Result<Self> {
        let gix_repo = ctx.clone_repo_for_merging_non_persisting()?;
        let merge_base_id = gix_repo
            .merge_base(stack.head_oid(ctx)?, target_base_oid)?
            .detach();
        let commit_graph_cache = gix_repo.commit_graph_if_enabled()?;
        let gerrit_mode = gix_repo
            .git_settings()?
            .gitbutler_gerrit_mode
            .unwrap_or(false);

        Ok(Self {
            target_ref_name,
            target_branch_name,
            gix_repo,
            commit_graph_cache,
            merge_base_id,
            target_base_oid,
            remote_name,
            gerrit_mode,
            force_push_protection: !skip_force_push_protection
                && ctx.legacy_project.force_push_protection,
            run_husky_hooks: ctx.legacy_project.husky_hooks_enabled,
        })
    }
}

struct GerritPushArgs {
    refspec: Option<String>,
    push_opts: Vec<String>,
}

struct PreparedBranchPush {
    branch_name: String,
    remote_refname: RemoteRefname,
    local_sha: gix::ObjectId,
    before_sha: gix::ObjectId,
}

struct PushedBranch {
    branch_name: String,
    remote_refname: gix::refs::FullName,
    before_sha: gix::ObjectId,
    after_sha: gix::ObjectId,
}

enum SkipBranchReason {
    Archived,
    HeadAtMergeBase,
    Integrated,
}

fn prepare_branch_push(
    branch: &StackBranch,
    push_env: &PushStackEnv,
) -> Result<Option<PreparedBranchPush>> {
    if branch.archived {
        log_skipped_branch(branch, SkipBranchReason::Archived);
        return Ok(None);
    }

    let local_sha = branch.head_oid(&push_env.gix_repo)?;
    if let Some(skip_reason) = skip_reason_for_branch(local_sha, push_env)? {
        log_skipped_branch(branch, skip_reason);
        return Ok(None);
    }

    let remote_refname = remote_refname_for_branch(branch, &push_env.remote_name)?;
    let before_sha = remote_before_sha(&push_env.gix_repo, &remote_refname)?;

    Ok(Some(PreparedBranchPush {
        branch_name: branch.name().to_owned(),
        remote_refname,
        local_sha,
        before_sha,
    }))
}

fn execute_branch_push(
    ctx: &mut Context,
    stack: &Stack,
    prepared_branch: PreparedBranchPush,
    push_env: &PushStackEnv,
    with_force: bool,
    run_hooks: bool,
    push_flags: &[but_gerrit::PushFlag],
) -> Result<PushedBranch> {
    let PreparedBranchPush {
        branch_name,
        remote_refname,
        local_sha,
        before_sha,
    } = prepared_branch;

    if run_hooks {
        run_pre_push_hook(push_env, local_sha, &remote_refname)?;
    }

    let gerrit_push_args = gerrit_push_args(push_env, local_sha, push_flags);
    let push_output = ctx.push(
        local_sha,
        &remote_refname,
        with_force,
        push_env.force_push_protection,
        gerrit_push_args.refspec,
        Some(Some(stack.id)),
        gerrit_push_args.push_opts,
    )?;

    maybe_record_gerrit_push_metadata(ctx, stack, push_env, &push_output)?;

    Ok(PushedBranch {
        branch_name,
        remote_refname: (&remote_refname).try_into()?,
        before_sha,
        after_sha: local_sha,
    })
}

fn skip_reason_for_branch(
    local_sha: gix::ObjectId,
    push_env: &PushStackEnv,
) -> Result<Option<SkipBranchReason>> {
    if local_sha == push_env.merge_base_id {
        return Ok(Some(SkipBranchReason::HeadAtMergeBase));
    }

    let mut graph = push_env
        .gix_repo
        .revision_graph(push_env.commit_graph_cache.as_ref());
    let mut check_commit = IsCommitIntegrated::new_with_target(
        push_env.target_ref_name.as_ref(),
        push_env.target_base_oid,
        &push_env.gix_repo,
        &mut graph,
    )?;
    if check_commit.is_integrated(local_sha)? {
        return Ok(Some(SkipBranchReason::Integrated));
    }

    Ok(None)
}

fn log_skipped_branch(branch: &StackBranch, skip_reason: SkipBranchReason) {
    match skip_reason {
        SkipBranchReason::Archived => {
            tracing::info!(
                branch = branch.name(),
                "skipping archived branch for pushing"
            );
        }
        SkipBranchReason::HeadAtMergeBase => {
            tracing::info!(
                branch = branch.name(),
                "nothing to push as head_oid == merge_base"
            );
        }
        SkipBranchReason::Integrated => {
            tracing::info!(
                branch = branch.name(),
                "Skipping push for integrated branch"
            );
        }
    }
}

fn remote_before_sha(
    gix_repo: &gix::Repository,
    remote_refname: &RemoteRefname,
) -> Result<gix::ObjectId> {
    Ok(gix_repo
        .try_find_reference(&remote_refname.to_string())?
        .map(|mut reference| reference.peel_to_commit())
        .transpose()?
        .map(|commit| commit.id)
        .unwrap_or(gix_repo.object_hash().null()))
}

fn remote_refname_for_branch(branch: &StackBranch, remote_name: &str) -> Result<RemoteRefname> {
    branch
        .remote_reference(remote_name)
        .parse()
        .map_err(Into::into)
}

fn run_pre_push_hook(
    push_env: &PushStackEnv,
    local_sha: gix::ObjectId,
    remote_refname: &RemoteRefname,
) -> Result<()> {
    let remote = push_env
        .gix_repo
        .find_remote(push_env.remote_name.as_str())?;
    let url = remote
        .url(gix::remote::Direction::Push)
        .or_else(|| remote.url(gix::remote::Direction::Fetch))
        .map(|url| url.to_bstring().to_string())
        .with_context(|| format!("Remote named {} didn't have a URL", push_env.remote_name))?;

    match hooks::pre_push(
        &push_env.gix_repo,
        &push_env.remote_name,
        &url,
        local_sha,
        remote_refname,
        push_env.run_husky_hooks,
    )? {
        hooks::HookResult::Success | hooks::HookResult::NotConfigured => Ok(()),
        hooks::HookResult::Failure(error_data) => Err(anyhow::anyhow!(
            "pre-push hook failed: {}",
            error_data.error
        )),
    }
}

/// Derive the target branch name while tolerating stale target remotes. Pushes
/// can use a configured push remote even when the integration target is a
/// preserved remote-tracking ref like `refs/remotes/origin/main`. When the
/// target remote is configured, keep using it so remote names containing slashes
/// remain unambiguous.
fn target_branch_name_from_ref_name(
    target_ref_name: &gix::refs::FullNameRef,
    remote_names: &gix::remote::Names<'_>,
) -> Result<String> {
    let (category, shorthand_name) = target_ref_name
        .category_and_short_name()
        .context("Target branch could not be categorized")?;
    if matches!(category, Category::RemoteBranch) {
        if let Some((_remote, short_name)) =
            extract_remote_name_and_short_name(target_ref_name, remote_names)
        {
            return Ok(short_name.to_string());
        }
        let remote_ref: RemoteRefname = target_ref_name.to_string().parse()?;
        return Ok(remote_ref.branch().to_owned());
    }
    Ok(shorthand_name.to_string())
}

fn gerrit_push_args(
    push_env: &PushStackEnv,
    head: gix::ObjectId,
    push_flags: &[but_gerrit::PushFlag],
) -> GerritPushArgs {
    if push_env.gerrit_mode {
        GerritPushArgs {
            refspec: Some(format!("{head}:refs/for/{}", push_env.target_branch_name,)),
            push_opts: push_flags.iter().map(|flag| flag.to_string()).collect(),
        }
    } else {
        GerritPushArgs {
            refspec: None,
            push_opts: vec![],
        }
    }
}

fn maybe_record_gerrit_push_metadata(
    ctx: &Context,
    stack: &Stack,
    push_env: &PushStackEnv,
    push_output: &str,
) -> Result<()> {
    if !push_env.gerrit_mode {
        return Ok(());
    }

    let push_output = but_gerrit::parse::push_output(push_output)?;
    let candidate_ids = stack.commits(ctx)?;
    but_gerrit::record_push_metadata_with_context(ctx, candidate_ids, push_output)
}

fn append_push_result(result: &mut PushResult, pushed_branch: PushedBranch) {
    result.branch_to_remote.push((
        pushed_branch.branch_name.clone(),
        pushed_branch.remote_refname,
    ));
    result.branch_sha_updates.push((
        pushed_branch.branch_name,
        pushed_branch.before_sha.to_string(),
        pushed_branch.after_sha.to_string(),
    ));
}

#[cfg(test)]
mod tests {
    use super::target_branch_name_from_ref_name;
    use std::borrow::Cow;

    use bstr::ByteSlice;

    fn remote_names(names: &[&str]) -> gix::remote::Names<'static> {
        names
            .iter()
            .map(|name| Cow::Owned(name.as_bytes().as_bstr().to_owned()))
            .collect()
    }

    #[test]
    fn target_branch_name_from_remote_ref_without_configured_remote() -> anyhow::Result<()> {
        let remote_names = remote_names(&[]);
        let ref_name: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
        assert_eq!(
            target_branch_name_from_ref_name(ref_name.as_ref(), &remote_names)?,
            "main"
        );
        Ok(())
    }

    #[test]
    fn target_branch_name_preserves_configured_nested_remote() -> anyhow::Result<()> {
        let remote_names = remote_names(&["nested/remote"]);
        let ref_name: gix::refs::FullName = "refs/remotes/nested/remote/feature/a".try_into()?;
        assert_eq!(
            target_branch_name_from_ref_name(ref_name.as_ref(), &remote_names)?,
            "feature/a"
        );
        Ok(())
    }
}
