use crate::commit::json::ChangesSource;
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_transaction::{Commit, with_transaction_with_perm_only};
use but_workspace::commit::ChangeSource;
use gix::bstr::ByteSlice;
use tracing::instrument;

use super::types::CommitCreateResult;

/// Amends the commit at `commit_id` with the `changes` of `changes_source`.
///
/// See [`but_workspace::commit::commit_amend()`] for lower-level implementation
/// details. When `dry_run` is enabled, the returned workspace previews the
/// amended commit without materializing the rebase.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    changes_source: ChangesSource,
    dry_run: DryRun,
) -> anyhow::Result<CommitCreateResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        &changes_source,
        dry_run,
        guard.write_permission(),
    )
}

pub(crate) fn commit_amend_only_impl(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    changes_source: &ChangesSource,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let worktree = crate::worktrees::open_changes_source(ctx, changes_source)?;
    let mut meta = ctx.meta()?;
    let (_, (amended, workspace)) =
        with_transaction_with_perm_only(ctx, &mut meta, perm, dry_run, |mut tx| {
            let source = worktree
                .as_ref()
                .map_or(ChangeSource::Head, |(name, repo)| ChangeSource::Worktree {
                    repo,
                    name: name.as_bstr(),
                });
            Ok(Commit(tx.amend_commit(commit_id, changes, source)?))
        })?;

    Ok(CommitCreateResult {
        new_commit: amended.new_commit.map(|commit| commit.id),
        rejected_specs: amended.rejected_specs,
        workspace,
    })
}

/// Amend the commit at `commit_id` with the `changes` of `changes_source` and
/// record an oplog snapshot on success.
///
/// This performs the rewrite under exclusive worktree access and creates a
/// best-effort `AmendCommit` oplog entry if the operation succeeds. For a linked
/// worktree `changes_source` it also covers the branch that worktree has checked
/// out, so undo leaves the change uncommitted there. When `dry_run` is enabled,
/// the returned workspace previews the amended commit and no oplog entry is
/// persisted. For lower-level implementation details, see
/// [`but_workspace::commit::commit_amend()`].
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
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::AmendCommit),
        worktree_ref.as_ref().map(|name| name.as_ref()).as_slice(),
        guard.read_permission(),
        dry_run,
    );

    let res = commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        &changes_source,
        dry_run,
        guard.write_permission(),
    );
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, guard.write_permission()).ok();
    }
    res
}
