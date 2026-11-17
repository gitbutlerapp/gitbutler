use std::collections::HashSet;

use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::{Commit, with_transaction_with_perm};
use tracing::instrument;

use super::types::CommitCherryPickResult;

/// Cherry-picks `source_commit_ids` to `side` of `relative_to`.
///
/// Sources may live anywhere in the repository, including outside the
/// workspace. They are deduplicated and applied in the order given.
#[but_api(try_from = crate::commit::json::CommitCherryPickResult)]
#[instrument(err(Debug))]
pub fn commit_cherry_pick(
    ctx: &mut but_ctx::Context,
    source_commit_ids: Vec<gix::ObjectId>,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitCherryPickResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_cherry_pick_with_perm(
        ctx,
        source_commit_ids,
        relative_to,
        side,
        dry_run,
        guard.write_permission(),
    )
}

pub fn commit_cherry_pick_with_perm(
    ctx: &mut but_ctx::Context,
    source_commit_ids: Vec<gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCherryPickResult> {
    let source_commit_count = source_commit_ids.iter().collect::<HashSet<_>>().len();
    let mut meta = ctx.meta()?;
    let (new_commits, workspace) = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::CherryPick).with_count(source_commit_count),
        &[],
        dry_run,
        |mut tx| {
            let picked = tx.cherry_pick_commits(source_commit_ids, relative_to, side, false)?;
            Ok(Commit(picked.into_iter().map(|commit| commit.id).collect()))
        },
    )?;

    Ok(CommitCherryPickResult {
        new_commits,
        workspace,
    })
}
