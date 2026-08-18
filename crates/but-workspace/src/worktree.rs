//! Utilities for reasoning about the repository worktree.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use bstr::{BString, ByteSlice};
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

/// Mark the conflicted uncommitted files at `rela_paths` as resolved.
///
/// A conflicted uncommitted file is one that has index entries at stages 1/2/3,
/// as written by a checkout that was allowed to conflict with uncommitted changes
/// (see `but_core::worktree::checkout`). Resolving it is what `git add -- <path>`
/// (or `git rm` for a deleted file) does: the conflict stages are replaced by a
/// stage-0 entry holding the current worktree content, or removed entirely if the
/// file no longer exists.
///
/// The worktree content is taken as-is, so leftover conflict markers are not checked.
/// Paths that are not conflicted are an error.
pub fn resolve_worktree_conflicts(
    repo: &gix::Repository,
    rela_paths: impl IntoIterator<Item = BString>,
) -> Result<()> {
    use crate::commit_engine::index::{delete_entry_by_path_bounded_stages, upsert_index_entry};
    use gix::index::entry::{Flags, Stage};

    let rela_paths: BTreeSet<_> = rela_paths.into_iter().collect();
    let mut index = repo.index_or_empty()?.into_owned_or_cloned();
    let (mut pipeline, index_for_filter) = repo.filter_pipeline(None)?;
    let mut path_check = gix::status::plumbing::SymlinkCheck::new(
        repo.workdir().context("non-bare repository")?.into(),
    );
    let mut num_sorted_entries = index.entries().len();

    for rela_path in &rela_paths {
        let rela_path = rela_path.as_bstr();
        let entries_before = num_sorted_entries;
        delete_entry_by_path_bounded_stages(
            &mut index,
            rela_path,
            &mut num_sorted_entries,
            &[Stage::Base, Stage::Ours, Stage::Theirs],
        );
        if entries_before == num_sorted_entries {
            anyhow::bail!("'{rela_path}' has no unresolved conflict");
        }
        // Stat before hashing so a write in between is caught by the stat mismatch.
        let md = match gix::index::fs::Metadata::from_path_no_follow(
            &path_check.verified_path_allow_nonexisting(rela_path)?,
        ) {
            Ok(md) => md,
            Err(err) if gix::fs::io_err::is_not_found(err.kind(), err.raw_os_error()) => {
                delete_entry_by_path_bounded_stages(
                    &mut index,
                    rela_path,
                    &mut num_sorted_entries,
                    &[Stage::Unconflicted],
                );
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        let (id, kind, _) = pipeline
            .worktree_file_to_object(rela_path, &index_for_filter)?
            .with_context(|| format!("'{rela_path}' is not a file, symlink or submodule"))?;
        upsert_index_entry(
            &mut index,
            rela_path,
            Some(&md),
            id,
            kind.into(),
            Flags::empty(),
            &mut num_sorted_entries,
        )?;
    }

    index.remove_tree();
    index.remove_resolve_undo();
    index.sort_entries();
    index.write(Default::default())?;
    Ok(())
}
