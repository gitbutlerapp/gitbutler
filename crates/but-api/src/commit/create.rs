use std::collections::BTreeMap;

use but_api_macros::but_api;
use but_core::{DiffSpec, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{GraphExt, LookupStep as _, mutate::InsertSide};
use tracing::instrument;

use super::types::{CommitCreateResult, RelativeTo};

/// Creates and inserts a commit relative to either a commit or a reference.
#[but_api(crate::commit::json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_create_only(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        message,
        context_lines,
        guard.write_permission(),
    )
}

/// Creates and inserts a commit relative to either a commit or a reference.
pub(crate) fn commit_create_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    context_lines: u32,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let meta = ctx.meta()?;
    let (repo, mut ws, _, _cache) = ctx.workspace_mut_and_db_and_cache_with_perm(perm)?;
    let editor = ws.graph.to_editor(&repo)?;
    let but_workspace::commit::CommitCreateOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_create(
        editor,
        changes,
        relative_to,
        side,
        &message,
        context_lines,
    )?;

    let (new_commit, replaced_commits) = match commit_selector {
        Some(commit_selector) => {
            let materialized = rebase.materialize()?;
            let new_commit = materialized.lookup_pick(commit_selector)?;
            let replaced_commits = materialized.history.commit_mappings();
            (Some(new_commit), replaced_commits)
        }
        None => (None, BTreeMap::new()),
    };

    ws.refresh_from_head(&repo, &meta)?;

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        replaced_commits,
    })
}

/// Creates and inserts a commit relative to either a commit or a reference, with oplog support.
#[but_api(napi, crate::commit::json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_create(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::CreateCommit),
    )
    .ok();

    let mut guard = ctx.exclusive_worktree_access();
    let res = commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        message,
        context_lines,
        guard.write_permission(),
    );
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}
