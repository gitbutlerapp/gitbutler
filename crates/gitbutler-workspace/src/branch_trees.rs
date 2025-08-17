use anyhow::Result;
use gitbutler_cherry_pick::RepositoryExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_oxidize::{ObjectIdExt, OidExt, RepoExt};
use gitbutler_project::access::{WorktreeReadPermission, WorktreeWritePermission};
use gitbutler_repo::RepositoryExt as _;
use gitbutler_stack::{Stack, VirtualBranchesHandle};

use crate::workspace_base;

/// A snapshot of the workspace at a point in time.
#[derive(Debug)]
pub struct WorkspaceState {
    /// The heads of the stacks in the workspace.
    heads: Vec<git2::Oid>,
    /// The base of the workspace.
    base: git2::Oid,
}

impl WorkspaceState {
    pub fn create(ctx: &CommandContext, perm: &WorktreeReadPermission) -> Result<Self> {
        let repo = ctx.repo();
        let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

        let heads = vb_state
            .list_stacks_in_workspace()?
            .iter()
            .map(|stack| -> Result<git2::Oid> {
                let head = stack.head_oid(&repo.to_gix()?)?.to_git2();
                let commit = repo.find_commit(head)?;
                let tree = repo.find_real_tree(&commit, Default::default())?;
                Ok(tree.id())
            })
            .collect::<Result<Vec<_>>>()?;

        let base = workspace_base(ctx, perm)?.to_git2();
        let base_tree_id = repo.find_commit(base)?.tree_id();

        Ok(WorkspaceState {
            heads,
            base: base_tree_id,
        })
    }
}

/// Update the uncommited changes from one snapshot of the workspace and rebase
/// them on top of the new snapshot.
pub fn update_uncommited_changes(
    ctx: &CommandContext,
    old: WorkspaceState,
    new: WorkspaceState,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repo = ctx.repo();
    let uncommited_changes = repo.create_wd_tree(0)?;

    update_uncommited_changes_with_tree(ctx, old, new, uncommited_changes.id(), None, perm)
}

pub fn update_uncommited_changes_with_tree(
    ctx: &CommandContext,
    old: WorkspaceState,
    new: WorkspaceState,
    old_uncommited_changes: git2::Oid,
    always_checkout: Option<bool>,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repo = ctx.repo();

    let mut new_uncommited_changes =
        move_tree_between_workspaces(repo, old_uncommited_changes, old, new)?;

    // If the new tree and old tree are the same, then we don't need to do anything
    if !new_uncommited_changes.has_conflicts() && !always_checkout.unwrap_or(false) {
        let tree = new_uncommited_changes.write_tree_to(repo)?;
        if tree == old_uncommited_changes {
            return Ok(());
        }
    }

    repo.checkout_index(
        Some(&mut new_uncommited_changes),
        Some(
            git2::build::CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;

    Ok(())
}

/// Take the changes on top of one workspace and return what they would look
/// like if they were on top of the new workspace.
fn move_tree_between_workspaces(
    repo: &git2::Repository,
    tree: git2::Oid,
    old: WorkspaceState,
    new: WorkspaceState,
) -> Result<git2::Index> {
    let old_workspace = merge_workspace(repo, old)?;
    let new_workspace = merge_workspace(repo, new)?;
    move_tree(repo, tree, old_workspace, new_workspace)
}

/// Cherry pick a tree from one base tree on to another, favoring the contents of the tree when conflicts occur
fn move_tree(
    repo: &git2::Repository,
    tree: git2::Oid,
    old_workspace: git2::Oid,
    new_workspace: git2::Oid,
) -> Result<git2::Index> {
    // Read: Take the diff between old_workspace and tree, and apply it on top
    //   of new_workspace
    let merge = repo.merge_trees(
        &repo.find_tree(old_workspace)?,
        &repo.find_tree(tree)?,
        &repo.find_tree(new_workspace)?,
        None,
    )?;

    Ok(merge)
}

/// Octopus merge
/// What: Takes N trees and a base tree and all the heads together with respect
/// to the given base.
///
/// If there are no heads provided, the base will be returned.
fn merge_workspace(repo: &git2::Repository, workspace: WorkspaceState) -> Result<git2::Oid> {
    let mut output = workspace.base;

    for head in workspace.heads {
        let mut merge_options = git2::MergeOptions::new();
        merge_options.fail_on_conflict(true);

        let mut merge = repo.merge_trees(
            &repo.find_tree(workspace.base)?,
            &repo.find_tree(output)?,
            &repo.find_tree(head)?,
            Some(&merge_options),
        )?;

        output = merge.write_tree_to(repo)?;
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
    repo: &git2::Repository,
    gix_repo: &gix::Repository,
    stack: &Stack,
    new_head: git2::Oid,
    ctx: &CommandContext,
) -> Result<BranchHeadAndTree> {
    #[expect(deprecated)]
    compute_updated_branch_head_for_commits(
        repo,
        gix_repo,
        stack.head_oid(&repo.to_gix()?)?.to_git2(),
        stack.tree(ctx)?,
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
    repo: &git2::Repository,
    gix_repo: &gix::Repository,
    old_head: git2::Oid,
    old_tree: git2::Oid,
    new_head: git2::Oid,
) -> Result<BranchHeadAndTree> {
    let (author, committer) = repo.signatures()?;

    let commited_tree = repo.commit_with_signature(
        None,
        &author,
        &committer,
        "Uncommited changes",
        &repo.find_tree(old_tree)?,
        &[&repo.find_commit(old_head)?],
        Default::default(),
    )?;

    let mut rebase = but_rebase::Rebase::new(gix_repo, Some(new_head.to_gix()), None)?;
    rebase.steps(Some(but_rebase::RebaseStep::Pick {
        commit_id: commited_tree.to_gix(),
        new_message: None,
    }))?;
    rebase.rebase_noops(false);
    let output = rebase.rebase()?;
    let rebased_tree = repo.find_commit(output.top_commit.to_git2())?;

    if rebased_tree.is_conflicted() {
        let auto_tree_id = repo.find_real_tree(&rebased_tree, Default::default())?.id();

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
