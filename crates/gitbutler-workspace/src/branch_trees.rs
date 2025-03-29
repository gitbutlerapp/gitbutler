use anyhow::{bail, Context, Result};
use gitbutler_cherry_pick::{ConflictedTreeKey, GixRepositoryExt as _, RepositoryExt};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_oxidize::{
    git2_to_gix_object_id, gix_to_git2_oid, GixRepositoryExt, ObjectIdExt, OidExt, RepoExt,
};
use gitbutler_project::access::{WorktreeReadPermission, WorktreeWritePermission};
use gitbutler_project::AUTO_TRACK_LIMIT_BYTES;
use gitbutler_repo::rebase::cherry_rebase_group;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use tracing::instrument;

use crate::workspace_base;

/// Checks out the combined trees of all branches in the workspace.
///
/// This function will fail if the applied branches conflict with each other.
#[instrument(level = tracing::Level::DEBUG, skip(ctx, _perm), err(Debug))]
#[deprecated]
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
        let heads = stacks
            .iter()
            .map(|b| b.head(&gix_repo).map(|h| h.to_gix()))
            .collect::<Result<Vec<_>>>()?;
        let merge_base_tree_id = gix_repo
            .merge_base_octopus(heads)?
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

pub struct WorkspaceState {
    heads: Vec<gix::ObjectId>,
    base: gix::ObjectId,
}

impl WorkspaceState {
    pub fn create(ctx: &CommandContext, perm: &WorktreeReadPermission) -> Result<Self> {
        let repository = ctx.gix_repository()?;
        let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

        let heads = vb_state
            .list_stacks_in_workspace()?
            .iter()
            .map(|stack| -> Result<gix::ObjectId> {
                let tree_id = repository.find_real_tree(
                    &stack.head(&repository)?.to_gix(),
                    ConflictedTreeKey::AutoResolution,
                )?;
                Ok(tree_id.detach())
            })
            .collect::<Result<Vec<_>>>()?;

        let base = workspace_base(ctx, perm)?;

        Ok(WorkspaceState { heads, base })
    }
}

/// Take the
pub fn update_uncommited_changes(
    ctx: &CommandContext,
    old: WorkspaceState,
    new: WorkspaceState,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let git2_repository = ctx.repo();
    let uncommited_changes = git2_repository.create_wd_tree(0)?.id().to_gix();

    update_uncommited_changes_with_tree(ctx, old, new, uncommited_changes, perm)
}

pub fn update_uncommited_changes_with_tree(
    ctx: &CommandContext,
    old: WorkspaceState,
    new: WorkspaceState,
    tree: gix::ObjectId,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repository = ctx.gix_repository()?;
    let git2_repository = ctx.repo();

    let new_uncommited_changes = move_tree_between_workspaces(&repository, tree, old, new)?;
    let new_uncommited_changes = git2_repository.find_tree(new_uncommited_changes.to_git2())?;

    git2_repository
        .checkout_tree_builder(&new_uncommited_changes)
        .force()
        .checkout()
        .context("failed to checkout tree")?;

    Ok(())
}

/// Take the changes on top of one workspace and return what they would look
/// like if they were on top of the new workspace.
fn move_tree_between_workspaces(
    repository: &gix::Repository,
    tree: gix::ObjectId,
    old: WorkspaceState,
    new: WorkspaceState,
) -> Result<gix::ObjectId> {
    let old_workspace = merge_workspace(repository, old)?;
    let new_workspace = merge_workspace(repository, new)?;
    move_tree(repository, tree, old_workspace, new_workspace)
}

/// Cherry pick a tree from one base tree on to another, favoring the contents of the tree when conflicts occur
fn move_tree(
    repository: &gix::Repository,
    tree: gix::ObjectId,
    old_workspace: gix::ObjectId,
    new_workspace: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let merge_options = repository
        .tree_merge_options()?
        .with_file_favor(Some(gix::merge::tree::FileFavor::Ours))
        .with_tree_favor(Some(gix::merge::tree::TreeFavor::Ours));

    // Read: Take the diff between old_workspace and tree, and apply it on top
    //   of new_workspace
    let mut merge = repository.merge_trees(
        old_workspace,
        tree,
        new_workspace,
        repository.default_merge_labels(),
        merge_options,
    )?;

    Ok(merge.tree.write()?.detach())
}

/// Octopus merge
/// What: Takes N trees and a base tree and all the heads together with respect
/// to the given base.
///
/// If there are no heads provided, the base will be returned.
fn merge_workspace(
    repository: &gix::Repository,
    workspace: WorkspaceState,
) -> Result<gix::ObjectId> {
    let mut output = workspace.base;

    for head in workspace.heads {
        let (merge_options_fail_fast, conflict_kind) = repository.merge_options_fail_fast()?;
        let mut merge = repository.merge_trees(
            workspace.base,
            output,
            head,
            repository.default_merge_labels(),
            merge_options_fail_fast.clone(),
        )?;

        if merge.has_unresolved_conflicts(conflict_kind) {
            bail!("There appears to be conflicts between the virtual branches");
        };

        output = merge.tree.write()?.detach();
    }

    Ok(output)
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
#[deprecated = "not needed after v3 is out"]
pub fn compute_updated_branch_head(
    repository: &git2::Repository,
    stack: &Stack,
    new_head: git2::Oid,
) -> Result<BranchHeadAndTree> {
    #[allow(deprecated)]
    compute_updated_branch_head_for_commits(
        repository,
        stack.head(&repository.to_gix()?)?,
        stack.tree,
        new_head,
    )
}

/// Given a new head for a branch, this comptues how the tree should be
/// rebased on top of the new head. If the rebased tree is conflicted, then
/// the function will return a new head commit which is the conflicted
/// tree commit, and the tree oid will be the auto-resolved tree.
///
/// If you have access to a [`Stack`] object, it's probably preferable to
/// use [`compute_updated_branch_head`] instead to prevent programmer error.
///
/// This does not mutate the branch, or update the virtual_branches.toml.
/// You probably also want to call [`checkout_branch_trees`] after you have
/// mutated the virtual_branches.toml.
#[deprecated = "not needed after v3 is out"]
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

    let rebased_tree = cherry_rebase_group(repository, new_head, &[commited_tree], false, false)?;
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
