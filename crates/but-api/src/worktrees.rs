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
/// checkout follows with the consumed changes cancelled out. When the worktree's
/// tip doesn't move (the target lives elsewhere), the consumed changes are
/// discarded from the worktree after the commit and all ref edits are durable -
/// so every failure window leaves a harmless duplicate of the changes, never a
/// loss. Consumed changes that no longer match the worktree's live state at that
/// point are left in place with a warning.
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
    let is_dry_run: bool = dry_run.into();
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    // When the worktree's tip didn't move, its checkout still shows the
    // now-committed changes as uncommitted duplicates - discard them, but only
    // after materialization made the commit and all ref edits durable.
    if !is_dry_run && new_commit.is_some() && !consumed_specs.is_empty() {
        let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, name)?;
        if wt_repo.head_id()?.detach() == old_worktree_head {
            let dropped =
                but_workspace::discard_workspace_changes(&wt_repo, consumed_specs, context_lines)?;
            if !dropped.is_empty() {
                tracing::warn!(
                    worktree = %name,
                    ?dropped,
                    "some committed changes no longer matched the worktree state - leaving them in place"
                );
            }
        }
    }

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
}
