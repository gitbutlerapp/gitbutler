use but_api_macros::but_api;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::GraphExt;
use tracing::instrument;

use super::types::MoveChangesResult;

/// Moves changes between two commits
///
/// Returns where the source and destination commits were mapped to.
#[but_api(crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_move_changes_between_only(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<MoveChangesResult> {
    let context_lines = ctx.settings.context_lines;
    let meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
    let editor = ws.graph.to_editor(&repo)?;

    let outcome = but_workspace::commit::move_changes_between_commits(
        editor,
        source_commit_id,
        destination_commit_id,
        changes,
        context_lines,
    )?;
    let materialized = outcome.rebase.materialize()?;

    ws.refresh_from_head(&repo, &meta)?;

    Ok(MoveChangesResult {
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Moves changes between two commits
///
/// Returns where the source and destination commits were mapped to.
#[but_api(napi, crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_move_changes_between(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<MoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::MoveCommitFile),
    )
    .ok();

    let res =
        commit_move_changes_between_only(ctx, source_commit_id, destination_commit_id, changes);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}
