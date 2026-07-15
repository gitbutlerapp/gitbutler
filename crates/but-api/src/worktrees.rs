//! Commands for listing and managing linked git worktrees (experimental).
//!
//! All commands here are gated on the `featureFlags.worktreeManipulation` setting
//! and are currently CLI-only - they aren't registered with the Tauri or server
//! command surfaces.

use anyhow::{Result, bail};
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun};
use but_rebase::graph_rebase::{Editor, GraphEditorOptions, LookupStep as _};
use but_workspace::worktrees::{WorktreeListing, WorktreeSource};
use gix::bstr::BStr;
use gix::prelude::ObjectIdExt as _;
use tracing::instrument;

use crate::{WorkspaceState, commit::types::CommitCreateResult};

/// Fail unless the user opted into worktree manipulation.
fn ensure_worktree_manipulation_enabled(ctx: &but_ctx::Context) -> Result<()> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        bail!("worktree manipulation is not enabled (featureFlags.worktreeManipulation)");
    }
    Ok(())
}

/// List all usable linked worktrees, split by archived state, with the commits of
/// active worktrees computed against the project target.
///
/// Without a resolvable target the commit lists degrade to empty, see
/// [`but_workspace::worktrees::list_worktrees()`].
#[but_api]
#[instrument(err(Debug))]
pub fn worktrees_list(ctx: &mut but_ctx::Context) -> Result<WorktreeListing> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let _guard = ctx.shared_worktree_access();
    // This reconciles the archived state and must run before any database
    // handle is borrowed.
    let sources: Vec<WorktreeSource> = ctx
        .worktrees_with_state()?
        .into_iter()
        .map(|wt| WorktreeSource {
            archived: wt.archived,
            path: wt.path,
            tip: wt.tip,
        })
        .collect();

    let meta = ctx.project_meta()?;
    let repo = ctx.repo.get()?;
    let target_id = meta.target_commit_id.or_else(|| {
        let target_ref = meta.target_ref.as_ref()?;
        let mut reference = repo.try_find_reference(target_ref.as_ref()).ok()??;
        Some(reference.peel_to_commit().ok()?.id)
    });
    but_workspace::worktrees::list_worktrees(&repo, sources, target_id)
}

/// Persist the archived state of the linked worktree named `name`.
///
/// Archived worktrees are hidden from graph traversal and only minimally listed.
#[but_api]
#[instrument(err(Debug))]
pub fn worktree_set_archived(
    ctx: &mut but_ctx::Context,
    name: String,
    archived: bool,
) -> Result<()> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let _guard = ctx.shared_worktree_access();
    let mut db = ctx.db.get_cache_mut()?;
    but_workspace::worktrees::set_worktree_archived(&mut db, BStr::new(name.as_str()), archived)
}

/// Compute the uncommitted changes in the linked worktree named `name`.
///
/// Unlike [`changes_in_worktree`](crate::diff::changes_in_worktree), which operates
/// on the main worktree, no hunk assignments or dependencies are computed - those
/// are workspace concepts.
#[but_api]
#[instrument(err(Debug))]
pub fn linked_worktree_changes(
    ctx: &mut but_ctx::Context,
    name: String,
) -> Result<but_core::ui::WorktreeChanges> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let _guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    but_workspace::worktrees::worktree_changes_by_name(&repo, BStr::new(name.as_str()))
}

