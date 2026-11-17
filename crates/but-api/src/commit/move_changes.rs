use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_transaction::with_transaction_with_perm;
use tracing::instrument;

use super::types::MoveChangesResult;

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
