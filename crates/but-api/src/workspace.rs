//! Functions that operate on the workspace.

use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::WorkspaceState;
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{
    DryRun, RefMetadata, extract_remote_name_and_short_name, is_workspace_ref_name,
    sync::{RepoExclusive, RepoShared},
};
use but_error::AnyhowContextExt as _;
use but_forge::ForgeReview;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_serde::BStringForFrontend;
use but_workspace::{
    BottomUpdate, BottomUpdateKind, IntegrateUpstreamOutcome, ReviewIntegrationHint,
};
use tracing::{instrument, warn};

/// The persisted status of fetches performed through [`workspace_fetch_from_remotes()`].
#[derive(Debug, Clone, Default, serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFetchStatus {
    /// When the most recent fetch attempt finished, in milliseconds since the Unix epoch.
    pub last_attempted_ms: Option<u64>,
    /// When the most recent successful fetch finished, in milliseconds since the Unix epoch.
    pub last_successful_ms: Option<u64>,
    /// The error produced by the most recent attempt, or `None` if it succeeded.
    pub last_error: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WorkspaceFetchStatus);

impl TryFrom<but_db::FetchStatus> for WorkspaceFetchStatus {
    type Error = std::num::TryFromIntError;

    fn try_from(value: but_db::FetchStatus) -> Result<Self, Self::Error> {
        Ok(Self {
            last_attempted_ms: Some(value.last_attempted_ms.try_into()?),
            last_successful_ms: value
                .last_successful_ms
                .map(TryInto::try_into)
                .transpose()?,
            last_error: value.last_error,
        })
    }
}

