use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_transaction::with_transaction_with_perm;
use tracing::instrument;

use super::types::MoveChangesResult;

/// Moves `changes` from `source_commit_id` to `destination_commit_id` and
/// records an oplog snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// changes.
///
/// When `dry_run` is enabled, the returned workspace previews the rewritten
/// commits and no oplog entry is persisted. For details, see
/// [`commit_move_changes_between_with_perm()`].
#[but_api(try_from = crate::commit::json::MoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_move_changes_between(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    dry_run: DryRun,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_move_changes_between_with_perm(
        ctx,
        source_commit_id,
        destination_commit_id,
        changes,
        dry_run,
        guard.write_permission(),
    )
}

/// Move `changes` from `source_commit_id` into `destination_commit_id`
/// under caller-held exclusive repository access and record an oplog snapshot
/// on success.
///
/// This records a best-effort `MoveCommitFile` oplog snapshot when the rebase
/// is materialized. When `dry_run` is enabled, it returns a preview of the
/// workspace state and skips oplog persistence. For lower-level implementation
/// details, see [`but_workspace::commit::move_changes_between_commits()`].
pub fn commit_move_changes_between_with_perm(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let mut meta = ctx.meta()?;
    let workspace = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::MoveCommitFile),
        &[],
        dry_run,
        |mut tx| {
            tx.move_committed_changes_between(source_commit_id, destination_commit_id, changes)?;
            Ok(())
        },
    )?;

    Ok(MoveChangesResult { workspace })
}
