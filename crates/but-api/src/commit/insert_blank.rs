use but_api_macros::but_api;
use but_core::sync::RepoExclusive;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    GraphExt, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use super::types::CommitInsertBlankResult;

/// Inserts a blank commit relative to either a commit or a reference
///
/// Returns the result including the new commit ID and any replaced commits.
#[but_api(crate::commit::json::UICommitInsertBlankResult)]
#[instrument(err(Debug))]
pub fn commit_insert_blank_only(
    ctx: &mut but_ctx::Context,
    relative_to: crate::commit::json::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<CommitInsertBlankResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_insert_blank_only_impl(ctx, relative_to, side, guard.write_permission())
}

/// Implementation of inserting a blank commit relative to either a commit or a reference
///
/// Returns the result including the new commit ID and any replaced commits.
pub(crate) fn commit_insert_blank_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: crate::commit::json::RelativeTo,
    side: InsertSide,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitInsertBlankResult> {
    let meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = ws.graph.to_editor(&repo)?;
    let relative_to: RelativeTo = (&relative_to).into();

    let (outcome, blank_commit_selector) =
        but_workspace::commit::insert_blank_commit(editor, side, relative_to)?;

    let outcome = outcome.materialize()?;
    let id = outcome.lookup_pick(blank_commit_selector)?;
    let replaced_commits = outcome.history.commit_mappings();

    ws.refresh_from_head(&repo, &meta)?;

    Ok(CommitInsertBlankResult {
        new_commit: id,
        replaced_commits,
    })
}

/// Inserts a blank commit relative to either a commit or a reference, with oplog support
///
/// Returns the result including the new commit ID and any replaced commits.
#[but_api(napi, crate::commit::json::UICommitInsertBlankResult)]
#[instrument(err(Debug))]
pub fn commit_insert_blank(
    ctx: &mut but_ctx::Context,
    relative_to: crate::commit::json::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<CommitInsertBlankResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
    )
    .ok();

    let mut guard = ctx.exclusive_worktree_access();
    let res = commit_insert_blank_only_impl(ctx, relative_to, side, guard.write_permission());
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}
