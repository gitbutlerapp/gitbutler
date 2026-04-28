use anyhow::bail;
use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use std::time::Instant;
use tracing::instrument;

use crate::WorkspaceState;

use super::types::CommitMoveResult;

/// Moves `subject_commit_ids` to `side` of `relative_to`.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// commit.
///
/// When `dry_run` is enabled, the returned workspace previews the moved commit
/// without materializing the rebase. For details, see
/// [`commit_move_only_with_perm()`].
#[but_api(try_from = crate::commit::json::CommitMoveResult)]
pub fn commit_move_only(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitMoveResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_move_only_with_perm(
        ctx,
        subject_commit_ids,
        relative_to,
        side,
        dry_run,
        guard.write_permission(),
    )
}

/// Move `subject_commit_ids` to the `side` of `relative_to` under
/// caller-held exclusive repository access.
///
/// This returns the post-operation workspace view without creating an oplog
/// entry. When `dry_run` is enabled, it returns a preview of the resulting
/// workspace state without materializing the rebase. For lower-level
/// implementation details, see [`but_workspace::commit::move_commit()`].
pub fn commit_move_only_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitMoveResult> {
    let operation_started = Instant::now();
    if subject_commit_ids.is_empty() {
        bail!("No commits were provided to move")
    }

    tracing::warn!(
        subject_commit_count = subject_commit_ids.len(),
        ?side,
        ?dry_run,
        "commit_move_only_with_perm: start"
    );

    let setup_started = Instant::now();
    let mut meta = ctx.meta()?;
    let use_local_only = ctx.settings.feature_flags.mutation_workspace_local_only;
    let (repo, mut ws) = if use_local_only {
        ctx.workspace_mut_with_perm_mutation_local_only(perm)?
    } else {
        ctx.workspace_mut_with_perm(perm)?
    };
    println!(
        "commit_move_only_with_perm: context prepared {:?}",
        setup_started.elapsed()
    );
    tracing::warn!(
        use_local_only,
        "commit_move_only_with_perm: selected mutation workspace accessor"
    );
    let creating_editor = Instant::now();
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    println!(
        "commit_move_only_with_perm: editor prepared {:?}",
        creating_editor.elapsed()
    );

    let ordering_started = Instant::now();
    let ordered_selectors = editor.order_commit_selectors_by_parentage(subject_commit_ids)?;
    let mut ordered_ids = ordered_selectors
        .iter()
        .map(|selector| editor.lookup_pick(*selector))
        .collect::<anyhow::Result<Vec<_>>>()?;
    tracing::warn!(
        ordered_commit_count = ordered_ids.len(),
        elapsed = ?ordering_started.elapsed(),
        "commit_move_only_with_perm: commit ordering resolved"
    );

    let ordered_ids = if matches!(side, InsertSide::Above) {
        ordered_ids.reverse();
        ordered_ids
    } else {
        ordered_ids
    };

    let mut subjects = ordered_ids.into_iter();
    let first_subject = subjects
        .next()
        .expect("non-empty commit list always has a first subject");

    let first_rebase_started = Instant::now();
    let mut rebase =
        but_workspace::commit::move_commit(editor, first_subject, relative_to.clone(), side)?;
    tracing::warn!(
        elapsed = ?first_rebase_started.elapsed(),
        "commit_move_only_with_perm: first commit move completed"
    );

    let loop_started = Instant::now();
    let mut moved_in_loop = 0usize;
    for original_subject_id in subjects {
        let remapped_ids = rebase.history.commit_mappings();
        let subject_id = remapped_ids
            .get(&original_subject_id)
            .copied()
            .unwrap_or(original_subject_id);

        let remapped_relative_to = match &relative_to {
            RelativeTo::Commit(target_commit_id) => RelativeTo::Commit(
                remapped_ids
                    .get(target_commit_id)
                    .copied()
                    .unwrap_or(*target_commit_id),
            ),
            RelativeTo::Reference(reference_name) => RelativeTo::Reference(reference_name.clone()),
        };

        rebase = but_workspace::commit::move_commit(
            rebase.into_editor(),
            subject_id,
            remapped_relative_to,
            side,
        )?;
        moved_in_loop += 1;
    }
    tracing::warn!(
        moved_in_loop,
        elapsed = ?loop_started.elapsed(),
        "commit_move_only_with_perm: remaining commit moves completed"
    );

    let workspace_started = Instant::now();
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;
    tracing::warn!(
        elapsed = ?workspace_started.elapsed(),
        total_elapsed = ?operation_started.elapsed(),
        "commit_move_only_with_perm: workspace state created"
    );

    Ok(CommitMoveResult { workspace })
}

/// Moves `subject_commit_ids` to `side` of `relative_to` and records an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before moving the
/// commit.
///
/// When `dry_run` is enabled, the returned workspace previews the moved commit
/// and no oplog entry is persisted. For details, see [`commit_move_with_perm()`].
#[but_api(napi, try_from = crate::commit::json::CommitMoveResult)]
#[instrument(err(Debug))]
pub fn commit_move(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitMoveResult> {
    let operation_started = Instant::now();
    let lock_wait_started = Instant::now();
    let mut guard = ctx.exclusive_worktree_access();
    tracing::warn!(
        elapsed = ?lock_wait_started.elapsed(),
        "commit_move: acquired exclusive worktree access"
    );

    let move_started = Instant::now();
    let res = commit_move_with_perm(
        ctx,
        subject_commit_ids,
        relative_to,
        side,
        dry_run,
        guard.write_permission(),
    );
    tracing::warn!(
        elapsed = ?move_started.elapsed(),
        total_elapsed = ?operation_started.elapsed(),
        success = res.is_ok(),
        "commit_move: finished"
    );
    res
}

/// Moves `subject_commit_ids` to `side` of `relative_to` under caller-held
/// exclusive repository access and records an oplog snapshot on success.
///
/// It prepares a best-effort `MoveCommit` oplog snapshot, performs the move,
/// and commits the snapshot only if the operation succeeds. When `dry_run` is
/// enabled, it returns a preview of the resulting workspace state and skips
/// oplog persistence. For lower-level implementation details, see
/// [`but_workspace::commit::move_commit()`].
pub fn commit_move_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitMoveResult> {
    let operation_started = Instant::now();
    let snapshot_started = Instant::now();
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::MoveCommit).with_count(subject_commit_ids.len()),
        perm.read_permission(),
        dry_run,
    );
    tracing::warn!(
        has_snapshot = maybe_oplog_entry.is_some(),
        elapsed = ?snapshot_started.elapsed(),
        "commit_move_with_perm: prepared oplog snapshot"
    );

    let move_started = Instant::now();
    let res = commit_move_only_with_perm(ctx, subject_commit_ids, relative_to, side, dry_run, perm);
    tracing::warn!(
        elapsed = ?move_started.elapsed(),
        success = res.is_ok(),
        "commit_move_with_perm: move operation completed"
    );

    let commit_snapshot_started = Instant::now();
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
        tracing::warn!(
            elapsed = ?commit_snapshot_started.elapsed(),
            "commit_move_with_perm: committed oplog snapshot"
        );
    }
    tracing::warn!(
        total_elapsed = ?operation_started.elapsed(),
        "commit_move_with_perm: finished"
    );
    res
}