/// Amend `changes` - uncommitted changes of the linked worktree named `name` -
/// into the commit at `commit_id`, which may live on any workspace stack or on
/// the branch of any active worktree (including `name`'s own).
///
/// The worktree's branch is rebased if the target is in its history, and its
/// checkout follows with the consumed changes cancelled out. When the rewrite
/// would leave any linked worktree's branch on a conflict-encoded commit (the
/// amend conflicts with later commits on that branch), this fails before
/// anything is materialized - zero mutation.
///
/// When the worktree's tip doesn't move (the target lives elsewhere), the
/// consumed changes are discarded from the worktree after the commit and all
/// ref edits are durable, and each file is only discarded after re-verifying
/// that its content still matches what was amended - so every failure window
/// leaves a harmless duplicate of the changes, never a loss. Consumed changes
/// whose file content changed in the meantime, or that no longer match the
/// worktree's live state, are left in place with a `tracing` warning only;
/// [`CommitCreateResult`] does not (yet) report which ones were left behind,
/// so callers can merely tell users that this may happen.
///
/// Note that unlike [`commit_amend`](crate::commit::amend::commit_amend), no
/// oplog snapshot is recorded yet - oplog coverage of linked worktrees is
/// deferred. Also, `dry_run` skips materialization and the discard, but the
/// previewed commit is still written loose into the shared object database,
/// where it stays unreachable until garbage-collected.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn worktree_commit_amend(
    ctx: &mut but_ctx::Context,
    name: String,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
) -> Result<CommitCreateResult> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;

    let name = BStr::new(name.as_str());
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, name)?;
    // Captured before any mutation: an unchanged tip afterwards means the
    // worktree's checkout didn't participate in the rewrite.
    let old_worktree_head = wt_repo.head_id()?.detach();
    // Content identity of every requested file, captured before the amend so
    // the discard fallback below can verify nothing wrote to them in between
    // (editor autosave, formatters, watchers - the amend takes long enough for
    // that race to be real).
    let content_snapshots: std::collections::BTreeMap<_, _> = changes
        .iter()
        .map(|spec| {
            (
                spec.path.clone(),
                snapshot_worktree_file(&wt_repo, spec.path.as_ref()),
            )
        })
        .collect();

    // Worktree branches are seeded into the graph as extra tips but aren't
    // reachable from `HEAD`, which leaves them immutable by default - without
    // forcing them mutable, an amend into one would rewrite the step graph
    // while the branch ref silently never moves. Untouched branches keep their
    // commit ids, so the all-tips superset is safe.
    let extra_mutable_refs = ws
        .graph
        .options
        .worktree_tips
        .iter()
        .filter_map(|tip| tip.ref_name.clone())
        .collect();
    let editor = Editor::create_with_opts(
        &mut ws,
        &mut meta,
        &repo,
        &GraphEditorOptions {
            extra_mutable_refs,
            ..Default::default()
        },
    )?;

    let but_workspace::commit::CommitAmendOutcome {
        rebase,
        commit_selector,
        rejected_specs,
        consumed_specs,
    } = but_workspace::commit::commit_amend_from_worktree(
        editor,
        commit_id,
        changes,
        context_lines,
        &wt_repo,
        name,
    )?;

    let new_commit = commit_selector
        .map(|commit_selector| rebase.lookup_pick(commit_selector))
        .transpose()?;

    // Refuse to leave any linked worktree's branch on a conflict-encoded
    // commit: its checkout would be skipped (conflicted trees are never
    // written into plain worktrees) while the ref still moves, silently
    // stranding a stale checkout on a GitButler-internal commit. Nothing has
    // been materialized yet, so bailing here mutates nothing.
    if new_commit.is_some() {
        for (wt_name, tip) in rebase.worktree_checkout_tips()? {
            if but_core::Commit::from_id(tip.attach(rebase.repo()))?.is_conflicted() {
                bail!(
                    "amending into {commit_id} would conflict with later commits on the branch \
                     checked out in worktree {wt_name} - aborting without any changes"
                );
            }
        }
    }
    if let Some(new_commit) = new_commit
        && but_core::Commit::from_id(new_commit.attach(rebase.repo()))?.is_conflicted()
    {
        bail!(
            "amending into {commit_id} would produce a conflicted commit - aborting without any changes"
        );
    }

    let is_dry_run: bool = dry_run.into();
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    // When the worktree's tip didn't move, its checkout still shows the
    // now-committed changes as uncommitted duplicates - discard them, but only
    // after materialization made the commit and all ref edits durable.
    if !is_dry_run && new_commit.is_some() && !consumed_specs.is_empty() {
        let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, name)?;
        if wt_repo.head_id()?.detach() == old_worktree_head {
            // Only discard files whose content still matches the pre-amend
            // snapshot - anything written in between is in neither the amended
            // commit nor the snapshot, so discarding it would destroy it.
            let (verified_specs, changed_specs): (Vec<_>, Vec<_>) =
                consumed_specs.into_iter().partition(|spec| {
                    content_snapshots.get(&spec.path).is_some_and(|before| {
                        before.is_verifiable()
                            && *before == snapshot_worktree_file(&wt_repo, spec.path.as_ref())
                    })
                });
            for spec in &changed_specs {
                tracing::warn!(
                    worktree = %name,
                    path = %spec.path,
                    "file content changed while amending - leaving it in the worktree"
                );
            }
            if !verified_specs.is_empty() {
                let dropped = but_workspace::discard_workspace_changes(
                    &wt_repo,
                    verified_specs,
                    context_lines,
                )?;
                if !dropped.is_empty() {
                    tracing::warn!(
                        worktree = %name,
                        ?dropped,
                        "some committed changes no longer matched the worktree state - leaving them in place"
                    );
                }
            }
        }
    }

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
}

/// The content identity of a worktree file at one point in time, used to
/// verify it didn't change between two reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSnapshot {
    /// Nothing exists at the path.
    Missing,
    /// A regular file or symlink whose content hashes to this blob id
    /// (symlinks hash their target path, like Git does).
    Blob(gix::ObjectId),
    /// The path exists but could not be read, or isn't a file/symlink.
    /// Never treated as matching anything, not even another unreadable state.
    Unreadable,
}

impl FileSnapshot {
    /// Whether this snapshot pins down actual content that a later read can be
    /// compared against.
    fn is_verifiable(&self) -> bool {
        !matches!(self, FileSnapshot::Unreadable)
    }
}

/// Hash the file at worktree-relative `rela_path` in `wt_repo`'s working
/// directory as a git blob, without writing any object.
///
/// The raw on-disk bytes are hashed (no filters applied) - the result is only
/// meant to be compared against another snapshot taken the same way.
fn snapshot_worktree_file(wt_repo: &gix::Repository, rela_path: &BStr) -> FileSnapshot {
    let Some(workdir) = wt_repo.workdir() else {
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
    match gix::objs::compute_hash(wt_repo.object_hash(), gix::object::Kind::Blob, &bytes) {
        Ok(id) => FileSnapshot::Blob(id),
        Err(_) => FileSnapshot::Unreadable,
    }
}
