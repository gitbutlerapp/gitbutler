//! Commands for listing and managing linked git worktrees (experimental).
//!
//! All commands here are gated on the `featureFlags.worktreeManipulation` setting
//! and are currently CLI-only - they aren't registered with the Tauri or server
//! command surfaces.

use anyhow::{Result, bail};
use but_api_macros::but_api;
use but_workspace::worktrees::{WorktreeListing, WorktreeSource};
use gix::bstr::BStr;
use tracing::instrument;

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
