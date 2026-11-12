use crate::commit::json::ChangesSource;
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_transaction::{Commit, with_transaction_with_perm};
use but_workspace::commit::ChangeSource;
use gix::bstr::ByteSlice;
use tracing::instrument;

use super::types::CommitCreateResult;

/// Amend the commit at `commit_id` with the `changes` of `changes_source`.
///
/// For a linked worktree `changes_source`, undo also covers the branch that
/// worktree has checked out, so it leaves the change uncommitted there.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    changes_source: ChangesSource,
    dry_run: DryRun,
) -> anyhow::Result<CommitCreateResult> {
    let mut guard = ctx.exclusive_worktree_access();
    let worktree_ref = crate::worktrees::changes_source_ref(ctx, &changes_source)?;
    let worktree = crate::worktrees::open_changes_source(ctx, &changes_source)?;
    let mut meta = ctx.meta()?;
    let (amended, workspace) = with_transaction_with_perm(
        ctx,
        &mut meta,
        guard.write_permission(),
        SnapshotDetails::new(OperationKind::AmendCommit),
        worktree_ref.as_ref().map(|name| name.as_ref()).as_slice(),
        dry_run,
        |mut tx| {
            let source = worktree
                .as_ref()
                .map_or(ChangeSource::Head, |(name, repo)| ChangeSource::Worktree {
                    repo,
                    name: name.as_bstr(),
                });
            Ok(Commit(tx.amend_commit(commit_id, changes, source)?))
        },
    )?;

    Ok(CommitCreateResult {
        new_commit: amended.new_commit.map(|commit| commit.id),
        rejected_specs: amended.rejected_specs,
        workspace,
    })
}
