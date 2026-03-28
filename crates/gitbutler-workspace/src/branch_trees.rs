use anyhow::Result;
use but_core::RepositoryExt as _;
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_cherry_pick::GixRepositoryExt as _;
use gitbutler_stack::VirtualBranchesHandle;

use crate::{workspace_base, workspace_base_from_heads};

/// A snapshot of the workspace at a point in time.
#[derive(Debug)]
pub struct WorkspaceState {
    /// The heads of the stacks in the workspace.
    heads: Vec<gix::ObjectId>,
    /// The base of the workspace.
    base: gix::ObjectId,
}

impl WorkspaceState {
    pub fn create(ctx: &Context, perm: &RepoShared) -> Result<Self> {
        let repo = &*ctx.repo.get()?;
        let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

        let heads = vb_state
            .list_stacks_in_workspace()?
            .iter()
            .map(|stack| -> Result<gix::ObjectId> {
                let head = stack.head_oid(ctx)?;
                let commit = repo.find_commit(head)?;
                let tree = repo.find_real_tree(&commit, Default::default())?;
                Ok(tree.detach())
            })
            .collect::<Result<Vec<_>>>()?;

        let base = workspace_base(ctx, perm)?;
        let base_tree_id = repo.find_commit(base)?.tree_id()?.detach();

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
        let repo = &*ctx.repo.get()?;

        let base = workspace_base_from_heads(ctx, perm, heads)?;

        let heads = heads
            .iter()
            .map(|head| -> Result<gix::ObjectId> {
                let commit = repo.find_commit(*head)?;
                let tree = repo.find_real_tree(&commit, Default::default())?;
                Ok(tree.detach())
            })
            .collect::<Result<Vec<_>>>()?;

        let base_tree_id = repo.find_commit(base)?.tree_id()?.detach();

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
    let repo = &*ctx.repo.get()?;
    let uncommitted_changes = if ctx.settings.feature_flags.cv3 {
        None
    } else {
        #[expect(deprecated)]
        Some(repo.create_wd_tree(0)?)
    };

    update_uncommitted_changes_with_tree(ctx, old, new, uncommitted_changes, None, perm)
}

/// `old_uncommitted_changes` is `None` if the `safe_checkout` feature is toggled on in `ctx`
pub fn update_uncommitted_changes_with_tree(
    ctx: &Context,
    old: WorkspaceState,
    new: WorkspaceState,
    old_uncommitted_changes: Option<gix::ObjectId>,
    always_checkout: Option<bool>,
    _perm: &mut RepoExclusive,
) -> Result<()> {
    if let Some(worktree_id) = old_uncommitted_changes {
        let gix_repo = ctx.clone_repo_for_merging()?;
        #[expect(deprecated, reason = "checkout/index materialization boundary")]
        let repo = &*ctx.git2_repo.get()?;
        let mut new_uncommitted_changes =
            move_tree_between_workspaces(repo, &gix_repo, worktree_id, &old, &new)?;

        // If the new tree and old tree are the same, then we don't need to do anything
        if !new_uncommitted_changes.has_conflicts() && !always_checkout.unwrap_or(false) {
            let tree = new_uncommitted_changes.write_tree_to(repo)?.to_gix();
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
        let gix_repo = ctx.clone_repo_for_merging()?;
        let old_tree_id = merge_workspace(&gix_repo, &old)?;
        let new_tree_id = merge_workspace(&gix_repo, &new)?;
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
    gix_repo: &gix::Repository,
    tree: gix::ObjectId,
    old: &WorkspaceState,
    new: &WorkspaceState,
) -> Result<git2::Index> {
    let old_workspace = merge_workspace(gix_repo, old)?;
    let new_workspace = merge_workspace(gix_repo, new)?;
    move_tree(repo, tree, old_workspace, new_workspace)
}

/// Cherry pick a tree from one base tree on to another, favoring the contents of the tree when conflicts occur
fn move_tree(
    repo: &git2::Repository,
    tree: gix::ObjectId,
    old_workspace: gix::ObjectId,
    new_workspace: gix::ObjectId,
) -> Result<git2::Index> {
    // Read: Take the diff between old_workspace and tree, and apply it on top
    //   of new_workspace
    let merge = repo.merge_trees(
        &repo.find_tree(old_workspace.to_git2())?,
        &repo.find_tree(tree.to_git2())?,
        &repo.find_tree(new_workspace.to_git2())?,
        None,
    )?;

    Ok(merge)
}

/// Octopus merge using gix, which correctly resolves adjacent-hunk changes that git2 treats as conflicts.
/// Takes N trees and a base tree and merges all the heads together with respect to the given base.
///
/// If there are no heads provided, the base will be returned.
pub fn merge_workspace(
    repo: &gix::Repository,
    workspace: &WorkspaceState,
) -> Result<gix::ObjectId> {
    let mut output = workspace.base;
    let base = workspace.base;

    let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;

    for head in &workspace.heads {
        let mut merge = repo.merge_trees(
            base,
            output,
            *head,
            repo.default_merge_labels(),
            merge_options.clone(),
        )?;

        if merge.has_unresolved_conflicts(conflict_kind) {
            anyhow::bail!("merge conflict when computing workspace tree");
        }
        output = merge.tree.write()?.detach();
    }

    Ok(output)
}

pub fn move_tree_has_conflicts(
    ctx: &Context,
    tree: gix::ObjectId,
    old_workspace: gix::ObjectId,
    new_workspace: gix::ObjectId,
) -> Result<bool> {
    #[expect(deprecated, reason = "tree merge/index materialization boundary")]
    let repo = &*ctx.git2_repo.get()?;
    Ok(move_tree(repo, tree, old_workspace, new_workspace)?.has_conflicts())
}
