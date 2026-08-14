use std::collections::HashSet;

use but_api_macros::but_api;
use but_core::{DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use crate::WorkspaceState;

use super::types::CommitCherryPickResult;

/// Cherry-picks `source_commit_ids` to `side` of `relative_to`.
///
/// Sources may live anywhere in the repository, including outside the
/// workspace. They are deduplicated and applied in the order given. When
/// `dry_run` is enabled, the returned workspace previews the rewritten graph
/// without materializing it.
#[but_api(try_from = crate::commit::json::CommitCherryPickResult)]
#[instrument(err(Debug))]
pub fn commit_cherry_pick_only(
    ctx: &mut but_ctx::Context,
    source_commit_ids: Vec<gix::ObjectId>,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitCherryPickResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_cherry_pick_only_with_perm(
        ctx,
        source_commit_ids,
        relative_to,
        side,
        dry_run,
        guard.write_permission(),
    )
}

/// Cherry-picks commits under caller-held exclusive repository access without
/// recording an oplog snapshot.
pub fn commit_cherry_pick_only_with_perm(
    ctx: &mut but_ctx::Context,
    source_commit_ids: Vec<gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCherryPickResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let (rebase, inserted_selectors) =
        but_workspace::commit::cherry_pick_commits(editor, source_commit_ids, relative_to, side)?;
    let new_commits = inserted_selectors
        .into_iter()
        .map(|selector| rebase.lookup_pick(selector))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(CommitCherryPickResult {
        new_commits,
        workspace: WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?,
    })
}

/// Cherry-picks `source_commit_ids` to `side` of `relative_to` and records an
/// oplog snapshot on success.
#[but_api(napi, try_from = crate::commit::json::CommitCherryPickResult)]
#[instrument(err(Debug))]
pub fn commit_cherry_pick(
    ctx: &mut but_ctx::Context,
    source_commit_ids: Vec<gix::ObjectId>,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<CommitCherryPickResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_cherry_pick_with_perm(
        ctx,
        source_commit_ids,
        relative_to,
        side,
        dry_run,
        guard.write_permission(),
    )
}

/// Cherry-picks commits under caller-held exclusive repository access and
/// records an oplog snapshot on success.
pub fn commit_cherry_pick_with_perm(
    ctx: &mut but_ctx::Context,
    source_commit_ids: Vec<gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCherryPickResult> {
    let source_commit_count = source_commit_ids.iter().collect::<HashSet<_>>().len();
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CherryPick).with_count(source_commit_count),
        perm.read_permission(),
        dry_run,
    );

    let res =
        commit_cherry_pick_only_with_perm(ctx, source_commit_ids, relative_to, side, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