/// Fetch all configured remotes and persist the outcome for [`workspace_fetch_status()`].
///
/// Fetching continues after an individual remote fails so every configured remote gets an attempt.
/// If any fetch fails, all errors are persisted and returned together. Credential prompts are
/// associated with `action`, which defaults to `"unknown"`.
///
/// The network fetch runs without any repository lock so other operations stay responsive while
/// remotes are contacted; exclusive access is only acquired afterwards for the bookkeeping that
/// reacts to updated remote refs. Within this process, overlapping fetch calls for the same
/// repository serialize among themselves so concurrent `git fetch` runs cannot trip over Git's
/// per-ref locks; fetches from other processes are not affected.
#[but_api(napi)]
#[instrument(skip_all, err(Debug))]
pub fn workspace_fetch_from_remotes(
    ctx: &mut but_ctx::Context,
    action: Option<String>,
) -> anyhow::Result<()> {
    let askpass_action = Some(action.unwrap_or_else(|| "unknown".to_owned()));
    let fetch_result = (|| {
        let repo_path = ctx.workdir_or_gitdir()?;
        let remotes = {
            let repo = ctx.repo.get()?;
            repo.remote_names()
                .iter()
                .map(|name| name.to_str().map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?
        };
        // Overlapping `git fetch` runs of the same repository can fail on Git's per-ref locks,
        // so fetches wait for each other while other operations stay unblocked.
        let repo_fetch_lock = fetch_lock(&ctx.gitdir);
        let _fetch_guard = repo_fetch_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let failures = remotes
            .iter()
            .filter_map(|remote| {
                gitbutler_git::fetch_with_askpass(&repo_path, remote, askpass_action.clone())
                    .err()
                    .map(|err| (remote, err))
            })
            .collect::<Vec<_>>();

        // Keep the failure's own error context (e.g. the `ProjectGitAuth` code and its
        // user-facing message) intact whenever possible: return a single failure as-is, and
        // reapply the first failure's code when several failures collapse into one message.
        match failures.len() {
            0 => Ok(()),
            1 => {
                let (remote, err) = failures.into_iter().next().expect("length checked above");
                Err(err.context(format!("fetching remote `{remote}` failed")))
            }
            _ => {
                let code = failures
                    .iter()
                    .find_map(|(_, err)| err.custom_context().map(|ctx| ctx.code));
                let joined = failures
                    .iter()
                    .map(|(remote, err)| format!("{remote}: {err}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let err = anyhow::anyhow!(joined);
                Err(match code {
                    Some(code) => err.context(code),
                    None => err,
                })
            }
        }
    })();

    let attempted_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| anyhow::anyhow!("system clock is before the Unix epoch: {err}"))?
        .as_millis()
        .try_into()
        .map_err(|err| anyhow::anyhow!("fetch timestamp does not fit in the database: {err}"))?;
    let _guard = ctx.exclusive_worktree_access();
    match &fetch_result {
        Ok(()) => ctx
            .db
            .get_cache_mut()?
            .fetch_status_mut()
            .record_success(attempted_ms)?,
        Err(err) => ctx
            .db
            .get_cache_mut()?
            .fetch_status_mut()
            .record_failure(attempted_ms, &format!("{err:#}"))?,
    }

    // A partial failure may still have updated some remote refs.
    ctx.invalidate_workspace_cache()?;
    prune_missing_branch_stack_order(ctx)?;
    fetch_result
}

/// Return the in-process lock that serializes network fetches for the repository at `gitdir`,
/// so overlapping fetches wait for each other without blocking other repository operations.
fn fetch_lock(gitdir: &std::path::Path) -> std::sync::Arc<std::sync::Mutex<()>> {
    static FETCH_LOCKS: std::sync::Mutex<
        std::collections::BTreeMap<std::path::PathBuf, std::sync::Arc<std::sync::Mutex<()>>>,
    > = std::sync::Mutex::new(std::collections::BTreeMap::new());
    let mut map = FETCH_LOCKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::sync::Arc::clone(map.entry(gitdir.to_owned()).or_default())
}

/// Reconcile branch-order metadata against the current local branch refs.
pub(crate) fn prune_missing_branch_stack_order(ctx: &but_ctx::Context) -> anyhow::Result<()> {
    let local_branch_refs = {
        let repo = ctx.repo.get()?;
        repo.references()?
            .prefixed("refs/heads/")?
            .filter_map(Result::ok)
            .map(|reference| reference.name().to_owned())
            .collect::<Vec<_>>()
    };
    ctx.meta()?
        .remove_missing_branch_stack_order_references(&local_branch_refs)?;
    Ok(())
}

/// Return the persisted status of fetches performed through
/// [`workspace_fetch_from_remotes()`].
///
/// A project that hasn't used the workspace fetch API returns an empty status. Legacy fetch state
/// is intentionally not imported.
#[but_api(napi)]
#[instrument(skip_all, err(Debug))]
pub fn workspace_fetch_status(ctx: &but_ctx::Context) -> anyhow::Result<WorkspaceFetchStatus> {
    ctx.db
        .get_cache()?
        .fetch_status()
        .get()?
        .map(TryInto::try_into)
        .transpose()
        .map(Option::unwrap_or_default)
        .map_err(Into::into)
}

/// Return the current detailed graph workspace for the frontend.
///
/// This is a read-only projection of the current workspace graph. It does not
/// mutate the cached [`WorkspaceState`] returned by mutation APIs.
#[but_api(napi)]
#[instrument(skip_all, err(Debug))]
pub fn get_workspace(
    ctx: &but_ctx::Context,
    perm: &RepoShared,
) -> anyhow::Result<but_workspace::ui::workspace::DetailedGraphWorkspace> {
    let mut meta = ctx.meta()?;
    let (repo, workspace, _) = ctx.workspace_and_db_with_perm(perm)?;
    let mut workspace = workspace.clone();
    but_workspace::workspace::detailed_graph_workspace(&mut workspace, &mut meta, &repo)
        .map(Into::into)
}

/// Make `target_ref` the project's default target without applying branches or entering
/// managed workspace mode.
///
/// This acquires exclusive worktree access from `ctx` before updating project metadata.
/// See [`but_workspace::init::set_target_ref_and_init_project()`] for details; notably the
/// target commit id is only computed when it wasn't set before, and an omitted
/// `push_remote` keeps the currently configured one. It deliberately records no oplog
/// snapshot - only project metadata changes, no repository state.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn set_target_ref_and_init_project(
    ctx: &mut but_ctx::Context,
    target_ref: &gix::refs::FullNameRef,
    push_remote: Option<String>,
) -> anyhow::Result<()> {
    let guard = ctx.exclusive_worktree_access();
    {
        let repo = ctx.repo.get()?;
        but_workspace::init::set_target_ref_and_init_project(&repo, target_ref, push_remote)?;
    }
    ctx.invalidate_workspace_cache()?;
    #[cfg(feature = "legacy")]
    {
        let mut guard = guard;
        crate::legacy::meta::reconcile_in_workspace_state_of_vb_toml(ctx, guard.write_permission())
            .ok();
    }
    #[cfg(not(feature = "legacy"))]
    drop(guard);
    Ok(())
}

/// Set the remote used to publish branches without changing the default target.
///
/// This acquires exclusive repository access, updates project metadata through
/// [`but_workspace::init::set_push_remote()`], and invalidates the cached workspace projection.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn set_push_remote(ctx: &mut but_ctx::Context, push_remote: String) -> anyhow::Result<()> {
    let guard = ctx.exclusive_worktree_access();
    {
        let repo = ctx.repo.get()?;
        but_workspace::init::set_push_remote(&repo, push_remote)?;
    }
    ctx.invalidate_workspace_cache()?;
    #[cfg(feature = "legacy")]
    {
        let mut guard = guard;
        crate::legacy::meta::reconcile_in_workspace_state_of_vb_toml(ctx, guard.write_permission())
            .ok();
    }
    #[cfg(not(feature = "legacy"))]
    drop(guard);
    Ok(())
}

