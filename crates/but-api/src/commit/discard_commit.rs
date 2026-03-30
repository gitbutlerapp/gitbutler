use but_api_macros::but_api;
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::Editor;
use tracing::instrument;

use crate::commit::types::CommitDiscardResult;

/// Discards a commit, no snapshots. No strings attached.
///
/// Returns the replaced commits that resulted from the operation.
#[but_api(crate::commit::json::UICommitDiscardResult)]
pub fn commit_discard_only(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
) -> anyhow::Result<CommitDiscardResult> {
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut ws, _, _cache) = ctx.workspace_mut_and_db_and_cache()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let rebase = but_workspace::commit::discard_commit(editor, subject_commit_id)?;

    let materialized = rebase.materialize()?;

    Ok(CommitDiscardResult {
        discarded_commit: subject_commit_id,
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Discards a commit.
///
/// Returns the replaced commits that resulted from the operation.
#[but_api(napi, crate::commit::json::UICommitDiscardResult)]
#[instrument(err(Debug))]
pub fn commit_discard(
    ctx: &mut but_ctx::Context,
    subject_commit_id: gix::ObjectId,
) -> anyhow::Result<CommitDiscardResult> {
    let details = SnapshotDetails::new(OperationKind::DiscardCommit).with_trailers(vec![Trailer {
        key: "sha".to_string(),
        value: subject_commit_id.to_string(),
    }]);
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(ctx, details).ok();

    let res = commit_discard_only(ctx, subject_commit_id);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}
