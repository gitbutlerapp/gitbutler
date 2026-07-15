use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use gix::bstr::{BStr, BString, ByteSlice as _};
use gix::prelude::ObjectIdExt as _;
use tracing::instrument;

use super::types::CommitCreateResult;

/// Amends the commit at `commit_id` with `changes`.
///
/// See [`but_workspace::commit::commit_amend()`] for lower-level implementation
/// details. When `dry_run` is enabled, the returned workspace previews the
/// amended commit without materializing the rebase.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        dry_run,
        context_lines,
        guard.write_permission(),
    )
}

pub(crate) fn commit_amend_only_impl(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
    context_lines: u32,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let old_worktree_head = repo.head_id()?.detach();
    let content_snapshots = snapshot_worktree_files(&repo, &changes);
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let but_workspace::commit::CommitAmendOutcome {
        rebase,
        commit_selector,
        rejected_specs,
        consumed_specs,
    } = but_workspace::commit::commit_amend(editor, commit_id, changes, context_lines)?;

    let new_commit = commit_selector
        .map(|commit_selector| rebase.lookup_pick(commit_selector))
        .transpose()?;
    if new_commit.is_some() {
        for (name, tip) in rebase.worktree_checkout_tips()? {
            if but_core::Commit::from_id(tip.attach(rebase.repo()))?.is_conflicted() {
                anyhow::bail!(
                    "amending into {commit_id} would conflict on the branch checked out in worktree {name} - aborting without any changes"
                );
            }
        }
    }
    let is_dry_run: bool = dry_run.into();
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;
    if !is_dry_run && new_commit.is_some() && !consumed_specs.is_empty() {
        discard_consumed_changes_if_unchanged(
            &repo,
            old_worktree_head,
            &content_snapshots,
            consumed_specs,
            context_lines,
        )?;
    }

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
}

/// Amend the commit at `commit_id` with `changes` and record an oplog snapshot on success.
///
/// This performs the rewrite under exclusive worktree access and creates a
/// best-effort `AmendCommit` oplog entry if the operation succeeds. When
/// `dry_run` is enabled, the returned workspace previews the amended commit
/// and no oplog entry is persisted. For lower-level implementation details, see
/// [`but_workspace::commit::commit_amend()`].
#[but_api(napi, try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::AmendCommit),
        guard.read_permission(),
        dry_run,
    );

    let res = commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        dry_run,
        context_lines,
        guard.write_permission(),
    );
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, guard.write_permission()).ok();
    }
    res
}

/// The content identity of a worktree file at one point in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileSnapshot {
    Missing,
    Blob(gix::ObjectId),
    /// Never treated as matching anything, not even another unreadable state.
    Unreadable,
}

impl FileSnapshot {
    fn is_verifiable(&self) -> bool {
        !matches!(self, FileSnapshot::Unreadable)
    }
}

pub(crate) fn snapshot_worktree_files(
    repo: &gix::Repository,
    changes: &[DiffSpec],
) -> std::collections::BTreeMap<BString, FileSnapshot> {
    changes
        .iter()
        .flat_map(|spec| {
            std::iter::once(spec.path.as_bstr())
                .chain(spec.previous_path.as_ref().map(|path| path.as_bstr()))
        })
        .map(|path| (path.to_owned(), snapshot_worktree_file(repo, path)))
        .collect()
}

/// Discard committed changes only while the checkout still exactly matches the
/// content from which the commit was created.
pub(crate) fn discard_consumed_changes_if_unchanged(
    repo: &gix::Repository,
    old_head: gix::ObjectId,
    content_snapshots: &std::collections::BTreeMap<BString, FileSnapshot>,
    consumed_specs: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<()> {
    if repo.head_id()?.detach() != old_head {
        return Ok(());
    }

    let (verified_specs, changed_specs): (Vec<_>, Vec<_>) =
        consumed_specs.into_iter().partition(|spec| {
            std::iter::once(spec.path.as_bstr())
                .chain(spec.previous_path.as_ref().map(|path| path.as_bstr()))
                .all(|path| {
                    content_snapshots.get(path).is_some_and(|before| {
                        before.is_verifiable() && *before == snapshot_worktree_file(repo, path)
                    })
                })
        });
    for spec in &changed_specs {
        tracing::warn!(
            path = %spec.path,
            "file content changed while amending - leaving it in the worktree"
        );
    }
    if !verified_specs.is_empty() {
        let dropped =
            but_workspace::discard_workspace_changes(repo, verified_specs, context_lines)?;
        if !dropped.is_empty() {
            tracing::warn!(
                ?dropped,
                "some committed changes no longer matched the worktree state - leaving them in place"
            );
        }
    }
    Ok(())
}

/// Hash the file at worktree-relative `rela_path` without writing an object.
fn snapshot_worktree_file(repo: &gix::Repository, rela_path: &BStr) -> FileSnapshot {
    let Some(workdir) = repo.workdir() else {
        return FileSnapshot::Unreadable;
    };
    let path = workdir.join(gix::path::from_bstr(rela_path));
    let md = match std::fs::symlink_metadata(&path) {
        Ok(md) => md,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return FileSnapshot::Missing,
        Err(_) => return FileSnapshot::Unreadable,
    };
    let bytes = if md.is_symlink() {
        match std::fs::read_link(&path)
            .map_err(anyhow::Error::from)
            .and_then(|target| Ok(gix::path::os_string_into_bstring(target.into())?))
        {
            Ok(target) => Vec::from(target),
            Err(_) => return FileSnapshot::Unreadable,
        }
    } else if md.is_file() {
        match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return FileSnapshot::Unreadable,
        }
    } else {
        return FileSnapshot::Unreadable;
    };
    match gix::objs::compute_hash(repo.object_hash(), gix::object::Kind::Blob, &bytes) {
        Ok(id) => FileSnapshot::Blob(id),
        Err(_) => FileSnapshot::Unreadable,
    }
}
