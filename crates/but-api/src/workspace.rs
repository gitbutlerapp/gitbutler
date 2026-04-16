//! Functions that operate on the workspace.

use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{DryRun, RefMetadata, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_workspace::IntegrateUpstreamOutcome;
use tracing::instrument;

/// JSON transport types for workspace APIs.
pub mod json {
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
}

/// Integrate upstream changes into the current workspace without recording an
/// oplog entry.
///
/// This acquires exclusive worktree access from `ctx`, applies the requested
/// upstream updates, and returns the resulting workspace state. When `dry_run`
/// is enabled, the returned workspace previews the integration without
/// materializing the rebase. See
/// [`workspace_integrate_upstream_only_with_perm()`] for lower-level details.
#[but_api(try_from = crate::json::WorkspaceState)]
#[instrument(err(Debug))]
pub fn workspace_integrate_upstream_only(
    ctx: &mut but_ctx::Context,
    updates: Vec<json::BottomUpdate>,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceState> {
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
#[but_api(napi, try_from = crate::json::WorkspaceState)]
#[instrument(err(Debug))]
pub fn workspace_integrate_upstream(
    ctx: &mut but_ctx::Context,
    updates: Vec<json::BottomUpdate>,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceState> {
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
/// and skips oplog persistence. For lower-level implementation details, see
/// [`but_workspace::integrate_upstream()`].
pub fn workspace_integrate_upstream_with_perm(
    ctx: &mut but_ctx::Context,
    updates: Vec<but_workspace::BottomUpdate>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceState> {
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
/// resulting workspace state. When `dry_run` is enabled, it returns a preview
/// of the resulting workspace without materializing the rebase.
pub fn workspace_integrate_upstream_only_with_perm(
    ctx: &mut but_ctx::Context,
    updates: Vec<but_workspace::BottomUpdate>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceState> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let IntegrateUpstreamOutcome { rebase, ws_meta } =
        but_workspace::integrate_upstream(&mut ws, &mut meta, &repo, updates)?;

    if dry_run.into() {
        return WorkspaceState::from_rebase_preview(&rebase, rebase.history.commit_mappings());
    }

    let materialized = rebase.materialize()?;

    if let Some(ref_name) = materialized.workspace.ref_name() {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        materialized.meta.set_workspace(&md)?;
    }

    WorkspaceState::from_workspace(
        materialized.workspace,
        &repo,
        materialized.history.commit_mappings(),
    )
}
