use but_api_macros::but_api;
use but_core::sync::RepoExclusive;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use tracing::instrument;

use super::types::CommitSquashResult;

/// Squash `subject_commit_id` into `target_commit_id`.
///
/// This acquires exclusive worktree access from `ctx` before rewriting the
/// commits.
///
/// For details, see [`commit_squash_only_with_perm()`].
#[but_api(crate::commit::json::UICommitSquashResult)]
#[instrument(err(Debug))]
pub fn commit_squash_only(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    target_commit_id: gix::ObjectId,
) -> anyhow::Result<CommitSquashResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_squash_only_with_perm(
        ctx,
        subject_commit_id,
        target_commit_id,
        guard.write_permission(),
    )
}

/// Squash `subject_commit_id` into `target_commit_id` under caller-held
/// exclusive repository access.
///
/// This materializes the squash rebase and returns the resulting squashed
/// commit ID together with rewritten commit mappings. This variant does not
/// create an oplog entry. For lower-level implementation details, see
/// [`but_workspace::commit::squash_commits()`].
pub fn commit_squash_only_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    target_commit_id: gix::ObjectId,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitSquashResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _, _cache) = ctx.workspace_mut_and_db_and_cache_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let outcome =
        but_workspace::commit::squash_commits(editor, subject_commit_id, target_commit_id)?;

    let materialized = outcome.rebase.materialize()?;
    let new_commit = materialized.lookup_pick(outcome.commit_selector)?;

    Ok(CommitSquashResult {
        new_commit,
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Squash `subject_commit_id` into `target_commit_id` and record an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before rewriting the
/// commits.
///
/// For details, see [`commit_squash_with_perm()`].
#[but_api(napi, crate::commit::json::UICommitSquashResult)]
#[instrument(err(Debug))]
pub fn commit_squash(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    target_commit_id: gix::ObjectId,
) -> anyhow::Result<CommitSquashResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_squash_with_perm(
        ctx,
        subject_commit_id,
        target_commit_id,
        guard.write_permission(),
    )
}

/// Squash `subject_commit_id` into `target_commit_id` under caller-held
/// exclusive repository access and record an oplog snapshot on success.
///
/// It prepares a best-effort `SquashCommit` oplog snapshot, performs the
/// squash, and commits the snapshot only if the operation succeeds. For
/// lower-level implementation details, see
/// [`but_workspace::commit::squash_commits()`].
pub fn commit_squash_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
    target_commit_id: gix::ObjectId,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitSquashResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::SquashCommit),
        perm.read_permission(),
    )
    .ok();

    let res = commit_squash_only_with_perm(ctx, subject_commit_id, target_commit_id, perm);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, perm).ok();
    };
    res
}
