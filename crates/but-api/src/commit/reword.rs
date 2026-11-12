use bstr::{BString, ByteSlice};
use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_transaction::{Commit, with_transaction_with_perm};
use tracing::instrument;

use super::types::CommitRewordResult;

/// Reword `commit_id` to `message` using the behavior described by
/// [`commit_reword_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx` before rewriting the
/// commit message and recording the oplog entry. When `dry_run` is enabled,
/// the returned workspace previews the rewritten message and no oplog entry is
/// persisted.
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

/// Rewords `commit_id` to `message` under caller-held exclusive repository
/// access and records an oplog snapshot on success.
///
/// `message` must be the full commit message payload: the title line, and when a
/// body is present, `\n\n` followed by the body. This records a best-effort
/// `UpdateCommitMessage` oplog snapshot when the reword is materialized, and
/// returns the new commit id plus the replaced-commit mapping. When `dry_run`
/// is enabled, it returns a preview of the resulting workspace state and skips
/// oplog persistence. For lower-level implementation details, see
/// [`but_workspace::commit::reword()`].
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
