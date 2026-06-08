//! Utilities for reasoning about the repository worktree.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use but_core::{Commit, RefMetadata, RepositoryExt};
use but_rebase::graph_rebase::SuccessfulRebase;
use gix::merge::tree::TreatAsUnresolved;
use gix::prelude::ObjectIdExt;

/// Return paths in the current dirty worktree that would conflict if applied
/// onto the workspace head produced by `rebase`.
///
/// This is intentionally preview-oriented: it uses the in-memory repository
/// behind the rebase result so callers can compute conflicts before
/// materialization, including during dry-runs.
pub fn worktree_conflicts_for_rebase<M: RefMetadata>(
    rebase: &SuccessfulRebase<'_, '_, M>,
) -> Result<Vec<but_serde::BStringForFrontend>> {
    let repo = rebase.repo();
    let current_head_tree = repo.head_tree_id_or_empty()?.detach();
    let dirty_worktree_trees = dirty_worktree_trees(repo, current_head_tree)?;
    if dirty_worktree_trees.is_empty() {
        return Ok(Vec::new());
    }

    let preview_workspace = rebase.overlayed_graph()?.into_workspace()?;
    let resulting_head = preview_workspace
        .graph
        .entrypoint()?
        .commit()
        .context("Cannot compute worktree conflicts without a resulting workspace head")?;
    let resulting_head_tree =
        Commit::from_id(resulting_head.id.attach(repo))?.tree_id_or_auto_resolution()?;

    let (merge_options, _) = repo.merge_options_no_rewrites_fail_fast()?;
    let conflict_kind = TreatAsUnresolved::git();
    let mut conflicts = BTreeSet::new();

    for dirty_worktree_tree in dirty_worktree_trees {
        let merge = repo.merge_trees(
            current_head_tree,
            resulting_head_tree,
            dirty_worktree_tree,
            repo.default_merge_labels(),
            merge_options
                .clone()
                .with_fail_on_conflict(Some(conflict_kind)),
        )?;

        conflicts.extend(
            merge
                .conflicts
                .iter()
                .filter(|conflict| conflict.is_unresolved(conflict_kind))
                .map(|conflict| conflict.ours.location().to_owned()),
        );
    }

    Ok(conflicts.into_iter().map(Into::into).collect())
}

fn dirty_worktree_trees(
    repo: &gix::Repository,
    current_head_tree: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    let changes = but_core::diff::worktree_changes_no_renames(repo)?;
    if changes.changes.is_empty()
        && changes.index_changes.is_empty()
        && changes.index_conflicts.is_empty()
    {
        return Ok(Vec::new());
    }

    let mut selection = changes
        .changes
        .iter()
        .map(|change| change.path.clone())
        .collect::<BTreeSet<_>>();
    selection.extend(
        changes
            .index_changes
            .iter()
            .map(|change| change.location().to_owned()),
    );
    selection.extend(changes.index_conflicts.iter().map(|(path, _)| path.clone()));

    let snapshot = but_core::snapshot::create_tree(
        current_head_tree.attach(repo),
        but_core::snapshot::create_tree::State {
            changes,
            selection,
            head: false,
        },
    )?;

    let mut trees = Vec::new();
    for tree in [snapshot.worktree, snapshot.index].into_iter().flatten() {
        if tree != current_head_tree && !trees.contains(&tree) {
            trees.push(tree);
        }
    }
    Ok(trees)
}
