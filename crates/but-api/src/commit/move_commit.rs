use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::with_transaction_with_perm;
use tracing::instrument;

use super::types::CommitMoveResult;

/// Moves `subject_commit_ids` to `side` of `relative_to` and records an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// commit.
///
/// When `dry_run` is enabled, the returned workspace previews the moved commit
/// and no oplog entry is persisted. For details, see [`commit_move_with_perm()`].
#[but_api(try_from = crate::commit::json::CommitMoveResult)]
#[instrument(err(Debug))]
pub fn commit_move(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitMoveResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_move_with_perm(
        ctx,
        subject_commit_ids,
        relative_to,
        side,
        dry_run,
        guard.write_permission(),
    )
}

/// Moves `subject_commit_ids` to `side` of `relative_to` under caller-held
/// exclusive repository access and records an oplog snapshot on success.
///
/// This records a best-effort `MoveCommit` oplog snapshot when the move is
/// materialized. When `dry_run` is enabled, it returns a preview of the
/// resulting workspace state and skips oplog persistence. For lower-level
/// implementation details, see [`but_workspace::commit::move_commits()`].
pub fn commit_move_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitMoveResult> {
    let details =
        SnapshotDetails::new(OperationKind::MoveCommit).with_count(subject_commit_ids.len());
    let mut meta = ctx.meta()?;
    let workspace =
        with_transaction_with_perm(ctx, &mut meta, perm, details, &[], dry_run, |mut tx| {
            tx.move_commits(subject_commit_ids, relative_to, side)
        })?;

    Ok(CommitMoveResult { workspace })
}
