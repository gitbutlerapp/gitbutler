use std::collections::HashSet;

use anyhow::Context as _;
use but_api_macros::but_api;
use but_core::{
    DryRun,
    sync::{RepoExclusive, RepoShared},
};
use but_hunk_assignment::{HunkAssignmentRequest, HunkAssignmentTarget};
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_transaction::{Commit, with_transaction_with_perm};
use tracing::instrument;

use super::types::{
    MoveChangesResult, UncommitChangesFailure, UncommitChangesFromCommitsResult,
    UncommitChangesSource, UncommitResult,
};

/// Tracks which worktree hunks an uncommit surfaces, so `assign_to` can claim exactly those.
struct SurfacedHunks {
    stack_id: but_core::ref_metadata::StackId,
    before_ids: HashSet<uuid::Uuid>,
}

impl SurfacedHunks {
    /// `None` when there is nothing to assign, and on a dry run - which reports no assignments and
    /// must persist none, so the exchange is skipped outright rather than performed and undone.
    fn record_before(
        ctx: &but_ctx::Context,
        assign_to: Option<but_core::ref_metadata::StackId>,
        dry_run: DryRun,
        perm: &RepoShared,
    ) -> anyhow::Result<Option<Self>> {
        let (Some(stack_id), DryRun::No) = (assign_to, dry_run) else {
            return Ok(None);
        };
        let (repo, workspace, mut db) = ctx.workspace_and_db_mut_with_perm(perm)?;
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &workspace,
            None::<Vec<but_core::TreeChange>>,
            ctx.settings.context_lines,
        )?;
        Ok(Some(Self {
            stack_id,
            before_ids: assignments
                .into_iter()
                .filter_map(|assignment| assignment.id)
                .collect(),
        }))
    }

    /// Assign every hunk that appeared since [`Self::record_before()`] to the target stack.
    fn assign_after(self, ctx: &but_ctx::Context, perm: &RepoShared) -> anyhow::Result<()> {
        let context_lines = ctx.settings.context_lines;
        let (repo, workspace, mut db) = ctx.workspace_and_db_mut_with_perm(perm)?;
        let mut tx = db.transaction()?;
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            tx.hunk_assignments_mut()?,
            &repo,
            &workspace,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;

        let to_assign: Vec<_> = after_assignments
            .into_iter()
            .filter(|assignment| {
                assignment
                    .id
                    .is_some_and(|id| !self.before_ids.contains(&id))
            })
            .map(|assignment| HunkAssignmentRequest {
                hunk_header: assignment.hunk_header,
                path_bytes: assignment.path_bytes,
                target: Some(HunkAssignmentTarget::Stack {
                    stack_id: self.stack_id,
                }),
            })
            .collect();

        but_hunk_assignment::assign(
            tx.hunk_assignments_mut()?,
            &repo,
            &workspace,
            to_assign,
            context_lines,
        )?;
        // Only reached on a real run, so the writes always persist.
        tx.commit()?;
        Ok(())
    }
}

/// Uncommit `subject_commit_ids`, keeping their changes in the worktree.
///
/// With `assign_to`, the hunks the uncommit surfaces are assigned to that stack.
#[but_api(try_from = crate::commit::json::UncommitResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<UncommitResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_with_perm(
        ctx,
        subject_commit_ids,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

pub fn commit_uncommit_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<UncommitResult> {
    if subject_commit_ids.is_empty() {
        anyhow::bail!("no commit IDs provided for uncommit");
    }
    let surfaced = SurfacedHunks::record_before(ctx, assign_to, dry_run, perm.read_permission())?;

    let details = SnapshotDetails::new(OperationKind::UndoCommit)
        .with_count(subject_commit_ids.len())
        .with_trailers(subject_commit_ids.iter().copied().map(Trailer::Sha));
    let mut meta = ctx.meta()?;
    let workspace =
        with_transaction_with_perm(ctx, &mut meta, perm, details, &[], dry_run, |mut tx| {
            tx.uncommit_commits(subject_commit_ids.iter().copied())
                .with_context(|| {
                    format!(
                        "failed to uncommit commits: {}",
                        subject_commit_ids
                            .iter()
                            .map(|id| id.to_hex().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
        })?;

    if let Some(surfaced) = surfaced {
        surfaced.assign_after(ctx, perm.read_permission())?;
    }

    Ok(UncommitResult {
        uncommitted_ids: subject_commit_ids,
        workspace,
    })
}

/// Take `changes` out of `commit_id`, keeping them in the worktree.
///
/// With `assign_to`, the hunks the uncommit surfaces are assigned to that stack.
#[but_api(try_from = crate::commit::json::MoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_with_perm(
        ctx,
        commit_id,
        changes,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

pub fn commit_uncommit_changes_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let surfaced = SurfacedHunks::record_before(ctx, assign_to, dry_run, perm.read_permission())?;

    let mut meta = ctx.meta()?;
    let workspace = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        &[],
        dry_run,
        |mut tx| {
            tx.uncommit_changes_from_commit(commit_id, changes)?;
            Ok(())
        },
    )?;

    if let Some(surfaced) = surfaced {
        surfaced.assign_after(ctx, perm.read_permission())?;
    }

    Ok(MoveChangesResult { workspace })
}

/// Take changes out of several commits, keeping them in the worktree.
///
/// `sources` may be unsorted and name a commit more than once. They're grouped
/// by commit and applied from child to parent, and the groups that can't be
/// applied are reported in the result rather than failing the call. With
/// `assign_to`, the hunks the uncommit surfaces are assigned to that stack.
#[but_api(try_from = crate::commit::json::UncommitChangesFromCommitsResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_from_commits(
    ctx: &mut but_ctx::Context,
    sources: Vec<UncommitChangesSource>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<UncommitChangesFromCommitsResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_from_commits_with_perm(
        ctx,
        sources,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

pub fn commit_uncommit_changes_from_commits_with_perm(
    ctx: &mut but_ctx::Context,
    sources: Vec<UncommitChangesSource>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<UncommitChangesFromCommitsResult> {
    let surfaced = SurfacedHunks::record_before(ctx, assign_to, dry_run, perm.read_permission())?;

    let mut meta = ctx.meta()?;
    let (failures, workspace) = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        &[],
        dry_run,
        |mut tx| {
            let sources =
                sources
                    .into_iter()
                    .map(|source| but_workspace::commit::UncommitChangesSource {
                        commit_id: source.commit_id,
                        changes: source.changes,
                    });
            Ok(Commit(tx.uncommit_changes_from_commits(sources)?))
        },
    )?;

    if let Some(surfaced) = surfaced {
        surfaced.assign_after(ctx, perm.read_permission())?;
    }

    Ok(UncommitChangesFromCommitsResult {
        workspace,
        failures: failures
            .into_iter()
            .map(|failure| UncommitChangesFailure {
                commit_id: failure.commit_id,
                changes: failure.changes,
                error: failure.error,
            })
            .collect(),
    })
}
