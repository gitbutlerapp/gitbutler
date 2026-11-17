use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::{Commit, with_transaction_with_perm};
use tracing::instrument;

use super::types::CommitInsertBlankResult;

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
