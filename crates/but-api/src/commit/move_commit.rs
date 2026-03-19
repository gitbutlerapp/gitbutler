use but_api_macros::but_api;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{GraphExt, mutate::InsertSide};
use tracing::instrument;

use super::types::{CommitMoveResult, RelativeTo};

/// Moves commit, no snapshots. No strings attached.
///
/// Returns the replaced that resulted from the operation.
#[but_api(crate::commit::json::UICommitMoveResult)]
pub fn commit_move_only(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
) -> anyhow::Result<CommitMoveResult> {
    let meta = ctx.meta()?;
    let (_guard, repo, mut ws, _, _cache) = ctx.workspace_mut_and_db_and_cache()?;
    let editor = ws.graph.to_editor(&repo)?;

    let rebase =
        but_workspace::commit::move_commit(editor, &ws, subject_commit_id, relative_to, side)?;

    let materialized = rebase.materialize()?;
    ws.refresh_from_head(&repo, &meta)?;

    Ok(CommitMoveResult {
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Moves a commit within or across stacks.
///
/// Returns the replaced that resulted from the operation.
#[but_api(napi, crate::commit::json::UICommitMoveResult)]
#[instrument(err(Debug))]
pub fn commit_move(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
) -> anyhow::Result<CommitMoveResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::MoveCommit),
    )
    .ok();

    let res = commit_move_only(ctx, subject_commit_id, relative_to, side);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}
