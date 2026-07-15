//! Commands for listing and managing linked git worktrees (experimental).
//!
//! All commands here are gated on the `featureFlags.worktreeManipulation` setting
//! and are currently CLI-only - they aren't registered with the Tauri or server
//! command surfaces.

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::worktrees::{WorktreeListing, WorktreeSource};
use gix::bstr::BStr;
use gix::prelude::ObjectIdExt as _;
use tracing::instrument;

use crate::{
    WorkspaceState,
    commit::{
        amend::{discard_consumed_changes_if_unchanged, snapshot_worktree_files},
        types::CommitCreateResult,
    },
};

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

/// Create a commit from `changes` in the active linked worktree named `name`.
///
/// The worktree must have a symbolic `HEAD`. Its branch and checkout move
/// together, while the consumed changes are cancelled from the checkout.
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
    let changes = but_workspace::flatten_diff_specs(changes);
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    let worktree = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.tip.name == name.as_bytes())
        .with_context(|| format!("Worktree {name} does not exist"))?;
    if worktree.archived {
        bail!("Worktree {name} is archived");
    }
    let ref_name = worktree
        .tip
        .ref_name
        .with_context(|| format!("Worktree {name} has a detached HEAD"))?;

    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
    let worktree_name = BStr::new(name.as_str());
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, worktree_name)?;
    let mut head = wt_repo.head()?;
    let current_ref = head
        .referent_name()
        .with_context(|| format!("Worktree {name} has a detached HEAD"))?;
    if current_ref != ref_name.as_ref() {
        bail!(
            "Worktree {name} changed branches from {} to {}",
            ref_name.shorten(),
            current_ref.shorten()
        );
    }
    if head.peel_to_commit()?.id != worktree.tip.id {
        bail!("Worktree {name} changed commits while preparing the commit");
    }

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let but_workspace::commit::CommitCreateOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_create_from_worktree(
        editor,
        changes,
        RelativeTo::Reference(ref_name),
        InsertSide::Below,
        &message,
        context_lines,
        &wt_repo,
        worktree_name,
    )?;

    let new_commit = commit_selector
        .map(|selector| rebase.lookup_pick(selector))
        .transpose()?;
    if new_commit.is_some() {
        for (name, tip) in rebase.worktree_checkout_tips()? {
            if but_core::Commit::from_id(tip.attach(rebase.repo()))?.is_conflicted() {
                bail!(
                    "creating the commit would conflict on the branch checked out in worktree {name} - aborting without any changes"
                );
            }
        }
    }
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;
    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
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
    worktree_commit_amend_inner(ctx, name, commit_id, changes, None, dry_run, perm, false)
}

/// Amend only if every linked-worktree change can be consumed, optionally
/// replacing the target message in the same rewrite.
pub fn worktree_commit_amend_all(
    ctx: &mut but_ctx::Context,
    name: String,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    new_message: Option<String>,
    dry_run: DryRun,
) -> Result<CommitCreateResult> {
    let mut guard = ctx.exclusive_worktree_access();
    worktree_commit_amend_all_with_perm(
        ctx,
        name,
        commit_id,
        changes,
        new_message,
        dry_run,
        guard.write_permission(),
    )
}

/// Like [`worktree_commit_amend_all()`], under caller-held exclusive access.
pub fn worktree_commit_amend_all_with_perm(
    ctx: &mut but_ctx::Context,
    name: String,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    new_message: Option<String>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> Result<CommitCreateResult> {
    worktree_commit_amend_inner(
        ctx,
        name,
        commit_id,
        changes,
        new_message,
        dry_run,
        perm,
        true,
    )
}

fn worktree_commit_amend_inner(
    ctx: &mut but_ctx::Context,
    name: String,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    new_message: Option<String>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
    require_all: bool,
) -> Result<CommitCreateResult> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let changes = but_workspace::flatten_diff_specs(changes);
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;

    let name = BStr::new(name.as_str());
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, name)?;
    // Captured before any mutation: an unchanged tip afterwards means the
    // worktree's checkout didn't participate in the rewrite.
    let old_worktree_head = wt_repo.head_id()?.detach();
    // Content identity of every requested file, captured before the amend so
    // the discard fallback below can verify nothing wrote to them in between
    // (editor autosave, formatters, watchers - the amend takes long enough for
    // that race to be real).
    let content_snapshots = snapshot_worktree_files(&wt_repo, &changes);

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let but_workspace::commit::CommitAmendOutcome {
        rebase,
        commit_selector,
        rejected_specs,
        consumed_specs,
    } = but_workspace::commit::commit_amend_from_worktree_with_message(
        editor,
        commit_id,
        changes,
        context_lines,
        &wt_repo,
        name,
        new_message,
    )?;

    if require_all {
        if !rejected_specs.is_empty() {
            bail!("Couldn't amend all linked-worktree changes");
        }
        if commit_selector.is_none() {
            bail!("No linked-worktree changes could be amended");
        }
    }

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
        discard_consumed_changes_if_unchanged(
            &wt_repo,
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
