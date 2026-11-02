use crate::commit::json::ChangesSource;
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::{Commit, with_transaction_with_perm_only};
use but_workspace::commit::ChangeSource;
use gix::bstr::ByteSlice;
use tracing::instrument;

use super::types::CommitCreateResult;

/// Creates a commit from the `changes` of `changes_source` with `message`,
/// inserted on `side` of `relative_to`.
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// commit. For lower-level implementation details, see
/// [`but_workspace::commit::commit_create()`]. When `dry_run` is enabled, the
/// returned workspace previews the inserted commit without materializing the
/// rebase.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_create_only(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    changes_source: ChangesSource,
    message: String,
    dry_run: DryRun,
) -> anyhow::Result<CommitCreateResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        &changes_source,
        message,
        dry_run,
        guard.write_permission(),
    )
}

/// Creates and inserts a commit relative to either a commit or a reference.
///
/// When `dry_run` is enabled, the returned workspace previews the inserted
/// commit without materializing the rebase.
#[expect(clippy::too_many_arguments)]
pub(crate) fn commit_create_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    changes_source: &ChangesSource,
    message: String,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let worktree = crate::worktrees::open_changes_source(ctx, changes_source)?;
    let (_, (created, workspace)) =
        with_transaction_with_perm_only(ctx, perm, dry_run, |mut tx| {
            let source = worktree
                .as_ref()
                .map_or(ChangeSource::Head, |(name, repo)| ChangeSource::Worktree {
                    repo,
                    name: name.as_bstr(),
                });
            Ok(Commit(tx.create_commit(
                relative_to,
                side,
                changes,
                message,
                source,
            )?))
        })?;

    Ok(CommitCreateResult {
        new_commit: created.new_commit.map(|commit| commit.id),
        rejected_specs: created.rejected_specs,
        workspace,
    })
}

/// Insert a new commit built from the `changes` of `changes_source` and record
/// an oplog snapshot on success.
///
/// `relative_to` and `side` choose where the commit is inserted. `message` is
/// the entire commit message text, not just the title. On success, this commits
/// a best-effort `CreateCommit` oplog snapshot using the same lock. For a linked
/// worktree `changes_source` it also covers the branch that worktree has checked
/// out, so undo leaves the change uncommitted there. When `dry_run` is enabled,
/// the returned workspace previews the inserted commit and no oplog entry is
/// persisted. For lower-level implementation details, see
/// [`but_workspace::commit::commit_create()`].
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(skip_all, fields(relative_to, side, message), err(Debug))]
#[expect(clippy::too_many_arguments)]
pub fn commit_create(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    changes_source: ChangesSource,
    message: String,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let worktree_ref = crate::worktrees::changes_source_ref(ctx, &changes_source)?;
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateCommit),
        worktree_ref.as_ref().map(|name| name.as_ref()).as_slice(),
        perm.read_permission(),
        dry_run,
    );

    let res = commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        &changes_source,
        message,
        dry_run,
        perm,
    );
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
