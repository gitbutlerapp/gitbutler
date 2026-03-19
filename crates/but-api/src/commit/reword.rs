use bstr::{BString, ByteSlice};
use but_api_macros::but_api;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{GraphExt, LookupStep as _};
use tracing::instrument;

use super::types::CommitRewordResult;

/// Rewords a commit.
///
/// Returns the result including the new commit ID and any replaced commits.
#[but_api(crate::commit::json::UICommitRewordResult)]
#[instrument(err(Debug))]
pub fn commit_reword_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
) -> anyhow::Result<CommitRewordResult> {
    let (_guard, repo, ws, _, _cache) = ctx.workspace_and_db_and_cache()?;
    let editor = ws.graph.to_editor(&repo)?;

    let (outcome, edited_commit_selector) =
        but_workspace::commit::reword(editor, commit_id, message.as_bstr())?;

    let outcome = outcome.materialize()?;
    let id = outcome.lookup_pick(edited_commit_selector)?;
    let replaced_commits = outcome.history.commit_mappings();

    Ok(CommitRewordResult {
        new_commit: id,
        replaced_commits,
    })
}

/// Rewords a commit, with oplog support.
///
/// Returns the result including the new commit ID and any replaced commits.
#[but_api(napi, crate::commit::json::UICommitRewordResult)]
#[instrument(err(Debug))]
pub fn commit_reword(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
) -> anyhow::Result<CommitRewordResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::UpdateCommitMessage),
    )
    .ok();

    let res = commit_reword_only(ctx, commit_id, message);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}
