use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use super::types::CommitCreateResult;

/// Creates a commit from `changes` with `message`, inserted on `side` of
/// `relative_to`.
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// commit. For lower-level implementation details, see
/// [`but_workspace::commit::commit_create()`]. When `dry_run` is enabled, the
/// returned workspace previews the inserted commit without materializing the
/// rebase.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_create_only(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    dry_run: DryRun,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        message,
        dry_run,
        context_lines,
        guard.write_permission(),
    )
}

/// Creates and inserts a commit relative to either a commit or a reference.
///
/// When `dry_run` is enabled, the returned workspace previews the inserted
/// commit without materializing the rebase.
#[expect(clippy::too_many_arguments)]
pub(crate) fn commit_create_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    dry_run: DryRun,
    context_lines: u32,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let but_workspace::commit::CommitCreateOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_create(
        editor,
        changes,
        relative_to,
        side,
        &message,
        context_lines,
    )?;

    let new_commit = commit_selector
        .map(|commit_selector| rebase.lookup_pick(commit_selector))
        .transpose()?;
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
}

/// Insert a new commit built from `changes` and record an oplog snapshot on
/// success.
///
/// `relative_to` and `side` choose where the commit is inserted. `message` is
/// the entire commit message text, not just the title. On success, this commits
/// a best-effort `CreateCommit` oplog snapshot using the same lock. When
/// `dry_run` is enabled, the returned workspace previews the inserted commit
/// and no oplog entry is persisted. For lower-level implementation details, see
/// [`but_workspace::commit::commit_create()`].
#[but_api(napi, try_from = crate::commit::json::CommitCreateResult)]
#[instrument(skip_all, fields(relative_to, side, message), err(Debug))]
pub fn commit_create(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateCommit),
        perm.read_permission(),
        dry_run,
    );

    let res = commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        message,
        dry_run,
        context_lines,
        perm,
    );
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
