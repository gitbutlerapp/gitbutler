use std::str::FromStr;

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{RepositoryExt, ref_metadata::StackId};
use but_ctx::{Context, ThreadSafeContext};
use but_rebase::{
    RebaseOutput,
    graph_rebase::{
        Editor, LookupStep as _,
        mutate::{InsertSide, RelativeToRef},
    },
};
use but_workspace::{
    commit_engine,
    legacy::{StacksFilter, ui::StackEntry},
};
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oplog::{
    OplogExt, SnapshotExt,
    entry::{OperationKind, SnapshotDetails},
};
use tracing::instrument;

use crate::json::HexHash;

#[but_api(napi, try_from = but_workspace::ui::RefInfo, provides = [Workspace])]
#[instrument(err(Debug))]
pub fn head_info(ctx: &but_ctx::Context) -> Result<but_workspace::RefInfo> {
    let traversal = ctx.graph_options(but_graph::init::Options::limited())?;
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.meta()?;
    let gerrit_mode_enabled = repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false);
    let db = gerrit_mode_enabled
        .then(|| ctx.db.get_cache())
        .transpose()?;
    let gerrit_mode = match db.as_ref() {
        Some(db) => but_workspace::ref_info::GerritMode::Enabled(db.gerrit_metadata()),
        None => but_workspace::ref_info::GerritMode::Disabled,
    };
    let mut info = but_workspace::head_info(
        &repo,
        &meta,
        but_workspace::ref_info::Options {
            project_meta: ctx.project_meta()?,
            traversal,
            expensive_commit_info: true,
            gerrit_mode,
        },
    )?
    .pruned_to_entrypoint();

    // Resolve each segment's PR association from the forge review cache instead
    // of stored branch metadata, keyed by the segment's remote/pushed short name.
    let forge_db = ctx.db.get_cache()?;
    info.apply_forge_review_associations(
        &repo,
        &crate::workspace_state::forge_prs_by_head(&forge_db)?,
    );

    Ok(info)
}

#[but_api]
#[instrument(err(Debug))]
pub fn stacks(
    ctx: &Context,
    filter: Option<but_workspace::legacy::StacksFilter>,
) -> Result<Vec<StackEntry>> {
    stacks_v3_from_ctx(ctx, filter.unwrap_or_default())
}
///
/// Return stack information for the repository that `ctx` refers to using legacy metadata.
#[expect(deprecated, reason = "calls but_workspace::legacy::stacks_v3")]
pub(crate) fn stacks_v3_from_ctx(
    ctx: &Context,
    filter: StacksFilter,
) -> anyhow::Result<Vec<but_workspace::legacy::ui::StackEntry>> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.meta()?;
    let workspace_ref = match repo.head() {
        Ok(head)
            if head.referent_name().is_some_and(|head_ref| {
                head_ref.as_bstr() == gitbutler_operating_modes::EDIT_BRANCH_REF
            }) =>
        {
            [
                gitbutler_operating_modes::WORKSPACE_BRANCH_REF,
                gitbutler_operating_modes::INTEGRATION_BRANCH_REF,
            ]
            .iter()
            .find_map(|&name| {
                let ref_name: &gix::refs::FullNameRef = name.try_into().ok()?;
                repo.try_find_reference(ref_name).ok().flatten()?;
                Some(ref_name)
            })
        }
        _ => None,
    };
    // Only prefer a workspace-like ref during edit mode. When HEAD points at
    // `gitbutler/edit`, querying stacks from HEAD would produce entries without stack IDs
    // because the edit branch itself is not part of the workspace metadata.
    let traversal = match workspace_ref {
        Some(_) => but_graph::init::Options::limited(),
        None => ctx.graph_options(but_graph::init::Options::limited())?,
    };
    but_workspace::legacy::stacks_v3(
        &repo,
        &meta,
        &ctx.project_meta()?,
        traversal,
        filter,
        workspace_ref,
    )
}

