use crate::commit::json::ChangesSource;
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::{Commit, with_transaction_with_perm};
use but_workspace::commit::ChangeSource;
use gix::bstr::ByteSlice;
use tracing::instrument;

use super::types::CommitCreateResult;

/// Insert a commit made of the `changes` of `changes_source` on `side` of
/// `relative_to`, with `message` as its full message.
///
/// For a linked worktree `changes_source`, undo also covers the branch that
/// worktree has checked out, so it leaves the change uncommitted there.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(skip_all, fields(relative_to, side, message), err(Debug))]
#[expect(clippy::too_many_arguments)]
pub fn commit_create(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    changes_source: ChangesSource,
    message: String,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let worktree_ref = crate::worktrees::changes_source_ref(ctx, &changes_source)?;
    let worktree = crate::worktrees::open_changes_source(ctx, &changes_source)?;
    let mut meta = ctx.meta()?;
    let (created, workspace) = with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::CreateCommit),
        worktree_ref.as_ref().map(|name| name.as_ref()).as_slice(),
        dry_run,
        |mut tx| {
            let source = worktree
                .as_ref()
                .map_or(ChangeSource::Head, |(name, repo)| ChangeSource::Worktree {
                    repo,
                    name: name.as_bstr(),
                });
            Ok(Commit(tx.create_commit(
                relative_to,
                side,
                changes,
                message,
                source,
            )?))
        },
    )?;

    Ok(CommitCreateResult {
        new_commit: created.new_commit.map(|commit| commit.id),
        rejected_specs: created.rejected_specs,
        workspace,
    })
}
