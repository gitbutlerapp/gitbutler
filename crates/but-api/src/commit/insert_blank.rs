use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::{Commit, with_transaction_with_perm};
use tracing::instrument;

use super::types::CommitInsertBlankResult;

/// Inserts a blank commit on `side` of `relative_to` and records an oplog
/// snapshot on success.
///
/// When `dry_run` is enabled, the returned workspace previews the inserted
/// commit and no oplog entry is persisted. For details, see
/// [`commit_insert_blank_with_perm()`].
#[but_api(try_from = crate::commit::json::CommitInsertBlankResult)]
#[instrument(err(Debug))]
pub fn commit_insert_blank(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitInsertBlankResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_insert_blank_with_perm(ctx, relative_to, side, dry_run, guard.write_permission())
}

/// Create an empty commit next to `relative_to` under caller-held exclusive
/// repository access and record an oplog snapshot on success.
///
/// `side` chooses whether the blank commit lands before or after `relative_to`.
/// This records a best-effort `InsertBlankCommit` oplog snapshot when the
/// commit is materialized. When `dry_run` is enabled, it returns a preview of
/// the resulting workspace state and skips oplog persistence. For lower-level
/// implementation details, see [`but_workspace::commit::insert_blank_commit()`].
pub fn commit_insert_blank_with_perm(
    ctx: &mut but_ctx::Context,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitInsertBlankResult> {
    let mut meta = ctx.meta()?;
    let (new_commit, workspace) = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
        &[],
        dry_run,
        |mut tx| Ok(Commit(tx.insert_blank_commit(relative_to, side)?.id)),
    )?;

    Ok(CommitInsertBlankResult {
        new_commit,
        workspace,
    })
}