#[cfg(unix)]
#[but_api]
#[instrument(err(Debug))]
pub fn show_graph_svg(ctx: &Context) -> Result<()> {
    let mut options = ctx.graph_options(but_graph::init::Options::limited())?;
    options.collect_tags = true;
    let repo = ctx.open_isolated_repo()?;
    let meta = ctx.meta()?;
    let graph = but_graph::Graph::from_head(&repo, &meta, ctx.project_meta()?, options)?;
    graph.open_as_svg();
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
#[expect(deprecated, reason = "calls but_workspace::legacy::stack_details_v3")]
pub fn stack_details(
    ctx: &Context,
    stack_id: Option<StackId>,
) -> Result<but_workspace::ui::StackDetails> {
    let traversal = ctx.graph_options(but_graph::init::Options::limited())?;
    let mut details = {
        let repo = ctx.clone_repo_for_merging_non_persisting()?;
        let meta = ctx.meta()?;
        but_workspace::legacy::stack_details_v3(
            stack_id,
            &repo,
            &meta,
            &ctx.project_meta()?,
            traversal,
        )
    }?;
    let repo = ctx.repo.get()?;
    let gerrit_mode = repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false);
    let db = ctx.db.get_cache()?;
    if gerrit_mode {
        for branch in details.branch_details.iter_mut() {
            handle_gerrit(branch, &repo, &db)?;
            update_push_status(branch);
        }
    }
    Ok(details)
}

fn update_push_status(branch: &mut but_workspace::ui::BranchDetails) {
    // If there are any commits that are LocalOnly, then the branch push state should be UnpushedCommits
    // However, if there are also any LocalAndRemote commits where the id != remote_commit_id, then it should be UnpushedCommitsRequiringForce

    let has_local_only = branch
        .commits
        .iter()
        .any(|c| matches!(c.state, but_workspace::ui::CommitState::LocalOnly));

    let has_diverged = branch.commits.iter().any(|c| {
        matches!(
            c.state,
            but_workspace::ui::CommitState::LocalAndRemote(remote_id) if c.id != remote_id
        )
    });

    let all_pushed = branch
        .commits
        .iter()
        .all(|c| matches!(c.state, but_workspace::ui::CommitState::LocalAndRemote(remote_id) if c.id == remote_id));

    branch.push_status = if has_diverged {
        but_workspace::ui::PushStatus::UnpushedCommitsRequiringForce
    } else if has_local_only {
        but_workspace::ui::PushStatus::UnpushedCommits
    } else if all_pushed {
        but_workspace::ui::PushStatus::NothingToPush
    } else {
        branch.push_status
    };
}

fn handle_gerrit(
    branch: &mut but_workspace::ui::BranchDetails,
    repo: &gix::Repository,
    db: &but_db::DbHandle,
) -> anyhow::Result<()> {
    let db = db.gerrit_metadata();
    for commit in branch.commits.iter_mut() {
        let change_id = repo
            .find_commit(commit.id)
            .map_err(anyhow::Error::from)
            .and_then(|c| c.change_id().ok_or(anyhow::anyhow!("no change-id")));
        if let Ok(change_id) = change_id
            && let Some(meta) = db.get(&change_id.to_string())?
        {
            commit.gerrit_review_url = Some(meta.review_url.clone());
            if matches!(commit.state, but_workspace::ui::CommitState::Integrated) {
                return Ok(());
            }
            if commit.id.to_string() == meta.commit_id {
                // Pushed, and identical at the remote
                commit.state = but_workspace::ui::CommitState::LocalAndRemote(commit.id);
            } else {
                // Pushed but diverged
                let remote_oid = gix::ObjectId::from_str(&meta.commit_id)?;
                commit.state = but_workspace::ui::CommitState::LocalAndRemote(remote_oid);
            }
        }
    }
    Ok(())
}

