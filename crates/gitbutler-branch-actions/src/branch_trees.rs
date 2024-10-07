use anyhow::{bail, Result};
use gitbutler_branch::Branch;
use gitbutler_cherry_pick::RepositoryExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::rebase::cherry_rebase_group;
use gitbutler_repo::RepositoryExt as _;

use crate::VirtualBranchesExt as _;

/// Checks out the combined trees of all branches in the workspace.
///
/// This function will fail if the applied branches conflict with each other.
pub fn checkout_branch_trees<'a>(
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

pub struct BranchHeadAndTree {
    /// This is a commit Oid.
    ///
    /// This should be used as the new head Oid for the branch.
    pub head: git2::Oid,
    /// This is a tree Oid.
    ///
    /// This should be used as the new tree Oid for the branch.
    pub tree: git2::Oid,
}

/// Given a new head for a branch, this comptues how the tree should be
/// rebased on top of the new head. If the rebased tree is conflicted, then
/// the function will return a new head commit which is the conflicted
/// tree commit, and the the tree oid will be the auto-resolved tree.
///
/// This does not mutate the branch, or update the virtual_branches.toml.
/// You probably also want to call [`checkout_branch_trees`] after you have
/// mutated the virtual_branches.toml.
pub fn compute_updated_branch_head(
    repository: &git2::Repository,
    branch: &Branch,
    new_head: git2::Oid,
    fearless_rebasing: bool,
) -> Result<BranchHeadAndTree> {
    compute_updated_branch_head_for_commits(
        repository,
        branch.head,
        branch.tree,
        new_head,
        fearless_rebasing,
    )
}

/// Given a new head for a branch, this comptues how the tree should be
/// rebased on top of the new head. If the rebased tree is conflicted, then
/// the function will return a new head commit which is the conflicted
/// tree commit, and the the tree oid will be the auto-resolved tree.
///
/// If you have access to a [`Branch`] object, it's probably preferable to
/// use [`compute_updated_branch_head`] instead to prevent programmer error.
///
/// This does not mutate the branch, or update the virtual_branches.toml.
/// You probably also want to call [`checkout_branch_trees`] after you have
/// mutated the virtual_branches.toml.
pub fn compute_updated_branch_head_for_commits(
    repository: &git2::Repository,
    old_head: git2::Oid,
    old_tree: git2::Oid,
    new_head: git2::Oid,
    fearless_rebasing: bool,
) -> Result<BranchHeadAndTree> {
    let (author, committer) = repository.signatures()?;

    let commited_tree = repository.commit_with_signature(
        None,
        &author,
        &committer,
        "Uncommited changes",
        &repository.find_tree(old_tree)?,
        &[&repository.find_commit(old_head)?],
        Default::default(),
    )?;

    let rebased_tree =
        cherry_rebase_group(repository, new_head, &[commited_tree], fearless_rebasing)?;
    let rebased_tree = repository.find_commit(rebased_tree)?;

    if rebased_tree.is_conflicted() {
        let auto_tree_id = repository
            .find_real_tree(&rebased_tree, Default::default())?
            .id();

        Ok(BranchHeadAndTree {
            head: rebased_tree.id(),
            tree: auto_tree_id,
        })
    } else {
        Ok(BranchHeadAndTree {
            head: new_head,
            tree: rebased_tree.tree_id(),
        })
    }
}

// These possibly could be considered more "integration" tests, but there is no
// need for `checkout_branch_trees` to be public, so it is tested here.
#[cfg(test)]
mod test {
    use std::fs;

    use gitbutler_branch::BranchCreateRequest;
    use gitbutler_command_context::CommandContext;
    use gitbutler_repo::RepositoryExt as _;
    use gitbutler_testsupport::{paths, testing_repository::assert_tree_matches, TestProject};

    #[test]
    fn checkout_with_two_branches() {
        let test_project = &TestProject::default();

        let data_dir = paths::data_dir();
        let projects = gitbutler_project::Controller::from_path(data_dir.path());

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        crate::set_base_branch(&project, &"refs/remotes/origin/master".parse().unwrap()).unwrap();

        let branch_1 =
            crate::create_virtual_branch(&project, &BranchCreateRequest::default()).unwrap();

        fs::write(test_project.path().join("foo.txt"), "content").unwrap();

        crate::create_commit(&project, branch_1, "commit one", None, false).unwrap();

        let branch_2 =
            crate::create_virtual_branch(&project, &BranchCreateRequest::default()).unwrap();

        fs::write(test_project.path().join("bar.txt"), "content").unwrap();

        crate::create_commit(&project, branch_2, "commit two", None, false).unwrap();

        let tree = test_project.local_repository.create_wd_tree().unwrap();

        // Assert original state
        assert_tree_matches(
            &test_project.local_repository,
            &tree,
            &[("foo.txt", b"content"), ("bar.txt", b"content")],
        );
        assert_eq!(tree.len(), 2);

        // Checkout an empty tree
        {
            let tree_oid = test_project
                .local_repository
                .treebuilder(None)
                .unwrap()
                .write()
                .unwrap();
            let tree = test_project.local_repository.find_tree(tree_oid).unwrap();
            test_project
                .local_repository
                .checkout_tree_builder(&tree)
                .force()
                .remove_untracked()
                .checkout()
                .unwrap();
        }

        // Assert tree is indeed empty
        {
            let tree: git2::Tree = test_project.local_repository.create_wd_tree().unwrap();

            // Tree should be empty
            assert_eq!(
                tree.len(),
                0,
                "Should be empty after checking out an empty tree"
            );
        }

        let ctx = CommandContext::open(&project).unwrap();
        let mut guard = project.exclusive_worktree_access();

        super::checkout_branch_trees(&ctx, guard.write_permission()).unwrap();

        let tree = test_project.local_repository.create_wd_tree().unwrap();

        // Should be back to original state
        assert_tree_matches(
            &test_project.local_repository,
            &tree,
            &[("foo.txt", b"content"), ("bar.txt", b"content")],
        );
        assert_eq!(tree.len(), 2, "Should match original state");
    }
}
