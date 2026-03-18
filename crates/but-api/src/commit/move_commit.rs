use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    GraphExt,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use super::types::CommitMoveResult;

/// Moves commit, no snapshots. No strings attached.
///
/// Returns the replaced that resulted from the operation.
pub fn commit_move_only(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    relative_to: crate::commit::json::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<CommitMoveResult> {
    let meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
    let editor = ws.graph.to_editor(&repo)?;

    let relative_to: RelativeTo = (&relative_to).into();

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
#[but_api_macros::but_api(napi, crate::commit::json::UICommitMoveResult)]
#[instrument(err(Debug))]
pub fn commit_move(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    relative_to: crate::commit::json::RelativeTo,
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