#[but_api(napi, provides = [Branches])]
#[instrument(err(Debug))]
pub fn branch_details(
    ctx: &but_ctx::Context,
    branch_name: String,
    remote: Option<String>,
) -> Result<but_workspace::ui::BranchDetails> {
    let mut details = {
        let repo = ctx.clone_repo_for_merging_non_persisting()?;
        let meta = ctx.meta()?;
        let ref_name: gix::refs::FullName = match remote.as_deref() {
            None => {
                format!("refs/heads/{branch_name}")
            }
            Some(remote) => {
                format!("refs/remotes/{remote}/{branch_name}")
            }
        }
        .try_into()
        .map_err(anyhow::Error::from)?;
        let project_meta = ctx.project_meta()?;
        but_workspace::branch_details(&repo, ref_name.as_ref(), &meta, &project_meta)
    }?;
    let repo = ctx.repo.get()?;
    let db = ctx.db.get_cache()?;

    // Derive the PR association from the forge cache rather than reading a stored
    // number off branch metadata: match the branch's remote/pushed short name
    // (what the forge records as a review's `source_branch`) against the cached
    // reviews. `review_id` is no longer used, so it is always cleared.
    details.review_id = None;
    details.pr_number = {
        let pushed_short_name = details
            .remote_tracking_branch
            .as_ref()
            .and_then(|full| gix::refs::FullName::try_from(full.clone()).ok())
            .and_then(|full| {
                but_core::extract_remote_name_and_short_name(full.as_ref(), &repo.remote_names())
                    .map(|(_, short)| short.to_string())
            });
        match pushed_short_name {
            Some(short) => but_forge::review_for_head_ref(&db, &short)?
                .and_then(|review| usize::try_from(review.number).ok()),
            None => None,
        }
    };

    let gerrit_mode = ctx
        .repo
        .get()?
        .git_settings()?
        .gitbutler_gerrit_mode
        .unwrap_or(false);
    if gerrit_mode {
        handle_gerrit(&mut details, &repo, &db)?;
        update_push_status(&mut details);
    }
    Ok(details)
}

/// Discard all worktree changes that match the specs in `worktree_changes`.
///
/// If whole files should be discarded, be sure to not pass any hunks
///
/// Returns the `worktree_changes` that couldn't be applied,
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn discard_worktree_changes(
    ctx: &mut but_ctx::Context,
    worktree_changes: Vec<but_core::DiffSpec>,
) -> Result<Vec<but_core::DiffSpec>> {
    let mut guard = ctx.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );
    let refused = but_workspace::discard_workspace_changes(
        &*ctx.repo.get()?,
        worktree_changes,
        ctx.settings.context_lines,
    )?;
    if !refused.is_empty() {
        tracing::warn!(?refused, "Failed to discard at least one hunk");
    }
    Ok(refused)
}

/// This API allows the user to quickly "stash" a bunch of uncommitted changes - getting them out of the worktree.
/// Unlike the regular stash, the user specifies a new branch where those changes will be 'saved'/committed.
/// Immediately after the changes are committed, the branch is unapplied from the workspace, and the "stash" branch can be re-applied at a later time
/// In theory it should be possible to specify an existing "dumping" branch for this, but currently this endpoint expects a new branch.
#[but_api(commit_engine::ui::CreateCommitOutcome)]
#[instrument(err(Debug))]
pub fn stash_into_branch(
    ctx: &mut Context,
    branch_name: String,
    worktree_changes: Vec<but_core::DiffSpec>,
) -> Result<commit_engine::CreateCommitOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();

    let _ = ctx.snapshot_stash_into_branch(branch_name.clone(), perm);

    let full_ref_name = gix::refs::Category::LocalBranch
        .to_full_name(but_core::branch::normalize_short_name(branch_name.as_str())?.as_bstr())?;
    crate::branch::branch_create_with_perm(
        ctx,
        Some(full_ref_name.clone()),
        crate::branch::json::BranchCreatePlacement::Independent,
        perm,
    )?;
    let stack_id = {
        let (_, ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
        ws.find_segment_and_stack_by_refname(full_ref_name.as_ref())
            .and_then(|(stack, _)| stack.id)
            .context("created stash branch is missing its stack id")?
    };

    ctx.reload_repo_and_invalidate_workspace(perm)?;

    let outcome = {
        let context_lines = ctx.settings.context_lines;
        let mut meta = ctx.meta()?;
        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let but_workspace::commit::CommitCreateOutcome {
            rebase,
            commit_selector,
            rejected_specs,
        } = but_workspace::commit::commit_create(
            editor,
            worktree_changes,
            RelativeToRef::Reference(full_ref_name.as_ref()),
            InsertSide::Below,
            "Mo-Stashed changes",
            context_lines,
            but_workspace::commit::ChangeSource::Head,
        )?;

        let new_commit = commit_selector
            .map(|selector| rebase.lookup_pick(selector))
            .transpose()?;
        let rebase_output = if let Some(new_commit) = new_commit {
            let materialized = rebase.materialize(Default::default())?;
            let commit_mapping: Vec<_> = materialized
                .history
                .commit_mappings()
                .into_iter()
                .map(|(old, new)| (None, old, new))
                .collect();
            (!commit_mapping.is_empty()).then_some(RebaseOutput {
                top_commit: new_commit,
                references: Vec::new(),
                commit_mapping,
            })
        } else {
            None
        };

        Ok(commit_engine::CreateCommitOutcome {
            rejected_specs,
            new_commit,
            changed_tree_pre_cherry_pick: None,
            references: Vec::new(),
            rebase_output,
            index: None,
        })
    };

    ctx.reload_repo_and_invalidate_workspace(perm)?;

    gitbutler_branch_actions::update_workspace_commit_with_perm(ctx, false, perm)
        .context("failed to update gitbutler workspace")?;

    super::virtual_branches::unapply_stack_with_perm(ctx, stack_id, perm)?;

    outcome
}

