use anyhow::{bail, Result};
use gitbutler_cherry_pick::RepositoryExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_oxidize::{git2_to_gix_object_id, gix_to_git2_oid, GixRepositoryExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_project::AUTO_TRACK_LIMIT_BYTES;
use gitbutler_repo::rebase::cherry_rebase_group;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use tracing::instrument;

/// Checks out the combined trees of all branches in the workspace.
///
/// This function will fail if the applied branches conflict with each other.
#[instrument(level = tracing::Level::DEBUG, skip(ctx, _perm), err(Debug))]
pub fn checkout_branch_trees<'a>(
    ctx: &'a CommandContext,
    _perm: &mut WorktreeWritePermission,
) -> Result<git2::Tree<'a>> {
    let repository = ctx.repo();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;

    if stacks.is_empty() {
        // If there are no applied branches, then return the current uncommtied state
        return repository.create_wd_tree(AUTO_TRACK_LIMIT_BYTES);
    };

    if stacks.len() == 1 {
        let tree = repository.find_tree(stacks[0].tree)?;
        repository
            .checkout_tree_builder(&tree)
            .force()
            .remove_untracked()
            .checkout()?;

        Ok(tree)
    } else {
        let gix_repo = ctx.gix_repository_for_merging()?;
        let merge_base_tree_id = gix_repo
            .merge_base_octopus(stacks.iter().map(|b| git2_to_gix_object_id(b.head())))?
            .object()?
            .into_commit()
            .tree_id()?;

        let mut final_tree_id = merge_base_tree_id;
        let (merge_options_fail_fast, conflict_kind) = gix_repo.merge_options_fail_fast()?;
        for branch in stacks {
            let their_tree_id = git2_to_gix_object_id(branch.tree);
            let mut merge = gix_repo.merge_trees(
                merge_base_tree_id,
                final_tree_id,
                their_tree_id,
                gix_repo.default_merge_labels(),
                merge_options_fail_fast.clone(),
            )?;

            if merge.has_unresolved_conflicts(conflict_kind) {
                bail!("There appears to be conflicts between the virtual branches");
            };

            final_tree_id = merge.tree.write()?;
        }

        let final_tree = repository.find_tree(gix_to_git2_oid(final_tree_id))?;
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
    stack: &Stack,
    new_head: git2::Oid,
) -> Result<BranchHeadAndTree> {
    compute_updated_branch_head_for_commits(repository, stack.head(), stack.tree, new_head)
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

    let rebased_tree = cherry_rebase_group(repository, new_head, &[commited_tree], false)?;
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
