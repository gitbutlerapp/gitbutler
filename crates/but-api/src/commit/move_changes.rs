use but_api_macros::but_api;
use but_core::sync::RepoExclusive;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::Editor;
use tracing::instrument;

use super::types::MoveChangesResult;

/// Moves `changes` from `source_commit_id` to `destination_commit_id`.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// changes.
///
/// For details, see [`commit_move_changes_between_only_with_perm()`].
#[but_api(crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_move_changes_between_only(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_move_changes_between_only_with_perm(
        ctx,
        source_commit_id,
        destination_commit_id,
        changes,
        guard.write_permission(),
    )
}

/// Move `changes` from `source_commit_id` into `destination_commit_id`
/// under caller-held exclusive repository access.
///
/// It materializes the move-changes rebase and returns the replaced-commit
/// mapping. For lower-level implementation details, see
/// [`but_workspace::commit::move_changes_between_commits()`].
pub fn commit_move_changes_between_only_with_perm(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _, _cache) = ctx.workspace_mut_and_db_and_cache_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let outcome = but_workspace::commit::move_changes_between_commits(
        editor,
        source_commit_id,
        destination_commit_id,
        changes,
        context_lines,
    )?;
    let materialized = outcome.rebase.materialize()?;

    Ok(MoveChangesResult {
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Moves `changes` from `source_commit_id` to `destination_commit_id` and
/// records an oplog snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// changes.
///
/// For details, see [`commit_move_changes_between_with_perm()`].
#[but_api(napi, crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_move_changes_between(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_move_changes_between_with_perm(
        ctx,
        source_commit_id,
        destination_commit_id,
        changes,
        guard.write_permission(),
    )
}

/// Move `changes` from `source_commit_id` into `destination_commit_id`
/// under caller-held exclusive repository access and record an oplog snapshot
/// on success.
///
/// This prepares a best-effort `MoveCommitFile` oplog snapshot, performs the
/// rebase, and commits the snapshot only if the operation succeeds. For
/// lower-level implementation details, see
/// [`but_workspace::commit::move_changes_between_commits()`].
pub fn commit_move_changes_between_with_perm(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::MoveCommitFile),
        perm.read_permission(),
    )
    .ok();

    let res = commit_move_changes_between_only_with_perm(
        ctx,
        source_commit_id,
        destination_commit_id,
        changes,
        perm,
    );
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, perm).ok();
    };
    res
}
