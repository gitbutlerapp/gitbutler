use bstr::{BString, ByteSlice};
use but_api_macros::but_api;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use tracing::instrument;

/// Rewords a commit, but without updating the oplog.
///
/// Returns the ID of the newly renamed commit
#[but_api]
#[instrument(err(Debug))]
pub fn reword_commit_only(
    ctx: &but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
) -> anyhow::Result<gix::ObjectId> {
    let mut guard = ctx.exclusive_worktree_access();
    let (repo, _, graph) = ctx.graph_and_meta_mut_and_repo_from_head(guard.write_permission())?;

    but_workspace::commit::reword(&graph, &repo, commit_id, message.as_bstr())
}

/// Rewords a commit.
///
/// Returns the ID of the newly renamed commit
#[but_api]
#[instrument(err(Debug))]
pub fn reword_commit(
    ctx: &but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
) -> anyhow::Result<gix::ObjectId> {
    // NOTE: since this is optional by nature, the same would be true if snapshotting/undo would be disabled via `ctx` app settings, for instance.
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::UpdateCommitMessage),
    )
    .ok();

    let res = reword_commit_only(ctx, commit_id, message);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx).ok();
    };
    res
}
