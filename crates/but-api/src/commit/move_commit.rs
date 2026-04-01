use but_api_macros::but_api;
use but_core::sync::RepoExclusive;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    Editor,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use super::types::CommitMoveResult;

/// Moves `subject_commit_id` to `side` of `relative_to`.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// commit.
///
/// For details, see [`commit_move_only_with_perm()`].
#[but_api(crate::commit::json::UICommitMoveResult)]
pub fn commit_move_only(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
) -> anyhow::Result<CommitMoveResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_move_only_with_perm(
        ctx,
        subject_commit_id,
        relative_to,
        side,
        guard.write_permission(),
    )
}

/// Move `subject_commit_id` to the `side` of `relative_to` under
/// caller-held exclusive repository access.
///
/// This materializes the rebase and returns the commit-ID mapping for rewritten
/// descendants. This variant does not create an oplog entry. For lower-level
/// implementation details, see [`but_workspace::commit::move_commit()`].
pub fn commit_move_only_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    relative_to: RelativeTo,
    side: InsertSide,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitMoveResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _, _cache) = ctx.workspace_mut_and_db_and_cache_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let rebase = but_workspace::commit::move_commit(editor, subject_commit_id, relative_to, side)?;

    let materialized = rebase.materialize()?;

    Ok(CommitMoveResult {
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Moves `subject_commit_id` to `side` of `relative_to` and records an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// commit.
///
/// For details, see [`commit_move_with_perm()`].
#[but_api(napi, crate::commit::json::UICommitMoveResult)]
#[instrument(err(Debug))]
pub fn commit_move(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
) -> anyhow::Result<CommitMoveResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_move_with_perm(
        ctx,
        subject_commit_id,
        relative_to,
        side,
        guard.write_permission(),
    )
}

/// Moves `subject_commit_id` to `side` of `relative_to` under caller-held
/// exclusive repository access and records an oplog snapshot on success.
///
/// It prepares a best-effort `MoveCommit` oplog snapshot, performs the move,
/// and commits the snapshot only if the operation succeeds. For lower-level
/// implementation details, see [`but_workspace::commit::move_commit()`].
pub fn commit_move_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    relative_to: RelativeTo,
    side: InsertSide,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitMoveResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::MoveCommit),
        perm.read_permission(),
    )
    .ok();

    let res = commit_move_only_with_perm(ctx, subject_commit_id, relative_to, side, perm);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, perm).ok();
    };
    res
}
