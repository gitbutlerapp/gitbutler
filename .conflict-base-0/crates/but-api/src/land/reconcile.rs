//! Reconciling the rest of the workspace after the target moved.
//!
//! This is the graph-shaped successor to the CLI's stack-based reconcile. Rather than asking the
//! legacy `upstream_integration_statuses` for `(StackId, status)` tuples and building
//! `Vec<Resolution { stack_id, .. }>`, it enumerates each applied stack's bottom from the graph
//! workspace ([`but_workspace::RefInfo`]) and marks it for `Rebase`. The integration algorithm in
//! [`but_workspace::integrate_upstream`] then does the classification itself: stacks whose commits
//! already landed upstream (including the branch we just landed) are detected as integrated and
//! dropped, while the rest are rebased onto the moved target. No `StackId` is involved.
//!
//! Because the landed branch is still applied at this point, it is always among the enumerated
//! bottoms, so the algorithm removes it as part of the same pass — matching the CLI's old
//! "delete integrated branches" behavior without a separate `Delete` resolution.

use std::collections::BTreeMap;

use but_core::DryRun;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_workspace::{BottomUpdate, BottomUpdateKind};

use crate::WorkspaceState;

/// The outcome of reconciling after a land.
pub(super) struct Reconciled {
    /// The resulting workspace state.
    pub workspace: WorkspaceState,
    /// `true` when uncommitted worktree changes blocked the rebase, so the remaining branches were
    /// left untouched. The target move already happened, so this is a partial success the caller
    /// should report (run `but pull`), not a failure.
    pub blocked_by_worktree: bool,
}

/// Reconcile the remaining applied branches onto the moved target, reusing the modern graph
/// integration path. Acquires its own exclusive worktree access and records the integration in the
/// oplog (the target move itself is not undoable — see the CLI's undo caveats).
pub(super) fn reconcile_after_land(ctx: &mut but_ctx::Context) -> anyhow::Result<Reconciled> {
    let mut guard = ctx.exclusive_worktree_access();

    let updates = bottom_updates(ctx, guard.write_permission())?;
    if updates.is_empty() {
        return Ok(Reconciled {
            workspace: current_state(ctx, guard.read_permission())?,
            blocked_by_worktree: false,
        });
    }

    // The target move already happened and is irreversible. Materializing the reconcile checks out
    // the new workspace head, which aborts — before moving any refs — when uncommitted changes would
    // be overwritten. Treat that one abort as a partial success (the branches just need a later
    // `but pull`) rather than failing the whole command after the push; any other error is real.
    // The aborted rebase may write unreferenced commit objects, which Git's gc reclaims.
    match crate::workspace::workspace_integrate_upstream_with_perm(
        ctx,
        updates,
        DryRun::No,
        guard.write_permission(),
    ) {
        Ok(outcome) => Ok(Reconciled {
            workspace: outcome.workspace_state,
            blocked_by_worktree: false,
        }),
        Err(err) if is_uncommitted_changes_block(&err) => Ok(Reconciled {
            workspace: current_state(ctx, guard.read_permission())?,
            blocked_by_worktree: true,
        }),
        Err(err) => Err(err),
    }
}

/// Whether the error is the checkout precondition that aborts when uncommitted worktree changes
/// would be overwritten. The integration moves no refs before that point (it checks out before
/// applying ref edits), so it is safe to treat as a deferred reconcile rather than a hard failure.
fn is_uncommitted_changes_block(err: &anyhow::Error) -> bool {
    err.downcast_ref::<but_error::Context>()
        .is_some_and(|ctx| ctx.code == but_error::Code::PreconditionFailed)
}

/// The current workspace state with no commit rewrites, for the paths that don't reconcile. The
/// cache is invalidated first so the state reflects what's actually on disk (the target moved, the
/// branches were not rebased) rather than any in-memory projection a failed integration left behind.
fn current_state(
    ctx: &mut but_ctx::Context,
    perm: &but_core::sync::RepoShared,
) -> anyhow::Result<WorkspaceState> {
    ctx.invalidate_workspace_cache()?;
    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
    WorkspaceState::from_workspace(&ws, &repo, BTreeMap::new())
}

/// Build one `Rebase` update per applied stack, selecting its bottom-most commit (or the bottom
/// segment's reference when that segment carries no commits of its own). Mirrors the frontend's
/// `buildUpstreamIntegrationUpdates`.
fn bottom_updates(
    ctx: &mut but_ctx::Context,
    perm: &mut but_core::sync::RepoExclusive,
) -> anyhow::Result<Vec<BottomUpdate>> {
    let (repo, ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let head_info = but_workspace::graph_to_ref_info(
        &ws,
        &repo,
        but_workspace::ref_info::Options {
            project_meta: ws.graph.project_meta.clone(),
            traversal: but_graph::init::Options::limited(),
            expensive_commit_info: false,
            ..Default::default()
        },
    )?;

    Ok(head_info
        .stacks
        .iter()
        .filter_map(bottom_update_for_stack)
        .collect())
}

/// The bottom update for one stack: rebase its bottom segment onto the target. The selector points
/// at the bottom-most commit when the bottom segment has commits, otherwise at the segment's
/// reference (an empty bottom branch).
fn bottom_update_for_stack(stack: &but_workspace::branch::Stack) -> Option<BottomUpdate> {
    let segment = stack.segments.last()?;
    let selector = match segment.commits.last() {
        Some(commit) => RelativeTo::Commit(commit.id),
        None => RelativeTo::Reference(segment.ref_info.as_ref()?.ref_name.clone()),
    };
    Some(BottomUpdate {
        kind: BottomUpdateKind::Rebase,
        selector,
    })
}