/// Result of integrating upstream changes into the current workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceIntegrateUpstreamOutcome {
    /// The post-operation or preview workspace state.
    pub workspace_state: WorkspaceState,
    /// Dirty worktree paths that would conflict when applied onto the resulting workspace head.
    pub worktree_conflicts: Vec<BStringForFrontend>,
}

/// JSON transport types for workspace APIs.
pub mod json {
    use but_serde::BStringForFrontend;
    use serde::{Deserialize, Serialize};

    /// JSON transport type for how a stack bottom should be updated.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub enum BottomUpdateKind {
        /// Rebase the selected bottom-most commit onto the target branch.
        Rebase,
        /// Create a merge commit at the top of the selected stack.
        Merge,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BottomUpdateKind);

    impl From<BottomUpdateKind> for but_workspace::BottomUpdateKind {
        fn from(value: BottomUpdateKind) -> Self {
            match value {
                BottomUpdateKind::Rebase => Self::Rebase,
                BottomUpdateKind::Merge => Self::Merge,
            }
        }
    }

    /// JSON transport type describing one stack bottom to update.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct BottomUpdate {
        /// How the selected stack bottom should be updated.
        pub kind: BottomUpdateKind,
        /// The bottom-most commit or empty bottom reference to update.
        pub selector: crate::commit::json::RelativeTo,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BottomUpdate);

    impl From<BottomUpdate> for but_workspace::BottomUpdate {
        fn from(value: BottomUpdate) -> Self {
            let BottomUpdate { kind, selector } = value;
            Self {
                kind: kind.into(),
                selector: selector.into(),
            }
        }
    }

    /// JSON transport type returned by upstream integration.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct WorkspaceIntegrateUpstreamOutcome {
        /// The post-operation or preview workspace state.
        pub workspace_state: crate::json::WorkspaceState,
        /// Dirty worktree paths that would conflict when applied onto the resulting workspace head.
        #[cfg_attr(feature = "export-schema", schemars(with = "Vec<String>"))]
        pub worktree_conflicts: Vec<BStringForFrontend>,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(WorkspaceIntegrateUpstreamOutcome);

    impl TryFrom<super::WorkspaceIntegrateUpstreamOutcome> for WorkspaceIntegrateUpstreamOutcome {
        type Error = anyhow::Error;

        fn try_from(value: super::WorkspaceIntegrateUpstreamOutcome) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace_state: value.workspace_state.try_into()?,
                worktree_conflicts: value.worktree_conflicts,
            })
        }
    }
}

/// Build one rebase update for the bottom of every visible workspace stack.
pub fn rebase_stack_bottoms(head_info: &but_workspace::RefInfo) -> Vec<BottomUpdate> {
    head_info
        .stacks
        .iter()
        .filter_map(|stack| {
            let segment = stack.segments.last()?;
            let selector = match segment.commits.last() {
                Some(commit) => RelativeTo::Commit(commit.id),
                None => RelativeTo::Reference(segment.ref_info.as_ref()?.ref_name.clone()),
            };
            Some(BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector,
            })
        })
        .collect()
}

