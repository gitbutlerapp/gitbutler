use anyhow::{bail, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::RepositoryExt as _;

use crate::VirtualBranchesExt as _;

pub(crate) fn checkout_branch_trees<'a>(
    ctx: &'a CommandContext,
    _perm: &mut WorktreeWritePermission,
) -> Result<git2::Tree<'a>> {
    let repository = ctx.repository();
    let vb_state = ctx.project().virtual_branches();
    let branches = vb_state.list_branches_in_workspace()?;

    if branches.is_empty() {
        // If there are no applied branches, then return the current uncommtied state
        return repository.create_wd_tree();
    };

    if branches.len() == 1 {
        let tree = repository.find_tree(branches[0].tree)?;
        repository
            .checkout_tree_builder(&tree)
            .force()
            .remove_untracked()
            .checkout()?;

        Ok(tree)
    } else {
        let merge_base =
            repository.merge_base_many(&branches.iter().map(|b| b.head).collect::<Vec<_>>())?;

        let merge_base_tree = repository.find_commit(merge_base)?.tree()?;

        let mut final_tree = merge_base_tree.clone();

        for branch in branches {
            let theirs = repository.find_tree(branch.tree)?;
            let mut merge_index =
                repository.merge_trees(&merge_base_tree, &final_tree, &theirs, None)?;

            if merge_index.has_conflicts() {
                bail!("There appears to be conflicts between the virtual branches");
            };

            let tree_oid = merge_index.write_tree_to(repository)?;
            final_tree = repository.find_tree(tree_oid)?;
        }

        repository
            .checkout_tree_builder(&final_tree)
            .force()
            .remove_untracked()
            .checkout()?;

        Ok(final_tree)
    }
}
