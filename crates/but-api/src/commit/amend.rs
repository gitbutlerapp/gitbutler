use std::collections::BTreeMap;

use but_api_macros::but_api;
use but_core::{DiffSpec, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use tracing::instrument;

use super::types::CommitCreateResult;

/// Amends an existing commit with selected changes.
#[but_api(crate::commit::json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        context_lines,
        guard.write_permission(),
    )
}

/// Amends an existing commit with selected changes.
pub(crate) fn commit_amend_only_impl(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _, _cache) = ctx.workspace_mut_and_db_and_cache_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let but_workspace::commit::CommitAmendOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_amend(editor, commit_id, changes, context_lines)?;

    let (new_commit, replaced_commits) = match commit_selector {
        Some(commit_selector) => {
            let materialized = rebase.materialize()?;
            let new_commit = materialized.lookup_pick(commit_selector)?;
            let replaced_commits = materialized.history.commit_mappings();
            (Some(new_commit), replaced_commits)
        }
        None => (None, BTreeMap::new()),
    };

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        replaced_commits,
    })
}

/// Amends an existing commit with selected changes, with oplog support.
#[but_api(napi, crate::commit::json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::AmendCommit),
    )
    .ok();

    let mut guard = ctx.exclusive_worktree_access();
    let res = commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        context_lines,
        guard.write_permission(),
    );
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}
