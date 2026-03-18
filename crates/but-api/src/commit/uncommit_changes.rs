use std::collections::HashSet;

use but_api_macros::but_api;
use but_hunk_assignment::HunkAssignmentRequest;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::GraphExt as _;
use tracing::instrument;

use crate::commit::types::MoveChangesResult;

/// Uncommits changes from a commit (removes them from the commit tree) without
/// performing a checkout.
///
/// This has the practical effect of leaving the changes that were in the commit
/// as uncommitted changes in the worktree.
///
/// If `assign_to` is provided, the newly uncommitted changes will be assigned
/// to the specified stack.
#[but_api(crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<MoveChangesResult> {
    let context_lines = ctx.settings.context_lines;
    let meta = ctx.meta()?;
    let (_guard, repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut()?;

    let before_assignments = if assign_to.is_some() {
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
            context_lines,
        )?;
        Some(assignments)
    } else {
        None
    };

    let editor = ws.graph.to_editor(&repo)?;
    let outcome =
        but_workspace::commit::uncommit_changes(editor, commit_id, changes, context_lines)?;

    let materialized = outcome.rebase.materialize_without_checkout()?;

    ws.refresh_from_head(&repo, &meta)?;
    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
            context_lines,
        )?;

        let before_ids: HashSet<_> = before_assignments
            .into_iter()
            .filter_map(|a| a.id)
            .collect();

        let to_assign: Vec<_> = after_assignments
            .into_iter()
            .filter(|a| a.id.is_some_and(|id| !before_ids.contains(&id)))
            .map(|a| HunkAssignmentRequest {
                hunk_header: a.hunk_header,
                path_bytes: a.path_bytes,
                stack_id: Some(stack_id),
            })
            .collect();

        but_hunk_assignment::assign(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            to_assign,
            None,
            context_lines,
        )?;
    }

    Ok(MoveChangesResult {
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Uncommits changes from a commit, with oplog and optional assign_to support
///
/// If `assign_to` is provided, the newly uncommitted changes will be assigned
/// to the specified stack.
#[but_api(napi, crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<MoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
    )
    .ok();

    let res = commit_uncommit_changes_only(ctx, commit_id, changes, assign_to);

    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };

    res
}
