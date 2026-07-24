//! Commands for listing and committing from linked git worktrees (experimental).
//!
//! All commands here are gated on the `featureFlags.worktreeManipulation` setting.
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_ctx::worktrees::WorktreeEntry;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::{
    commit::ChangeSource,
    worktrees::{WorktreeListing, WorktreeSource, open_worktree_repo},
};
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
    active_worktree(ctx, &name)?;
    let repo = ctx.repo.get()?;
    let wt_repo = open_worktree_repo(&repo, BStr::new(name.as_str()))?;
    Ok(but_core::diff::worktree_changes(&wt_repo)?.into())
}

/// Create a commit from `changes` in the linked worktree named `name`, on top of
/// the branch that worktree has checked out.
///
/// The worktree's branch and checkout move together, with the consumed changes
/// cancelled from the checkout. Note that unlike
/// [`commit_create`](crate::commit::create::commit_create), no oplog snapshot is
/// recorded - oplog coverage of linked worktrees is deferred.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn worktree_commit_create(
    ctx: &mut but_ctx::Context,
    name: String,
    changes: Vec<DiffSpec>,
    message: String,
    dry_run: DryRun,
) -> Result<CommitCreateResult> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    let ref_name = active_worktree(ctx, &name)?
        .ref_name
        .with_context(|| format!("Worktree {name} has a detached HEAD"))?;

    let mut meta = ctx.meta()?;
    let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
    let wt_repo = open_worktree_repo(&repo, BStr::new(name.as_str()))?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let but_workspace::commit::CommitCreateOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_create(
        editor,
        changes,
        RelativeTo::Reference(ref_name),
        InsertSide::Below,
        &message,
        context_lines,
        ChangeSource::Worktree {
            repo: &wt_repo,
            name: BStr::new(name.as_str()),
        },
    )?;

    let new_commit = commit_selector
        .map(|commit_selector| rebase.lookup_pick(commit_selector))
        .transpose()?;
    let workspace = WorkspaceState::from_successful_rebase_with_db(rebase, &repo, dry_run, &db)?;

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
}

/// Amend `changes` - uncommitted changes of the linked worktree named `name` -
/// into the commit at `commit_id`, which may live on any workspace stack or on
/// the branch of any active worktree, including `name`'s own.
///
/// The worktree's branch is rebased if the target is in its history, and its
/// checkout follows. Either way the consumed changes are cancelled from that
/// checkout. When the rewrite would leave any linked worktree's branch on a
/// conflict-encoded commit, materialization fails before any ref moves, leaving
/// every checkout untouched.
///
/// Note that unlike [`commit_amend`](crate::commit::amend::commit_amend), no oplog
/// snapshot is recorded - oplog coverage of linked worktrees is deferred. Also,
/// `dry_run` skips materialization, but the previewed commit is still written
/// loose into the shared object database, where it stays unreachable until
/// garbage-collected.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn worktree_commit_amend(
    ctx: &mut but_ctx::Context,
    name: String,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
) -> Result<CommitCreateResult> {
    let mut guard = ctx.exclusive_worktree_access();
    worktree_commit_amend_with_perm(
        ctx,
        name,
        commit_id,
        changes,
        dry_run,
        guard.write_permission(),
    )
}

/// Like [`worktree_commit_amend()`], under caller-held exclusive repository access.
pub fn worktree_commit_amend_with_perm(
    ctx: &mut but_ctx::Context,
    name: String,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> Result<CommitCreateResult> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let context_lines = ctx.settings.context_lines;
    active_worktree(ctx, &name)?;

    let mut meta = ctx.meta()?;
    let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let wt_repo = open_worktree_repo(&repo, BStr::new(name.as_str()))?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let but_workspace::commit::CommitAmendOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_amend(
        editor,
        commit_id,
        changes,
        context_lines,
        ChangeSource::Worktree {
            repo: &wt_repo,
            name: BStr::new(name.as_str()),
        },
    )?;

    let new_commit = commit_selector
        .map(|commit_selector| rebase.lookup_pick(commit_selector))
        .transpose()?;
    let workspace = WorkspaceState::from_successful_rebase_with_db(rebase, &repo, dry_run, &db)?;

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
}
