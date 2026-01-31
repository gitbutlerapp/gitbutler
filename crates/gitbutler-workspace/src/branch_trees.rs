use anyhow::Result;
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_cherry_pick::RepositoryExt;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_stack::VirtualBranchesHandle;

use crate::{workspace_base, workspace_base_from_heads};

/// A snapshot of the workspace at a point in time.
#[derive(Debug)]
pub struct WorkspaceState {
    /// The heads of the stacks in the workspace.
    heads: Vec<git2::Oid>,
    /// The base of the workspace.
    base: git2::Oid,
}

impl WorkspaceState {
    pub fn create(ctx: &Context, perm: &RepoShared) -> Result<Self> {
        let repo = &*ctx.git2_repo.get()?;
        let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

        let heads = vb_state
            .list_stacks_in_workspace()?
            .iter()
            .map(|stack| -> Result<git2::Oid> {
                let head = stack.head_oid(ctx)?.to_git2();
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

    pub fn create_from_heads(
        ctx: &Context,
        perm: &RepoShared,
        heads: &[gix::ObjectId],
    ) -> Result<Self> {
        let repo = &*ctx.git2_repo.get()?;

        let base = workspace_base_from_heads(ctx, perm, heads)?;

        let heads = heads
            .iter()
            .map(|head| -> Result<git2::Oid> {
                let commit = repo.find_commit(head.to_git2())?;
                let tree = repo.find_real_tree(&commit, Default::default())?;
                Ok(tree.id())
            })
            .collect::<Result<Vec<_>>>()?;

        let base_tree_id = repo.find_commit(base.to_git2())?.tree_id();

        Ok(WorkspaceState {
            heads,
            base: base_tree_id,
        })
    }
}

/// Update the uncommitted changes from one snapshot of the workspace and rebase
/// them on top of the new snapshot.
pub fn update_uncommitted_changes(
    ctx: &Context,
    old: WorkspaceState,
    new: WorkspaceState,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;
    let uncommitted_changes = (!ctx.settings.feature_flags.cv3)
        .then(|| repo.create_wd_tree(0).map(|tree| tree.id()))
        .transpose()?;

    update_uncommitted_changes_with_tree(ctx, old, new, uncommitted_changes, None, perm)
}

/// `old_uncommitted_changes` is `None` if the `safe_checkout` feature is toggled on in `ctx`
pub fn update_uncommitted_changes_with_tree(
    ctx: &Context,
    old: WorkspaceState,
    new: WorkspaceState,
    old_uncommitted_changes: Option<git2::Oid>,
    always_checkout: Option<bool>,
    _perm: &mut RepoExclusive,
) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;
    if let Some(worktree_id) = old_uncommitted_changes {
        let mut new_uncommitted_changes =
            move_tree_between_workspaces(repo, worktree_id, old, new)?;

        // If the new tree and old tree are the same, then we don't need to do anything
        if !new_uncommitted_changes.has_conflicts() && !always_checkout.unwrap_or(false) {
            let tree = new_uncommitted_changes.write_tree_to(repo)?;
            if tree == worktree_id {
                return Ok(());
            }
        }

        repo.checkout_index(
            Some(&mut new_uncommitted_changes),
            Some(
                git2::build::CheckoutBuilder::new()
                    .force()
                    .remove_untracked(true)
                    .conflict_style_diff3(true),
            ),
        )?;
    } else {
        let old_tree_id = merge_workspace(repo, old)?.to_gix();
        let new_tree_id = merge_workspace(repo, new)?.to_gix();
        let gix_repo = ctx.clone_repo_for_merging()?;
        but_core::worktree::safe_checkout(
            old_tree_id,
            new_tree_id,
            &gix_repo,
            but_core::worktree::checkout::Options::default(),
        )?;
    }
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
pub fn move_tree(
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
pub fn merge_workspace(repo: &git2::Repository, workspace: WorkspaceState) -> Result<git2::Oid> {
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