/// Returns a new available branch name based on a simple template - user_initials-branch-count
/// The main point of this is to be able to provide branch names that are not already taken.
/// This checks local branches and the short-names of remote tracking branches. The reason for
/// the latter is that the but-graph traversal, for now, associates local branches
/// with remote tracking branches by name, not only by configuration, to support older GitButler setups.
///
// TODO(apply): once the new apply is used by default, we can start thinking about phasing this out
//              as it will setup normal Git tracking branch associations via `.git/config`.
#[but_api]
#[instrument(err(Debug))]
pub fn canned_branch_name(ctx: &Context) -> Result<String> {
    let rn = but_core::branch::unique_canned_refname(&*ctx.repo.get()?)?;
    Ok(rn.shorten().to_string())
}

#[but_api]
#[instrument(err(Debug))]
pub fn target_commits(
    ctx: &but_ctx::Context,
    last_commit_id: Option<HexHash>,
    page_size: Option<usize>,
) -> Result<Vec<but_workspace::ui::Commit>> {
    but_workspace::legacy::log_target_first_parent(
        ctx,
        last_commit_id.map(|id| id.into()),
        page_size.unwrap_or(30),
    )
}

/// Push a branch and any parent references that lie within the current workspace projection.
#[but_api(napi, json::PushResult, invalidates = [Workspace, Reviews, MergeStatus, Checks, ReviewTimeline])]
#[instrument(err(Debug))]
pub async fn workspace_branch_and_ancestors_push(
    ctx: ThreadSafeContext,
    with_force: bool,
    skip_force_push_protection: bool,
    branch: String,
    run_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
) -> Result<WorkspaceBranchAndAncestorsPushOutcome> {
    let branch: gix::refs::FullName = branch.try_into()?;
    let sync_ctx = ctx.clone();
    let review_target_updates = {
        let ctx = ctx.clone().into_thread_local();
        crate::legacy::forge::review_target_updates_for_branch(&ctx, branch.as_ref())?
    };
    let review_targets = review_target_updates
        .iter()
        .map(|(_, desired, current)| (desired.clone(), current.clone()))
        .collect::<Vec<_>>();
    let flattened_review_targets =
        crate::legacy::forge::flatten_review_targets_before_push(sync_ctx.clone(), &review_targets)
            .await?;
    let push_branch = branch.clone();
    // This API also awaits forge synchronization, but the Git push remains synchronous and may
    // perform network I/O, hooks, and credential handling. Keep it off Tokio's async worker pool.
    let result = tokio::task::spawn_blocking(move || {
        let mut ctx = ctx.into_thread_local();
        workspace_branch_and_ancestors_push_only(
            &mut ctx,
            with_force,
            skip_force_push_protection,
            push_branch.as_ref(),
            run_hooks,
            push_opts,
        )
    })
    .await
    .context("Push task failed")
    .and_then(|result| result);
    let result = match result {
        Ok(result) => result,
        Err(push_err) => {
            if let Some(flattening) = &flattened_review_targets
                && let Err(restore_err) =
                    crate::legacy::forge::restore_review_targets(sync_ctx, flattening).await
            {
                return Err(push_err.context(format!(
                    "Additionally failed to restore review targets: {restore_err:#}"
                )));
            }
            return Err(push_err);
        }
    };
    let review_sync = if !push_needs_review_sync(&result) {
        if flattened_review_targets.is_some() {
            crate::legacy::forge::sync_review_stack_for_branch(sync_ctx, branch).await
        } else {
            but_forge::ReviewSyncOutcome::NotNeeded
        }
    } else {
        crate::legacy::forge::sync_review_stacks_after_push(
            sync_ctx,
            branch,
            result.branch_sha_updates.clone(),
        )
        .await
    };
    Ok(WorkspaceBranchAndAncestorsPushOutcome {
        push: result,
        review_sync,
    })
}

