use std::collections::{BTreeMap, HashSet};

use crate::WorkspaceState;
use anyhow::{Context as _, bail};
use but_api_macros::but_api;
use but_core::{DryRun, RepositoryExt, sync::RepoExclusive};
use but_hunk_assignment::{HunkAssignmentRequest, HunkAssignmentTarget};
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::Editor;
use gix::prelude::ObjectIdExt as _;
use tracing::instrument;

use super::types::{
    MoveChangesResult, UncommitChangesFailure, UncommitChangesFromCommitsResult,
    UncommitChangesSource, UncommitResult,
};

#[derive(Debug, Clone, Copy)]
struct WorktreeMaterialization {
    before: gix::ObjectId,
    after: gix::ObjectId,
}

/// Prove that all removed changes can coexist with the current main worktree,
/// and return the exact worktree tree to materialize before the history rewrite.
///
/// This must happen before refs move: a failed post-rewrite cherry-pick would
/// otherwise remove the source changes without surfacing them anywhere.
fn preflight_worktree_materialization(
    repo: &gix::Repository,
    removed_changes: impl IntoIterator<Item = (gix::ObjectId, gix::ObjectId)>,
) -> anyhow::Result<WorktreeMaterialization> {
    #[expect(
        deprecated,
        reason = "uncommit must preserve the complete main worktree while adding committed changes from another checkout"
    )]
    let before = repo.create_wd_tree(0)?;
    let mut after = before;
    let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;

    for (change_base, change_tip) in removed_changes {
        let mut merge = repo.merge_trees(
            change_base,
            change_tip,
            after,
            repo.default_merge_labels(),
            merge_options.clone(),
        )?;
        if merge.has_unresolved_conflicts(conflict_kind) {
            bail!(
                "Cannot uncommit changes into the workspace because they conflict with existing uncommitted changes"
            );
        }
        after = merge.tree.write()?.detach();
    }

    Ok(WorktreeMaterialization { before, after })
}

