use crate::WorkspaceState;
use bstr::{BString, ByteSlice};
use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use tracing::instrument;

use super::types::CommitRewordResult;

/// Replace the title and message of `commit_id` with `message`.
///
/// This acquires exclusive worktree access from `ctx` before rewriting the
/// commit message.
///
/// `message` must be the full commit message payload: the title line, and when a
/// body is present, `\n\n` followed by the body.
/// When `dry_run` is enabled, the returned workspace previews the rewritten
/// commit message without materializing the rebase. See
/// [`commit_reword_only_with_perm()`] for details.
#[but_api(try_from = crate::commit::json::CommitRewordResult)]
#[instrument(err(Debug))]
pub fn commit_reword_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
    dry_run: DryRun,
) -> anyhow::Result<CommitRewordResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_reword_only_with_perm(ctx, commit_id, message, dry_run, guard.write_permission())
}

/// Replace the stored message of `commit_id` under caller-held exclusive
/// repository access.
///
/// It materializes the reword rebase and returns the new commit id plus the
/// replaced-commit mapping. When `dry_run` is enabled, it returns a preview of
/// the resulting workspace state without materializing the rebase. This
/// variant does not create an oplog entry. For lower-level implementation
/// details, see [`but_workspace::commit::reword()`].
pub fn commit_reword_only_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitRewordResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let (rebase, edited_commit_selector) =
        but_workspace::commit::reword(editor, commit_id, message.as_bstr())?;
    let new_commit = rebase.lookup_pick(edited_commit_selector)?;
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    Ok(CommitRewordResult {
        new_commit,
        workspace,
    })
}

/// Reword `commit_id` to `message` using the behavior described by
/// [`commit_reword_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx` before rewriting the
/// commit message and recording the oplog entry. When `dry_run` is enabled,
/// the returned workspace previews the rewritten message and no oplog entry is
/// persisted.
#[but_api(napi, try_from = crate::commit::json::CommitRewordResult)]
#[instrument(err(Debug))]
pub fn commit_reword(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
    dry_run: DryRun,
) -> anyhow::Result<CommitRewordResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_reword_with_perm(ctx, commit_id, message, dry_run, guard.write_permission())
}

/// Rewords `commit_id` to `message` under caller-held exclusive repository
/// access and records an oplog snapshot on success.
///
/// It prepares a best-effort `UpdateCommitMessage` oplog snapshot, performs
/// the reword, and commits the snapshot only if the operation succeeds. When
/// `dry_run` is enabled, it returns a preview of the resulting workspace state
/// and skips oplog persistence. For lower-level implementation details, see
/// [`but_workspace::commit::reword()`].
pub fn commit_reword_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitRewordResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::UpdateCommitMessage),
        perm.read_permission(),
        dry_run,
    );

    let res = commit_reword_only_with_perm(ctx, commit_id, message, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