fn push_needs_review_sync(result: &gitbutler_git::PushResult) -> bool {
    !result.branch_to_remote.is_empty()
}

/// The durable push result plus best-effort review synchronization.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBranchAndAncestorsPushOutcome {
    #[serde(flatten)]
    pub push: gitbutler_git::PushResult,
    pub review_sync: but_forge::ReviewSyncOutcome,
}

/// Perform only the Git push portion of [`workspace_branch_and_ancestors_push()`].
pub fn workspace_branch_and_ancestors_push_only(
    ctx: &mut Context,
    with_force: bool,
    skip_force_push_protection: bool,
    branch: &gix::refs::FullNameRef,
    run_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
) -> Result<gitbutler_git::PushResult> {
    let traversal = ctx.graph_options(but_graph::init::Options::limited())?;
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.meta()?;
    let gerrit_mode_enabled = repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false);
    let mut db = ctx.db.get_cache_mut()?;
    let gerrit_mode = if gerrit_mode_enabled {
        but_workspace::ref_info::GerritMode::Enabled(db.gerrit_metadata())
    } else {
        but_workspace::ref_info::GerritMode::Disabled
    };
    let (head_info, ws) = but_workspace::head_info_and_workspace(
        &repo,
        &meta,
        but_workspace::ref_info::Options {
            project_meta: ctx.project_meta()?,
            traversal,
            expensive_commit_info: true,
            gerrit_mode,
        },
    )?;
    let head_info = head_info.pruned_to_entrypoint();

    let result = but_workspace::legacy::push::workspace_branch_and_ancestors_push(
        &repo,
        &ws,
        &ctx.project_meta()?,
        &head_info,
        &mut db,
        gerrit_mode_enabled,
        with_force,
        skip_force_push_protection,
        ctx.legacy_project.force_push_protection,
        branch,
        run_hooks,
        ctx.legacy_project.husky_hooks_enabled,
        push_opts,
    )?;

    Ok(result)
}

pub mod json {
    use serde::Serialize;

    /// JSON-friendly version of [`gitbutler_git::PushResult`].
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct PushResult {
        /// The name of the remote to which the branches were pushed.
        pub remote: String,
        /// The list of pushed branches and their corresponding remote refnames.
        pub branch_to_remote: Vec<(String, String)>,
        /// The list of branches with their before/after commit SHAs.
        /// Format: (branch_name, before_sha, after_sha)
        pub branch_sha_updates: Vec<(String, String, String)>,
        /// Best-effort synchronization of the complete review stack affected by the push.
        pub review_sync: but_forge::ReviewSyncOutcome,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(PushResult);

    impl From<super::WorkspaceBranchAndAncestorsPushOutcome> for PushResult {
        fn from(value: super::WorkspaceBranchAndAncestorsPushOutcome) -> Self {
            Self {
                remote: value.push.remote,
                branch_to_remote: value
                    .push
                    .branch_to_remote
                    .into_iter()
                    .map(|(name, refname)| (name, refname.to_string()))
                    .collect(),
                branch_sha_updates: value.push.branch_sha_updates,
                review_sync: value.review_sync,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::push_needs_review_sync;

    #[test]
    fn no_op_push_does_not_request_review_synchronization() {
        let no_op = gitbutler_git::PushResult {
            remote: "origin".into(),
            branch_to_remote: Vec::new(),
            branch_sha_updates: Vec::new(),
        };
        assert!(!push_needs_review_sync(&no_op));

        let pushed = gitbutler_git::PushResult {
            remote: "origin".into(),
            branch_to_remote: vec![(
                "feature".into(),
                "refs/remotes/origin/feature"
                    .try_into()
                    .expect("valid remote reference"),
            )],
            branch_sha_updates: Vec::new(),
        };
        assert!(push_needs_review_sync(&pushed));
    }
}
