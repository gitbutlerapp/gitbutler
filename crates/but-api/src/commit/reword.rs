use bstr::{BString, ByteSlice};
use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_transaction::{Commit, with_transaction_with_perm};
use tracing::instrument;

use super::types::CommitRewordResult;

/// Replace the message of `commit_id` with `message`.
///
/// `message` is the full commit message: the title line, and when a body is
/// present, `\n\n` followed by the body.
#[but_api(try_from = crate::commit::json::CommitRewordResult)]
#[instrument(err(Debug))]
pub fn commit_reword(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
    dry_run: DryRun,
) -> anyhow::Result<CommitRewordResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_reword_with_perm(ctx, commit_id, message, dry_run, guard.write_permission())
}

pub fn commit_reword_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitRewordResult> {
    let mut meta = ctx.meta()?;
    let (new_commit, workspace) = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::UpdateCommitMessage),
        &[],
        dry_run,
        |mut tx| Ok(Commit(tx.reword_commit(commit_id, message.as_bstr())?.id)),
    )?;

    Ok(CommitRewordResult {
        new_commit,
        workspace,
    })
}
