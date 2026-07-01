use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use but_workspace::commit::{SquashCommitsOutcome, squash_commits::MessageCombinationStrategy};
use tracing::instrument;

use crate::WorkspaceState;

use super::types::CommitSquashResult;

/// Squash `subject_commit_ids` into `target_commit_id`.
///
/// This acquires exclusive worktree access from `ctx` before rewriting the
/// commits.
///
/// When `dry_run` is enabled, the returned workspace previews the squashed
/// result without materializing the rebase. For details, see
/// [`commit_squash_only_with_perm()`].
#[but_api(try_from = crate::commit::json::CommitSquashResult)]
#[instrument(err(Debug))]
pub fn commit_squash_only(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    target_commit_id: gix::ObjectId,
    how_to_combine_messages: MessageCombinationStrategy,
    dry_run: DryRun,
) -> anyhow::Result<CommitSquashResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_squash_only_with_perm(
        ctx,
        subject_commit_ids,
        target_commit_id,
        how_to_combine_messages,
        dry_run,
        guard.write_permission(),
    )
}

/// Squash `subject_commit_ids` into `target_commit_id` under caller-held
/// exclusive repository access.
///
/// This variant does not create an oplog entry. When `dry_run` is enabled, it
/// returns a preview of the resulting workspace state without materializing the rebases.
pub fn commit_squash_only_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    target_commit_id: gix::ObjectId,
    how_to_combine_messages: MessageCombinationStrategy,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitSquashResult> {
    if subject_commit_ids.is_empty() {
        anyhow::bail!("No commits were provided to squash")
    }

    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let SquashCommitsOutcome {
        rebase,
        commit_selector,
    } = but_workspace::commit::squash_commits(
        editor,
        subject_commit_ids,
        target_commit_id,
        how_to_combine_messages,
    )?;
    let new_commit = rebase.lookup_pick(commit_selector)?;
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    Ok(CommitSquashResult {
        new_commit,
        workspace,
    })
}

/// Squash `subject_commit_ids` into `target_commit_id` and record an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before rewriting the
/// commits.
///
/// When `dry_run` is enabled, the returned workspace previews the squashed
/// result and no oplog entry is persisted. For details, see [`commit_squash_with_perm()`].
#[but_api(napi, try_from = crate::commit::json::CommitSquashResult)]
#[instrument(err(Debug))]
pub fn commit_squash(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    target_commit_id: gix::ObjectId,
    how_to_combine_messages: MessageCombinationStrategy,
    dry_run: DryRun,
) -> anyhow::Result<CommitSquashResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_squash_with_perm(
        ctx,
        subject_commit_ids,
        target_commit_id,
        how_to_combine_messages,
        dry_run,
        guard.write_permission(),
    )
}

/// Squash `subject_commit_ids` into `target_commit_id` under caller-held
/// exclusive repository access and record an oplog snapshot on success.
///
/// It prepares a best-effort `SquashCommit` oplog snapshot, performs the
/// squashes, and commits the snapshot only if the operation succeeds. When
/// `dry_run` is enabled, it returns a preview of the resulting workspace state
/// and skips oplog persistence.
pub fn commit_squash_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    target_commit_id: gix::ObjectId,
    how_to_combine_messages: MessageCombinationStrategy,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitSquashResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::SquashCommit),
        perm.read_permission(),
        dry_run,
    );

    let res = commit_squash_only_with_perm(
        ctx,
        subject_commit_ids,
        target_commit_id,
        how_to_combine_messages,
        dry_run,
        perm,
    );
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
