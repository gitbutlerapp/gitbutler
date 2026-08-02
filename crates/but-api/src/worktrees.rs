//! Commands for listing linked git worktrees (experimental), and the resolution
//! of a [`ChangesSource`](crate::commit::json::ChangesSource) for the commands
//! that can commit from one.
//!
//! All commands here are gated on the `featureFlags.worktreeManipulation` setting.
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use but_ctx::worktrees::WorktreeEntry;
use but_workspace::worktrees::{WorktreeListing, WorktreeSource, open_worktree_repo};
use gix::bstr::{BStr, BString, ByteSlice};
use tracing::instrument;

use crate::commit::json::ChangesSource;

/// Fail unless the user opted into worktree manipulation.
fn ensure_worktree_manipulation_enabled(ctx: &but_ctx::Context) -> Result<()> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        bail!("worktree manipulation is not enabled (featureFlags.worktreeManipulation)");
    }
    Ok(())
}

/// Look up the *active* linked worktree named `name`.
///
/// Every command here operates on active worktrees only - an archived one is
/// hidden from the graph, so operations against it could not be materialized.
///
/// Must not be called while a database handle is borrowed, see
/// [`but_ctx::Context::worktrees_with_state()`].
fn active_worktree(ctx: &but_ctx::Context, name: &str) -> Result<WorktreeEntry> {
    let worktree = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.name == name.as_bytes())
        .with_context(|| format!("Worktree {name} does not exist"))?;
    if worktree.archived {
        bail!("Worktree {name} is archived");
    }
    Ok(worktree)
}

/// Open the checkout that `source` reads its changes from, returning its stable
/// name along with a plain from-disk open of it, or `None` for the main worktree.
///
/// Callers turn this into a [`ChangeSource`](but_workspace::commit::ChangeSource)
/// for the duration of an editor-backed operation.
///
/// Must not be called while a database handle is borrowed, see
/// [`but_ctx::Context::worktrees_with_state()`].
pub(crate) fn open_changes_source(
    ctx: &but_ctx::Context,
    source: &ChangesSource,
) -> Result<Option<(BString, gix::Repository)>> {
    let ChangesSource::Worktree(name) = source else {
        return Ok(None);
    };
    ensure_worktree_manipulation_enabled(ctx)?;
    let name = active_worktree(ctx, name)?.name;
    let repo = ctx.repo.get()?;
    let wt_repo = open_worktree_repo(&repo, name.as_bstr())?;
    Ok(Some((name, wt_repo)))
}

/// List all usable linked worktrees, split by archived state.
#[but_api]
#[instrument(err(Debug))]
pub fn worktrees_list(ctx: &mut but_ctx::Context) -> Result<WorktreeListing> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let _guard = ctx.shared_worktree_access();
    // This reconciles the archived state and must run before any database
    // handle is borrowed.
    let sources = ctx
        .worktrees_with_state()?
        .into_iter()
        .map(|worktree| WorktreeSource {
            archived: worktree.archived,
            path: worktree.path,
            name: worktree.name,
            ref_name: worktree.ref_name,
            head: worktree.head,
        })
        .collect();
    Ok(but_workspace::worktrees::list_worktrees(sources))
}

/// Persist the archived state of the linked worktree named `name`.
///
/// Archived worktrees are hidden from graph traversal and only minimally listed,
/// which is how projects that predate GitButler's worktree support avoid showing
/// every worktree ever created.
#[but_api]
#[instrument(err(Debug))]
pub fn worktree_set_archived(
    ctx: &mut but_ctx::Context,
    name: String,
    archived: bool,
) -> Result<()> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let _guard = ctx.shared_worktree_access();
    ctx.set_worktree_archived(BStr::new(name.as_str()), archived)
}
