use crate::{DiffSpec, commit_engine::index::apply_lhs_to_rhs};
use anyhow::Context;
use but_status::get_status;

use super::utils::{
    ChangesSource, create_tree_without_diff, index_entries_to_update, update_wd_to_tree,
};

/// Same as create_index_without_changes, but specifically for the worktree.
///
/// The index will be written to the repository if any changes are made to it.
pub fn discard_workspace_changes(
    repository: &gix::Repository,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<Vec<DiffSpec>> {
    let (tree, dropped) =
        create_tree_without_diff(repository, ChangesSource::Worktree, changes, context_lines)?;
    let status_changes = get_status(repository)?;

    update_wd_to_tree(repository, tree)?;

    let tree_as_index = repository.index_from_tree(&tree)?;
    let mut index = repository.index_or_empty()?.into_owned_or_cloned();

    let paths_to_update = index_entries_to_update(status_changes)?;

    apply_lhs_to_rhs(
        repository.workdir().context("non-bare repository")?,
        &tree_as_index,
        &mut index,
        Some(paths_to_update),
    )?;
    index.write(Default::default())?;

    Ok(dropped)
}