fn whole_commit_changes(
    repo: &gix::Repository,
    commit_ids: &[gix::ObjectId],
) -> anyhow::Result<Vec<(gix::ObjectId, gix::ObjectId)>> {
    let mut commits = commit_ids
        .iter()
        .filter(|commit_id| !commit_is_reachable_from_head(repo, **commit_id))
        .map(|commit_id| {
            let commit = but_core::Commit::from_id(commit_id.attach(repo))?;
            let before = match commit.parents.first() {
                Some(parent_id) => but_core::Commit::from_id(parent_id.attach(repo))?
                    .tree_id_or_auto_resolution()?
                    .detach(),
                None => gix::ObjectId::empty_tree(repo.object_hash()),
            };
            let after = commit.tree_id_or_auto_resolution()?.detach();
            Ok((*commit_id, before, after))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    commits.sort_by_key(|(commit_id, _, _)| commit_first_parent_depth(repo, *commit_id));
    Ok(commits
        .into_iter()
        .map(|(_, before, after)| (before, after))
        .collect())
}

fn selected_commit_changes(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<Option<(gix::ObjectId, gix::ObjectId)>> {
    if commit_is_reachable_from_head(repo, commit_id) {
        return Ok(None);
    }
    but_workspace::commit::trees_for_changes_to_uncommit(repo, commit_id, changes, context_lines)
        .map(Some)
}

fn commit_is_reachable_from_head(repo: &gix::Repository, commit_id: gix::ObjectId) -> bool {
    repo.head_id().ok().is_some_and(|head_id| {
        repo.merge_base(head_id, commit_id)
            .is_ok_and(|merge_base| merge_base.detach() == commit_id)
    })
}

fn commit_first_parent_depth(repo: &gix::Repository, mut commit_id: gix::ObjectId) -> usize {
    let mut depth = 0;
    while let Ok(commit) = but_core::Commit::from_id(commit_id.attach(repo)) {
        let Some(parent_id) = commit.parents.first() else {
            break;
        };
        depth += 1;
        commit_id = *parent_id;
    }
    depth
}

fn group_selected_commit_changes(
    sources: &[UncommitChangesSource],
) -> Vec<(gix::ObjectId, Vec<but_core::DiffSpec>)> {
    let mut groups = Vec::<(gix::ObjectId, Vec<but_core::DiffSpec>)>::new();
    for source in sources {
        if let Some((_, changes)) = groups
            .iter_mut()
            .find(|(commit_id, _)| *commit_id == source.commit_id)
        {
            changes.extend(source.changes.clone());
        } else {
            groups.push((source.commit_id, source.changes.clone()));
        }
    }
    groups
}

/// Put the preflighted tree in the main worktree before refs move. If the
/// subsequent history rewrite fails, this deliberately leaves a duplicate of
/// the source changes in the workspace rather than losing them.
fn materialize_worktree(
    repo: &gix::Repository,
    materialization: WorktreeMaterialization,
) -> anyhow::Result<()> {
    but_core::worktree::safe_checkout_from_head(
        materialization.after,
        repo,
        but_core::worktree::checkout::Options {
            skip_head_update: true,
            // The destination already contains the original worktree state.
            // Treating it as the merge base prevents checkout from applying
            // those same local changes a second time.
            merge_base_override: Some(materialization.before),
            allow_conflicted_commit_checkout: false,
        },
    )?;

    sync_worktree_index(repo)
}

/// Leave the materialized content as ordinary workspace dirt relative to the
/// current `HEAD`. This is called again after refs move because uncommitting a
/// main-workspace commit can rewrite that `HEAD` without checking it out.
fn sync_worktree_index(repo: &gix::Repository) -> anyhow::Result<()> {
    repo.index_from_tree(&repo.head_tree_id_or_empty()?)?
        .write(Default::default())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Uncommit entire commits (changes are kept in the workspace)
// ---------------------------------------------------------------------------

/// Uncommit one or more commits, removing them from branch history while
/// **keeping their changes** in the workspace as uncommitted modifications.
///
/// Unlike [`super::discard_commit::commit_discard()`], which permanently
/// removes the commit's changes, this operation reassigns the affected hunks
/// so they remain available for further editing or recommitting.
///
/// When `dry_run` is enabled, the returned workspace previews the result
/// without materializing the rewrite or persisting an oplog entry.
/// See [`commit_uncommit_only_with_perm()`] for details.
#[but_api(napi, try_from = crate::commit::json::UncommitResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<UncommitResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_with_perm(
        ctx,
        subject_commit_ids,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

/// Uncommit one or more commits, removing them from branch history while
/// **keeping their changes** in the workspace.
///
/// When `dry_run` is enabled, the returned workspace previews the result
/// without materializing the rewrite.
/// See [`commit_uncommit_only_with_perm()`] for details.
pub fn commit_uncommit_only(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<UncommitResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_only_with_perm(
        ctx,
        subject_commit_ids,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

/// Uncommit one or more commits, removing them from branch history while
/// **keeping their changes** in the workspace, and record an oplog snapshot.
///
/// When `dry_run` is enabled, the returned workspace previews the result
/// and skips oplog persistence.
/// See [`commit_uncommit_only_with_perm()`] for details.
pub fn commit_uncommit_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<UncommitResult> {
    let details = SnapshotDetails::new(OperationKind::UndoCommit)
        .with_count(subject_commit_ids.len())
        .with_trailers(subject_commit_ids.iter().copied().map(Trailer::Sha));
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        details,
        perm.read_permission(),
        dry_run,
    );

    let res = commit_uncommit_only_with_perm(ctx, subject_commit_ids, assign_to, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}

/// Uncommit one or more commits, under caller-held exclusive repository access.
///
/// The commits are removed from branch history, but their changes are
/// **kept** — they surface as uncommitted workspace modifications. When
/// `assign_to` is set, newly surfaced hunks are assigned to that stack.
///
/// This contrasts with [`super::discard_commit::commit_discard()`], which
/// removes both the commit and its changes.
///
/// When `dry_run` is enabled, it returns a preview of the resulting workspace
/// state without materializing the rewrite.
pub fn commit_uncommit_only_with_perm(
    ctx: &mut but_ctx::Context,
    subject_commit_ids: Vec<gix::ObjectId>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<UncommitResult> {
    if subject_commit_ids.is_empty() {
        anyhow::bail!("no commit IDs provided for uncommit");
    }
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let mut tx = db.transaction()?;

    let before_assignments = if assign_to.is_some() {
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            tx.hunk_assignments_mut()?,
            &repo,
            &ws,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;
        Some(assignments)
    } else {
        None
    };

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let mut rebase =
        but_workspace::commit::discard_commits(editor, subject_commit_ids.iter().copied())
            .with_context(|| {
                format!(
                    "failed to uncommit commits: {}",
                    subject_commit_ids
                        .iter()
                        .map(|id| id.to_hex().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;

    let worktree_materialization = preflight_worktree_materialization(
        &repo,
        whole_commit_changes(&repo, &subject_commit_ids)?,
    )?;

    let (workspace, replaced_commits, repo, meta) = if dry_run.into() {
        let graph = rebase.overlayed_graph()?;
        let replaced_commits = rebase.history.commit_mappings();
        let (repo, meta) = rebase.repo_and_meta_mut();
        (&mut graph.into_workspace()?, replaced_commits, repo, meta)
    } else {
        materialize_worktree(&repo, worktree_materialization)?;
        let materialized = rebase.materialize_without_checkout()?;
        sync_worktree_index(&repo)?;
        (
            materialized.workspace,
            materialized.history.commit_mappings(),
            &*repo,
            materialized.meta,
        )
    };

    if let (Some(before_assignments), Some(assign_to)) = (before_assignments, assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            tx.hunk_assignments_mut()?,
            repo,
            workspace,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;

        let before_ids: HashSet<_> = before_assignments
            .into_iter()
            .filter_map(|assignment| assignment.id)
            .collect();

        let to_assign: Vec<_> = after_assignments
            .into_iter()
            .filter(|assignment| assignment.id.is_some_and(|id| !before_ids.contains(&id)))
            .map(|assignment| HunkAssignmentRequest {
                hunk_header: assignment.hunk_header,
                path_bytes: assignment.path_bytes,
                target: Some(HunkAssignmentTarget::Stack {
                    stack_id: assign_to,
                }),
            })
            .collect();

        but_hunk_assignment::assign(
            tx.hunk_assignments_mut()?,
            repo,
            workspace,
            to_assign,
            context_lines,
        )?;
    }

    if dry_run == DryRun::No {
        tx.commit()?;
    }

    Ok(UncommitResult {
        uncommitted_ids: subject_commit_ids,
        workspace: WorkspaceState::from_workspace(workspace, meta, repo, replaced_commits)?,
    })
}

// ---------------------------------------------------------------------------
// Uncommit specific changes from a commit (changes are kept in the workspace)
// ---------------------------------------------------------------------------

/// Uncommit specific changes from a commit (removes them from the commit tree)
/// without performing a checkout.
///
/// When `dry_run` is enabled, the returned workspace previews the extracted
/// changes without materializing the rebase. See
/// [`commit_uncommit_changes_only_with_perm()`] for details.
#[but_api(try_from = crate::commit::json::MoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_only_with_perm(
        ctx,
        commit_id,
        changes,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

/// Extract `changes` from `commit_id` without performing a checkout, under
/// caller-held exclusive repository access.
///
/// The removed diff stays in the workspace as uncommitted changes. When
/// `assign_to` is set, newly surfaced hunks are reassigned to that stack after
/// the rebase is materialized. When `dry_run` is enabled, the returned
/// workspace previews the extracted changes and no hunk assignments are
/// persisted. For lower-level implementation details, see
/// [`but_workspace::commit::uncommit_changes()`].
pub fn commit_uncommit_changes_only_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let mut tx = db.transaction()?;

    let before_assignments = if assign_to.is_some() {
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            tx.hunk_assignments_mut()?,
            &repo,
            &ws,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;
        Some(assignments)
    } else {
        None
    };

    let selected_changes = changes.clone();
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let mut outcome =
        but_workspace::commit::uncommit_changes(editor, commit_id, changes, context_lines)?;
    let worktree_materialization = preflight_worktree_materialization(
        &repo,
        selected_commit_changes(&repo, commit_id, selected_changes, context_lines)?,
    )?;

    let (workspace, replaced_commits, repo, meta) = if dry_run.into() {
        let graph = outcome.rebase.overlayed_graph()?;
        let replaced_commits = outcome.rebase.history.commit_mappings();
        let (repo, meta) = outcome.rebase.repo_and_meta_mut();
        (&mut graph.into_workspace()?, replaced_commits, repo, meta)
    } else {
        materialize_worktree(&repo, worktree_materialization)?;
        let materialized = outcome.rebase.materialize_without_checkout()?;
        sync_worktree_index(&repo)?;
        (
            materialized.workspace,
            materialized.history.commit_mappings(),
            &*repo,
            materialized.meta,
        )
    };

    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            tx.hunk_assignments_mut()?,
            repo,
            workspace,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;

        let before_ids: HashSet<_> = before_assignments
            .into_iter()
            .filter_map(|assignment| assignment.id)
            .collect();

        let to_assign: Vec<_> = after_assignments
            .into_iter()
            .filter(|assignment| assignment.id.is_some_and(|id| !before_ids.contains(&id)))
            .map(|assignment| HunkAssignmentRequest {
                hunk_header: assignment.hunk_header,
                path_bytes: assignment.path_bytes,
                target: Some(HunkAssignmentTarget::Stack { stack_id }),
            })
            .collect();

        but_hunk_assignment::assign(
            tx.hunk_assignments_mut()?,
            repo,
            workspace,
            to_assign,
            context_lines,
        )?;
    }

    if dry_run == DryRun::No {
        tx.commit()?;
    }

    Ok(MoveChangesResult {
        workspace: WorkspaceState::from_workspace(workspace, meta, repo, replaced_commits)?,
    })
}

/// Extract `changes` from `commit_id` and record the rewrite in the oplog.
///
/// When `dry_run` is enabled, the returned workspace previews the extracted
/// changes and no oplog entry is persisted. See
/// [`commit_uncommit_changes_with_perm()`] for details.
#[but_api(napi, try_from = crate::commit::json::MoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<MoveChangesResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_with_perm(
        ctx,
        commit_id,
        changes,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

/// Extract `changes` from `commit_id` under caller-held exclusive repository
/// access and record an oplog snapshot on success.
///
/// When `assign_to` is set, newly surfaced hunks are assigned to that stack
/// after the rebase is materialized. This prepares a best-effort
/// `DiscardChanges` oplog snapshot and commits it only if the operation
/// succeeds. When `dry_run` is enabled, it returns a preview of the resulting
/// workspace state and skips both hunk-assignment persistence and oplog
/// persistence. For lower-level implementation details, see
/// [`but_workspace::commit::uncommit_changes()`].
pub fn commit_uncommit_changes_with_perm(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        perm.read_permission(),
        dry_run,
    );

    let res =
        commit_uncommit_changes_only_with_perm(ctx, commit_id, changes, assign_to, dry_run, perm);

    if let Some(snapshot) = maybe_oplog_entry
        && res
            .as_ref()
            .is_ok_and(|result| !result.workspace.replaced_commits.is_empty())
    {
        snapshot.commit(ctx, perm).ok();
    }

    res
}

// ---------------------------------------------------------------------------
// Uncommit specific changes from multiple commits (best effort)
// ---------------------------------------------------------------------------

/// Uncommit specific changes from multiple commits without performing a
/// checkout.
///
/// Input sources are flat and may be unsorted or contain multiple entries for
/// the same commit. The backend groups them by commit, applies successful
/// groups in child-to-parent order, and reports failed groups in the result.
#[but_api(try_from = crate::commit::json::UncommitChangesFromCommitsResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_from_commits_only(
    ctx: &mut but_ctx::Context,
    sources: Vec<UncommitChangesSource>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<UncommitChangesFromCommitsResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_from_commits_only_with_perm(
        ctx,
        sources,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

/// Uncommit specific changes from multiple commits under caller-held
/// exclusive repository access.
pub fn commit_uncommit_changes_from_commits_only_with_perm(
    ctx: &mut but_ctx::Context,
    sources: Vec<UncommitChangesSource>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<UncommitChangesFromCommitsResult> {
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let mut tx = db.transaction()?;

    let before_assignments = if assign_to.is_some() {
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            tx.hunk_assignments_mut()?,
            &repo,
            &ws,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;
        Some(assignments)
    } else {
        None
    };

    let mut preflight_sources = group_selected_commit_changes(&sources);
    preflight_sources.sort_by_key(|(commit_id, _)| commit_first_parent_depth(&repo, *commit_id));
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let workspace_sources = sources
        .into_iter()
        .map(|source| but_workspace::commit::UncommitChangesSource {
            commit_id: source.commit_id,
            changes: source.changes,
        })
        .collect::<Vec<_>>();
    let outcome = but_workspace::commit::uncommit_changes_from_commits(
        editor,
        workspace_sources,
        context_lines,
    )?;
    let failed_ids: HashSet<_> = outcome
        .failures
        .iter()
        .map(|failure| failure.commit_id)
        .collect();
    let worktree_materialization = if outcome.rebase.is_some() {
        let removed_changes = preflight_sources
            .into_iter()
            .filter(|(commit_id, _)| !failed_ids.contains(commit_id))
            .map(|(commit_id, changes)| {
                selected_commit_changes(&repo, commit_id, changes, context_lines)
            })
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .flatten();
        Some(preflight_worktree_materialization(&repo, removed_changes)?)
    } else {
        None
    };
    let failures = outcome
        .failures
        .into_iter()
        .map(|failure| UncommitChangesFailure {
            commit_id: failure.commit_id,
            changes: failure.changes,
            error: failure.error,
        })
        .collect::<Vec<_>>();

    let mut rebase = outcome.rebase;
    let (workspace, replaced_commits, repo, meta) = if dry_run.into() {
        if let Some(rebase) = rebase.as_mut() {
            let graph = rebase.overlayed_graph()?;
            let replaced_commits = rebase.history.commit_mappings();
            let (repo, meta) = rebase.repo_and_meta_mut();
            (&mut graph.into_workspace()?, replaced_commits, repo, meta)
        } else {
            (&mut *ws, BTreeMap::new(), &*repo, &mut meta)
        }
    } else if let Some(rebase) = rebase {
        materialize_worktree(
            &repo,
            worktree_materialization.expect("a successful rebase has extracted changes"),
        )?;
        let materialized = rebase.materialize_without_checkout()?;
        sync_worktree_index(&repo)?;
        (
            materialized.workspace,
            materialized.history.commit_mappings(),
            &*repo,
            materialized.meta,
        )
    } else {
        (&mut *ws, BTreeMap::new(), &*repo, &mut meta)
    };

    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            tx.hunk_assignments_mut()?,
            repo,
            workspace,
            None::<Vec<but_core::TreeChange>>,
            context_lines,
        )?;

        let before_ids: HashSet<_> = before_assignments
            .into_iter()
            .filter_map(|assignment| assignment.id)
            .collect();

        let to_assign: Vec<_> = after_assignments
            .into_iter()
            .filter(|assignment| assignment.id.is_some_and(|id| !before_ids.contains(&id)))
            .map(|assignment| HunkAssignmentRequest {
                hunk_header: assignment.hunk_header,
                path_bytes: assignment.path_bytes,
                target: Some(HunkAssignmentTarget::Stack { stack_id }),
            })
            .collect();

        but_hunk_assignment::assign(
            tx.hunk_assignments_mut()?,
            repo,
            workspace,
            to_assign,
            context_lines,
        )?;
    }

    if dry_run == DryRun::No {
        tx.commit()?;
    }

    Ok(UncommitChangesFromCommitsResult {
        workspace: WorkspaceState::from_workspace(workspace, meta, repo, replaced_commits)?,
        failures,
    })
}

/// Uncommit specific changes from multiple commits and record an oplog
/// snapshot on success.
#[but_api(
    napi,
    try_from = crate::commit::json::UncommitChangesFromCommitsResult
)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_from_commits(
    ctx: &mut but_ctx::Context,
    sources: Vec<UncommitChangesSource>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
) -> anyhow::Result<UncommitChangesFromCommitsResult> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_uncommit_changes_from_commits_with_perm(
        ctx,
        sources,
        assign_to,
        dry_run,
        guard.write_permission(),
    )
}

/// Uncommit specific changes from multiple commits under caller-held
/// exclusive repository access and record an oplog snapshot when at least the
/// operation itself succeeds.
pub fn commit_uncommit_changes_from_commits_with_perm(
    ctx: &mut but_ctx::Context,
    sources: Vec<UncommitChangesSource>,
    assign_to: Option<but_core::ref_metadata::StackId>,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<UncommitChangesFromCommitsResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        perm.read_permission(),
        dry_run,
    );

    let res =
        commit_uncommit_changes_from_commits_only_with_perm(ctx, sources, assign_to, dry_run, perm);

    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }

    res
}

#[cfg(test)]
mod tests {
    use but_testsupport::{CommandExt as _, git_at_dir, open_repo};

    use super::{preflight_worktree_materialization, whole_commit_changes};

    #[test]
    fn preflight_rejects_overlap_without_changing_head_or_worktree() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let git = || git_at_dir(tmp.path());
        git().args(["init", "-b", "main"]).run();
        git().args(["config", "user.name", "GitButler"]).run();
        git()
            .args(["config", "user.email", "gitbutler@example.com"])
            .run();
        std::fs::write(tmp.path().join("file.txt"), "base\n")?;
        git().args(["add", "file.txt"]).run();
        git().args(["commit", "-m", "base"]).run();
        git().args(["checkout", "-b", "source"]).run();
        std::fs::write(tmp.path().join("file.txt"), "from source commit\n")?;
        git().args(["commit", "-am", "source"]).run();
        git().args(["checkout", "main"]).run();
        std::fs::write(tmp.path().join("file.txt"), "existing workspace dirt\n")?;

        let repo = open_repo(tmp.path())?;
        let source_id = repo.rev_parse_single("source")?.detach();
        let head_before = repo.head_id()?.detach();
        let contents_before = std::fs::read(tmp.path().join("file.txt"))?;

        let err =
            preflight_worktree_materialization(&repo, whole_commit_changes(&repo, &[source_id])?)
                .expect_err("overlapping committed and workspace changes must be rejected");
        assert!(err.to_string().contains("conflict"));
        assert_eq!(repo.head_id()?.detach(), head_before);
        assert_eq!(std::fs::read(tmp.path().join("file.txt"))?, contents_before);
        Ok(())
    }
}
