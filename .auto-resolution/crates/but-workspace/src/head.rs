//! Functions relate to the GitButler workspace head
use anyhow::{Context, Result};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::{
    GixRepositoryExt, ObjectIdExt, OidExt, RepoExt, git2_to_gix_object_id, gix_to_git2_oid,
};
use gitbutler_repo::{RepositoryExt, SignaturePurpose};
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use gix::merge::tree::TreatAsUnresolved;
use tracing::instrument;

const WORKSPACE_HEAD: &str = "Workspace Head";

/// Merges the tree of the workspace with the tree of the worktree, agnostic to which branch HEAD is pointing to
pub fn merge_worktree_with_workspace<'a>(
    ctx: &CommandContext,
    gix_repo: &'a gix::Repository,
) -> Result<(gix::merge::tree::Outcome<'a>, TreatAsUnresolved)> {
    let mut head = gix_repo.head()?;

    // The uncommitted changes
    let workdir_tree = ctx.repo().create_wd_tree(0)?.id().to_gix();

    // The tree of where the gitbutler workspace is at
    let workspace_tree = gix_repo
        .find_commit(super::head(ctx)?.to_gix())?
        .tree_id()?
        .detach();

    let (merge_options_fail_fast, _conflict_kind) =
        gix_repo.merge_options_no_rewrites_fail_fast()?;

    let conflict_kind = TreatAsUnresolved::git();
    let outcome = gix_repo.merge_trees(
        head.peel_to_commit_in_place()?.tree_id()?,
        workdir_tree,
        workspace_tree,
        gix_repo.default_merge_labels(),
        merge_options_fail_fast.with_fail_on_conflict(Some(conflict_kind)),
    )?;
    Ok((outcome, conflict_kind))
}

/// Creates and returns a merge commit of all active branch heads.
///
/// This is the base against which we diff the working directory to understand
/// what files have been modified.
///
/// This should be used to update the `gitbutler/workspace` ref with, which is usually
/// done from [`update_workspace_commit()`], after any of its input changes.
/// This is namely the conflicting state, or any head of the virtual branches.
#[instrument(level = tracing::Level::DEBUG, skip(ctx))]
pub fn head(ctx: &CommandContext) -> Result<git2::Oid> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let target = vb_state
        .get_default_target()
        .context("failed to get target")?;
    let repo: &git2::Repository = ctx.repo();

    let mut stacks: Vec<Stack> = vb_state.list_stacks_in_workspace()?;

    let target_commit = repo.find_commit(target.sha)?;
    let mut workspace_tree = repo.find_real_tree(&target_commit, Default::default())?;
    let mut workspace_tree_id = git2_to_gix_object_id(workspace_tree.id());

    let gix_repo = ctx.gix_repo_for_merging()?;
    let (merge_options_fail_fast, conflict_kind) = gix_repo.merge_options_fail_fast()?;
    let merge_tree_id = git2_to_gix_object_id(repo.find_commit(target.sha)?.tree_id());
    for stack in stacks.iter_mut() {
        stack.migrate_change_ids(ctx).ok(); // If it fails thats ok - best effort migration
        let branch_head = repo.find_commit(stack.head_oid(&gix_repo)?.to_git2())?;
        let branch_tree_id =
            git2_to_gix_object_id(repo.find_real_tree(&branch_head, Default::default())?.id());

        let mut merge = gix_repo.merge_trees(
            merge_tree_id,
            workspace_tree_id,
            branch_tree_id,
            gix_repo.default_merge_labels(),
            merge_options_fail_fast.clone(),
        )?;

        if !merge.has_unresolved_conflicts(conflict_kind) {
            workspace_tree_id = merge.tree.write()?.detach();
        } else {
            // This branch should have already been unapplied during the "update" command but for some reason that failed
            tracing::warn!("Merge conflict between base and {:?}", stack.name);
            stack.in_workspace = false;
            vb_state.set_stack(stack.clone())?;
        }
    }
    workspace_tree = repo.find_tree(gix_to_git2_oid(workspace_tree_id))?;

    let committer = gitbutler_repo::signature(SignaturePurpose::Committer)?;
    let author = gitbutler_repo::signature(SignaturePurpose::Author)?;
    let gix_repo = repo.to_gix()?;
    let mut heads: Vec<git2::Commit<'_>> = stacks
        .iter()
        .filter_map(|stack| stack.head_oid(&gix_repo).ok())
        .filter_map(|h| repo.find_commit(h.to_git2()).ok())
        .collect();

    if heads.is_empty() {
        heads = vec![target_commit]
    }

    // TODO: Why does commit only accept a slice of commits? Feels like we
    //       could make use of AsRef with the right traits.
    let head_refs: Vec<&git2::Commit<'_>> = heads.iter().collect();

    let workspace_head_id = repo.commit(
        None,
        &author,
        &committer,
        WORKSPACE_HEAD,
        &workspace_tree,
        head_refs.as_slice(),
    )?;
    Ok(workspace_head_id)
}
