use anyhow::{bail, Result};
use gitbutler_oxidize::GixRepositoryExt;

fn get_first_parent(commit: gix::Commit) -> Result<gix::Commit> {
    let first_parent = commit.parent_ids().take(1).collect::<Vec<_>>();
    let Some(first_parent) = first_parent.first() else {
        bail!("Failed to find first parent of {}", commit.id())
    };
    let first_parent = first_parent.object()?.into_commit();
    Ok(first_parent)
}

/// Takes two trees and
fn is_subset(
    repository: &gix::Repository,
    superset_id: gix::ObjectId,
    subset_id: gix::ObjectId,
    common_base_id: gix::ObjectId,
) -> Result<bool> {
    // Find all the relevant commits
    let superset = repository.find_commit(superset_id)?;
    let superset_parent = get_first_parent(superset.clone())?;
    let subset = repository.find_commit(subset_id)?;
    let subset_parent = get_first_parent(subset.clone())?;
    let common_base = repository.find_commit(common_base_id)?;

    let exclusive_superset = repository
        .merge_trees(
            superset_parent.tree_id()?,
            superset.tree_id()?,
            common_base.tree_id()?,
            Default::default(),
            repository.merge_options_force_ours()?,
        )?
        .tree
        .write()?;

    let exclusive_subset = repository
        .merge_trees(
            subset_parent.tree_id()?,
            subset.tree_id()?,
            common_base.tree_id()?,
            Default::default(),
            repository.merge_options_force_ours()?,
        )?
        .tree
        .write()?;

    let (options, _) = repository.merge_options_fail_fast()?;
    let mut merged_exclusives = repository.merge_trees(
        common_base.tree_id()?,
        exclusive_superset,
        exclusive_subset,
        Default::default(),
        options,
    )?;

    if merged_exclusives.conflicts.is_empty() {
        Ok(exclusive_superset == merged_exclusives.tree.write()?)
    } else {
        Ok(false)
    }
}
