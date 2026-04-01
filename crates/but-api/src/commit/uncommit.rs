use std::collections::HashSet;

use but_api_macros::but_api;
use but_hunk_assignment::HunkAssignmentRequest;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::Editor;
use tracing::instrument;

use super::types::MoveChangesResult;

/// Uncommits changes from a commit (removes them from the commit tree) without
/// performing a checkout.
///
/// This acquires exclusive worktree access from `ctx` before extracting the
/// changes.
///
/// See [`commit_uncommit_changes_only_with_perm()`] for details.
#[but_api(crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_only_with_perm(
        ctx,
        commit_id,
        changes,
        assign_to,
        guard.write_permission(),
    )
}

/// Extract `changes` from `commit_id` without performing a checkout, under
/// caller-held exclusive repository access.
///
/// The removed diff stays in the workspace as uncommitted changes. When
/// `assign_to` is set, newly surfaced hunks are reassigned to that stack after
/// the rebase is materialized. For lower-level implementation details, see
/// [`but_workspace::commit::uncommit_changes()`].
pub fn commit_uncommit_changes_only_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    perm: &mut but_ctx::access::RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db, _cache) = ctx.workspace_mut_and_db_mut_and_cache_with_perm(perm)?;

    let before_assignments = if assign_to.is_some() {
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;
        Some(assignments)
    } else {
        None
    };

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome =
        but_workspace::commit::uncommit_changes(editor, commit_id, changes, context_lines)?;

    let materialized = outcome.rebase.materialize_without_checkout()?;

    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            materialized.workspace,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;

        let before_ids: HashSet<_> = before_assignments
            .into_iter()
            .filter_map(|assignment| assignment.id)
            .collect();

        let to_assign: Vec<_> = after_assignments
            .into_iter()
            .filter(|assignment| assignment.id.is_some_and(|id| !before_ids.contains(&id)))
            .map(|assignment| HunkAssignmentRequest {
                hunk_header: assignment.hunk_header,
                path_bytes: assignment.path_bytes,
                stack_id: Some(stack_id),
            })
            .collect();

        but_hunk_assignment::assign(
            db.hunk_assignments_mut()?,
            &repo,
            materialized.workspace,
            to_assign,
            context_lines,
        )?;
    }

    Ok(MoveChangesResult {
        replaced_commits: materialized.history.commit_mappings(),
    })
}

/// Extract `changes` from `commit_id` and record the rewrite in the oplog.
///
/// This acquires exclusive worktree access from `ctx` before extracting the
/// changes.
///
/// See [`commit_uncommit_changes_with_perm()`] for details.
#[but_api(napi, crate::commit::json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_with_perm(ctx, commit_id, changes, assign_to, guard.write_permission())
}

/// Extract `changes` from `commit_id` under caller-held exclusive repository
/// access and record an oplog snapshot on success.
///
/// When `assign_to` is set, newly surfaced hunks are assigned to that stack
/// after the rebase is materialized. This prepares a best-effort
/// `DiscardChanges` oplog snapshot and commits it only if the operation
/// succeeds. For lower-level implementation details, see
/// [`but_workspace::commit::uncommit_changes()`].
pub fn commit_uncommit_changes_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    perm: &mut but_ctx::access::RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        perm.read_permission(),
    )
    .ok();

    let res = commit_uncommit_changes_only_with_perm(ctx, commit_id, changes, assign_to, perm);

    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, perm).ok();
    };

    res
}
