use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use super::types::CommitInsertBlankResult;

/// Inserts a blank commit on `side` of `relative_to`.
///
/// `side` chooses whether the blank commit lands before or after `relative_to`.
/// When `dry_run` is enabled, the returned workspace previews the inserted
/// commit without materializing the rebase.
#[but_api(try_from = crate::commit::json::CommitInsertBlankResult)]
#[instrument(err(Debug))]
pub fn commit_insert_blank_only(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitInsertBlankResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_insert_blank_only_impl(ctx, relative_to, side, dry_run, guard.write_permission())
}

/// Create an empty commit next to `relative_to` under caller-held exclusive
/// repository access.
///
/// When `dry_run` is enabled, the returned workspace previews the inserted
/// commit without materializing the rebase.
pub(crate) fn commit_insert_blank_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitInsertBlankResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let (rebase, blank_commit_selector) =
        but_workspace::commit::insert_blank_commit(editor, side, relative_to)?;
    let new_commit = rebase.lookup_pick(blank_commit_selector)?;
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    Ok(CommitInsertBlankResult {
        new_commit,
        workspace,
    })
}

/// Inserts a blank commit on `side` of `relative_to` and records an oplog
/// snapshot on success.
///
/// When `dry_run` is enabled, the returned workspace previews the inserted
/// commit and no oplog entry is persisted. For details, see
/// [`commit_insert_blank_with_perm()`].
#[but_api(napi, try_from = crate::commit::json::CommitInsertBlankResult)]
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

/// Create an empty commit next to `relative_to` under caller-held exclusive
/// repository access and record an oplog snapshot on success.
///
/// `side` chooses whether the blank commit lands before or after `relative_to`.
/// This prepares a best-effort `InsertBlankCommit` oplog snapshot, creates the
/// commit, and commits the snapshot only if the operation succeeds. When
/// `dry_run` is enabled, it returns a preview of the resulting workspace state
/// and skips oplog persistence. For lower-level implementation details, see
/// [`but_workspace::commit::insert_blank_commit()`].
pub fn commit_insert_blank_with_perm(
    ctx: &mut but_ctx::Context,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitInsertBlankResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
        perm.read_permission(),
        dry_run,
    );

    let res = commit_insert_blank_only_impl(ctx, relative_to, side, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