pub(crate) fn target_branch_name(
    symbolic_remote_names: &[String],
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> Option<String> {
    let target_ref = project_meta.target_ref.as_ref()?;
    let mut symbolic_remote_names = symbolic_remote_names.iter().collect::<Vec<_>>();
    symbolic_remote_names.sort_by_key(|name| name.len());
    let remote_names = symbolic_remote_names
        .iter()
        .map(|name| name.as_bytes().as_bstr().to_owned())
        .collect();
    Some(
        extract_remote_name_and_short_name(target_ref.as_ref(), &remote_names)
            .map(|(_, short_name)| short_name.to_string())
            .unwrap_or_else(|| target_ref.shorten().to_string()),
    )
}

fn review_integration_hints_from_reviews(
    target_branch_name: &str,
    incoming_commit_ids: &HashSet<String>,
    reviews: impl IntoIterator<Item = ForgeReview>,
) -> Vec<ReviewIntegrationHint> {
    let mut seen = HashSet::new();

    reviews
        .into_iter()
        .filter(|review| {
            review.is_merged()
                && review.target_branch == target_branch_name
                && review
                    .integration_commit_shas
                    .iter()
                    .any(|sha| incoming_commit_ids.contains(sha))
        })
        .filter_map(|review| gix::ObjectId::from_hex(review.sha.as_bytes()).ok())
        .filter(|head_commit_at_merge| seen.insert(*head_commit_at_merge))
        .map(|head_commit_at_merge| ReviewIntegrationHint {
            head_commit_at_merge,
        })
        .collect()
}

fn forge_review_integration_hints(
    workspace: &but_graph::Workspace,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    db: &but_db::DbHandle,
) -> anyhow::Result<Vec<ReviewIntegrationHint>> {
    let Some(target_branch_name) =
        target_branch_name(&workspace.graph.symbolic_remote_names, project_meta)
    else {
        return Ok(vec![]);
    };

    let incoming_commit_ids = workspace
        .incoming_target_commit_ids()?
        .into_iter()
        .map(|id| id.to_hex().to_string())
        .collect::<HashSet<_>>();
    if incoming_commit_ids.is_empty() {
        return Ok(vec![]);
    }

    let associated_reviews = but_forge::list_cached_forge_reviews(db)?;

    Ok(review_integration_hints_from_reviews(
        &target_branch_name,
        &incoming_commit_ids,
        associated_reviews,
    ))
}

/// Integrate upstream changes into the current workspace without recording an
/// oplog entry.
///
/// This acquires exclusive worktree access from `ctx`, applies the requested
/// upstream updates, and returns the resulting workspace state plus information about
/// worktree conflicts. When `dry_run`
/// is enabled, the returned workspace previews the integration without
/// materializing the rebase. See
/// [`workspace_integrate_upstream_only_with_perm()`] for lower-level details.
#[but_api(try_from = json::WorkspaceIntegrateUpstreamOutcome)]
#[instrument(err(Debug))]
pub fn workspace_integrate_upstream_only(
    ctx: &mut but_ctx::Context,
    updates: Vec<json::BottomUpdate>,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    workspace_integrate_upstream_only_with_perm(
        ctx,
        updates.into_iter().map(Into::into).collect(),
        dry_run,
        guard.write_permission(),
    )
}

/// Integrate upstream changes into the current workspace and record an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx`, applies the requested
/// upstream updates, and commits a best-effort `MergeUpstream` oplog snapshot
/// when the integration succeeds. When `dry_run` is enabled, the returned
/// workspace previews the integration and no oplog entry is persisted. See
/// [`workspace_integrate_upstream_with_perm()`] for lower-level details.
#[but_api(napi, try_from = json::WorkspaceIntegrateUpstreamOutcome)]
#[instrument(err(Debug))]
pub fn workspace_integrate_upstream(
    ctx: &mut but_ctx::Context,
    updates: Vec<json::BottomUpdate>,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    workspace_integrate_upstream_with_perm(
        ctx,
        updates.into_iter().map(Into::into).collect(),
        dry_run,
        guard.write_permission(),
    )
}

/// Integrate upstream changes under caller-held exclusive repository access
/// and record an oplog snapshot on success.
///
/// It prepares a best-effort `MergeUpstream` oplog snapshot, performs the
/// integration, and commits the snapshot only if the mutation succeeds. When
/// `dry_run` is enabled, it returns a preview of the resulting workspace state
/// plus worktree conflicts and skips oplog persistence.
/// For lower-level implementation details, see [`but_workspace::integrate_upstream()`].
pub fn workspace_integrate_upstream_with_perm(
    ctx: &mut but_ctx::Context,
    updates: Vec<but_workspace::BottomUpdate>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::MergeUpstream),
        perm.read_permission(),
        dry_run,
    );

    let result = workspace_integrate_upstream_only_with_perm(ctx, updates, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && result.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }

    result
}

