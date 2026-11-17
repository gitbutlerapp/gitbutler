use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_transaction::{Commit, with_transaction_with_perm};
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use tracing::instrument;

use super::types::CommitSquashResult;

#[but_api(try_from = crate::commit::json::CommitSquashResult)]
#[instrument(err(Debug))]
pub fn commit_squash(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    target_commit_id: gix::ObjectId,
    how_to_combine_messages: MessageCombinationStrategy,
    dry_run: DryRun,
) -> anyhow::Result<CommitSquashResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_squash_with_perm(
        ctx,
        subject_commit_ids,
        target_commit_id,
        how_to_combine_messages,
        dry_run,
        guard.write_permission(),
    )
}

pub fn commit_squash_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    target_commit_id: gix::ObjectId,
    how_to_combine_messages: MessageCombinationStrategy,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitSquashResult> {
    if subject_commit_ids.is_empty() {
        anyhow::bail!("No commits were provided to squash")
    }
    let mut meta = ctx.meta()?;
    let (new_commit, workspace) = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::SquashCommit),
        &[],
        dry_run,
        |mut tx| {
            let squashed = tx.squash_commits(
                subject_commit_ids,
                target_commit_id,
                how_to_combine_messages,
            )?;
            Ok(Commit(squashed.id))
        },
    )?;

    Ok(CommitSquashResult {
        new_commit,
        workspace,
    })
}
