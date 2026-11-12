use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_transaction::with_transaction_with_perm;
use tracing::instrument;

use crate::commit::types::{CommitDiscardResult, MoveChangesResult};

fn unique_subject_commit_ids(
    subject_commit_ids: Vec<gix::ObjectId>,
) -> anyhow::Result<Vec<gix::ObjectId>> {
    let mut seen = gix::hashtable::HashSet::default();
    let subject_commit_ids = subject_commit_ids
        .into_iter()
        .filter(|commit_id| seen.insert(*commit_id))
        .collect::<Vec<_>>();
    if subject_commit_ids.is_empty() {
        anyhow::bail!("no commit IDs provided for discard");
    }
    Ok(subject_commit_ids)
}

/// Discard `subject_commit_ids` and their changes.
///
/// Unlike [`super::uncommit::commit_uncommit()`], the changes don't stay in the
/// workspace.
#[but_api(try_from = crate::commit::json::CommitDiscardResult)]
#[instrument(err(Debug))]
pub fn commit_discard(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    dry_run: DryRun,
) -> anyhow::Result<CommitDiscardResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_discard_with_perm(ctx, subject_commit_ids, dry_run, guard.write_permission())
}

pub fn commit_discard_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitDiscardResult> {
    let subject_commit_ids = unique_subject_commit_ids(subject_commit_ids)?;
    let details = SnapshotDetails::new(OperationKind::DiscardCommit)
        .with_count(subject_commit_ids.len())
        .with_trailers(subject_commit_ids.iter().copied().map(Trailer::Sha));
    let mut meta = ctx.meta()?;
    let workspace =
        with_transaction_with_perm(ctx, &mut meta, perm, details, &[], dry_run, |mut tx| {
            tx.discard_commits(subject_commit_ids.iter().copied())
        })?;

    Ok(CommitDiscardResult {
        discarded_commits: subject_commit_ids,
        workspace,
    })
}

/// Discard `changes` from `commit_id` and from the worktree.
///
/// Unlike [`super::uncommit::commit_uncommit_changes()`], the changes don't stay
/// in the workspace.
#[but_api(try_from = crate::commit::json::MoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_discard_changes(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_discard_changes_with_perm(ctx, commit_id, changes, dry_run, guard.write_permission())
}

pub fn commit_discard_changes_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let details = SnapshotDetails::new(OperationKind::DiscardChanges)
        .with_trailers([Trailer::Sha(commit_id)]);
    let mut meta = ctx.meta()?;
    let workspace =
        with_transaction_with_perm(ctx, &mut meta, perm, details, &[], dry_run, |mut tx| {
            tx.discard_changes_from_commit(commit_id, changes)?;
            Ok(())
        })?;

    Ok(MoveChangesResult { workspace })
}