/// Integrate upstream changes under caller-held exclusive repository access.
///
/// This delegates to [`but_workspace::integrate_upstream()`] and returns the
/// resulting workspace state plus worktree conflicts info.
/// When `dry_run` is enabled, it returns a preview of the resulting workspace
/// without materializing the rebase.
pub fn workspace_integrate_upstream_only_with_perm(
    ctx: &mut but_ctx::Context,
    updates: Vec<but_workspace::BottomUpdate>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceIntegrateUpstreamOutcome> {
    let mut meta = ctx.meta()?;
    let (workspace_state, worktree_conflicts) = {
        let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
        let project_meta = ctx.project_meta()?;
        let review_hints = match forge_review_integration_hints(&ws, &project_meta, &db) {
            Ok(review_hints) => review_hints,
            Err(err) => {
                warn!(
                    ?err,
                    "failed to derive forge review integration hints; continuing without hints"
                );
                Vec::new()
            }
        };
        let IntegrateUpstreamOutcome {
            mut rebase,
            ws_meta,
            project_meta,
        } = but_workspace::integrate_upstream_with_hints(
            &mut ws,
            &mut meta,
            project_meta,
            &repo,
            updates,
            &review_hints,
        )?;
        let worktree_conflicts = but_workspace::worktree_conflicts_for_rebase(&rebase)?;

        if dry_run.into() {
            let replaced_commits = rebase.history.commit_mappings();
            let workspace_state =
                WorkspaceState::from_rebase_preview_with_db(&mut rebase, replaced_commits, &db)?;
            return Ok(WorkspaceIntegrateUpstreamOutcome {
                workspace_state,
                worktree_conflicts,
            });
        }

        let materialized = rebase.materialize(Default::default())?;
        project_meta.persist(&repo)?;

        if let Some(ref_name) = materialized.workspace.ref_name()
            && let Some(ws_meta) = ws_meta
            && is_workspace_ref_name(ref_name)
        {
            let mut md = materialized.meta.workspace(ref_name)?;
            *md = ws_meta;
            materialized.meta.set_workspace(&md)?;
        }

        let workspace_state = WorkspaceState::from_workspace_with_db(
            materialized.workspace,
            materialized.meta,
            &repo,
            materialized.history.commit_mappings(),
            &db,
        )?;
        (workspace_state, worktree_conflicts)
    };
    ctx.invalidate_workspace_cache()?;

    Ok(WorkspaceIntegrateUpstreamOutcome {
        workspace_state,
        worktree_conflicts,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        review_integration_hints_from_reviews, target_branch_name, workspace_fetch_from_remotes,
    };
    use but_core::RefMetadata;
    use but_testsupport::{CommandExt, git_at_dir, open_repo};
    use std::collections::HashSet;
    use std::path::Path;

    fn repo_with_feature_branch() -> anyhow::Result<(gix::Repository, tempfile::TempDir)> {
        let tmp = tempfile::tempdir()?;
        git_at_dir(tmp.path()).args(["init"]).run();
        git_at_dir(tmp.path())
            .args(["config", "user.name", "GitButler"])
            .run();
        git_at_dir(tmp.path())
            .args(["config", "user.email", "gitbutler@example.com"])
            .run();
        write_file(tmp.path(), "file.txt", "one\n")?;
        git_at_dir(tmp.path()).args(["add", "file.txt"]).run();
        git_at_dir(tmp.path()).args(["commit", "-m", "one"]).run();
        git_at_dir(tmp.path()).args(["branch", "feature"]).run();
        git_at_dir(tmp.path())
            .args(["config", "remote.origin.url", "../origin"])
            .run();
        git_at_dir(tmp.path())
            .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
            .run();
        write_file(tmp.path(), "file.txt", "two\n")?;
        git_at_dir(tmp.path()).args(["commit", "-am", "two"]).run();

        Ok((open_repo(tmp.path())?, tmp))
    }

    fn write_file(root: &Path, relative_path: &str, content: &str) -> anyhow::Result<()> {
        std::fs::write(root.join(relative_path), content)?;
        Ok(())
    }

    #[test]
    fn failed_fetch_prunes_missing_branch_stack_order() -> anyhow::Result<()> {
        but_askpass::disable();
        let (repo, tmp) = repo_with_feature_branch()?;
        let feature: gix::refs::FullName = "refs/heads/feature".try_into()?;
        let main = repo.head_name()?.expect("HEAD is symbolic").to_owned();
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.meta()?
            .set_branch_stack_order(&[feature.clone(), main.clone()])?;

        git_at_dir(tmp.path())
            .args(["branch", "-D", "feature"])
            .run();

        workspace_fetch_from_remotes(&mut ctx, None)
            .expect_err("the configured origin does not exist");

        assert!(
            ctx.meta()?.branch_stack_order(main.as_ref())?.is_none(),
            "failed fetch should still prune missing branch-order references"
        );
        Ok(())
    }

    #[test]
    fn set_target_ref_accepts_remote_tracking_ref_and_persists_metadata() -> anyhow::Result<()> {
        let (repo, _tmp) = repo_with_feature_branch()?;
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;

        super::set_target_ref_and_init_project(
            &mut ctx,
            target_ref.as_ref(),
            Some("origin".into()),
        )?;

        let stored_meta = ctx.project_meta()?;
        assert_eq!(stored_meta.target_ref.as_ref(), Some(&target_ref));
        assert!(stored_meta.target_commit_id.is_some());
        assert_eq!(stored_meta.push_remote.as_deref(), Some("origin"));

        Ok(())
    }

    #[test]
    fn set_push_remote_preserves_target_metadata() -> anyhow::Result<()> {
        let (repo, tmp) = repo_with_feature_branch()?;
        drop(repo);
        git_at_dir(tmp.path())
            .args(["config", "remote.fork.url", "../fork"])
            .run();
        let repo = open_repo(tmp.path())?;
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
        super::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
        let before = ctx.project_meta()?;

        super::set_push_remote(&mut ctx, "fork".into())?;

        let after = ctx.project_meta()?;
        assert_eq!(after.target_ref, before.target_ref);
        assert_eq!(after.target_commit_id, before.target_commit_id);
        assert_eq!(after.push_remote.as_deref(), Some("fork"));
        Ok(())
    }

    #[test]
    fn set_target_ref_rejects_local_branch_refs() -> anyhow::Result<()> {
        let (repo, _tmp) = repo_with_feature_branch()?;
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        let target_ref = gix::refs::FullName::try_from("refs/heads/feature")?;

        let err = super::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)
            .expect_err("local branches are not valid default targets");
        assert_eq!(
            err.to_string(),
            "target ref 'refs/heads/feature' must be a remote tracking branch"
        );

        Ok(())
    }

    #[test]
    fn set_target_ref_uses_merge_base_not_target_tip() -> anyhow::Result<()> {
        let (_repo, tmp) = repo_with_feature_branch()?;
        git_at_dir(tmp.path()).args(["checkout", "feature"]).run();
        write_file(tmp.path(), "feature.txt", "feature\n")?;
        git_at_dir(tmp.path()).args(["add", "feature.txt"]).run();
        git_at_dir(tmp.path())
            .args(["commit", "-m", "feature"])
            .run();
        git_at_dir(tmp.path())
            .args(["update-ref", "refs/remotes/origin/feature", "HEAD"])
            .run();
        git_at_dir(tmp.path()).args(["checkout", "main"]).run();

        let repo = open_repo(tmp.path())?;
        let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/feature")?;
        let target_tip = repo
            .find_reference(target_ref.as_ref())?
            .peel_to_id()?
            .detach();
        let current_head = repo.head_id()?.detach();
        let expected_merge_base = repo.merge_base(current_head, target_tip)?.detach();
        assert_ne!(expected_merge_base, target_tip);

        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        super::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;

        let project_meta = ctx.project_meta()?;
        assert_eq!(project_meta.target_commit_id, Some(expected_merge_base));
        assert_eq!(project_meta.push_remote, None);

        Ok(())
    }

    fn review(
        sha: &str,
        integration_commit_shas: &[&str],
        target_branch: &str,
        merged_at: Option<&str>,
    ) -> but_forge::ForgeReview {
        but_forge::ForgeReview {
            html_url: "https://example.test/review/1".into(),
            number: 1,
            title: "review".into(),
            body: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature".into(),
            target_branch: target_branch.into(),
            sha: sha.into(),
            integration_commit_shas: integration_commit_shas
                .iter()
                .map(|s| s.to_string())
                .collect(),
            created_at: None,
            modified_at: None,
            merged_at: merged_at.map(str::to_owned),
            closed_at: merged_at.map(str::to_owned),
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            head_repo_is_fork: false,
            auto_merge_enabled: false,
            reviewers: vec![],
            unit_symbol: "#".into(),
            last_sync_at: Default::default(),
        }
    }

    #[test]
    fn target_branch_name_prefers_longest_matching_remote_name() {
        let project_meta = but_core::ref_metadata::ProjectMeta {
            target_ref: Some(
                "refs/remotes/foo/bar/main"
                    .try_into()
                    .expect("target ref is a valid full ref name"),
            ),
            ..Default::default()
        };
        let remote_names = vec!["foo".to_string(), "foo/bar".to_string()];

        assert_eq!(
            target_branch_name(&remote_names, &project_meta).as_deref(),
            Some("main"),
            "the longest matching remote name should be stripped from the target ref"
        );
    }

    #[test]
    fn review_hints_keep_only_merged_reviews_on_the_target_branch() {
        let hints = review_integration_hints_from_reviews(
            "main",
            &HashSet::from(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]),
            vec![
                review(
                    "1234567890abcdef1234567890abcdef12345678",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    Some("2026-06-24T12:00:00Z"),
                ),
                review(
                    "abcdef1234567890abcdef1234567890abcdef12",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "release",
                    Some("2026-06-24T12:00:00Z"),
                ),
                review(
                    "fedcba9876543210fedcba9876543210fedcba98",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    None,
                ),
            ],
        );

        assert_eq!(
            hints.len(),
            1,
            "only merged reviews targeting the current branch produce hints"
        );
        assert_eq!(
            hints[0].head_commit_at_merge.to_hex().to_string(),
            "1234567890abcdef1234567890abcdef12345678",
            "the hint should use the review head SHA reported by the forge"
        );
    }

    #[test]
    fn review_hints_dedupe_duplicate_review_heads() {
        let hints = review_integration_hints_from_reviews(
            "main",
            &HashSet::from(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]),
            vec![
                review(
                    "1234567890abcdef1234567890abcdef12345678",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    Some("2026-06-24T12:00:00Z"),
                ),
                review(
                    "1234567890abcdef1234567890abcdef12345678",
                    &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                    "main",
                    Some("2026-06-24T12:00:00Z"),
                ),
            ],
        );

        assert_eq!(
            hints.len(),
            1,
            "multiple incoming commits may map to the same merged review head"
        );
    }

    #[test]
    fn review_hints_ignore_reviews_without_matching_incoming_commit() {
        let hints = review_integration_hints_from_reviews(
            "main",
            &HashSet::from(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]),
            vec![review(
                "1234567890abcdef1234567890abcdef12345678",
                &["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],
                "main",
                Some("2026-06-24T12:00:00Z"),
            )],
        );

        assert!(
            hints.is_empty(),
            "cached review hints should only match reviews whose landing commit is among incoming upstream commits"
        );
    }
}
