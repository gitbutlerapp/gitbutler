//! Functions relate to the GitButler workspace head

use anyhow::{Context as _, Result, bail};
use but_core::RepositoryExt;
use but_ctx::Context;
use gitbutler_repo::{SignaturePurpose, commit_without_signature_gix, signature_gix};
use gix::merge::tree::TreatAsUnresolved;
use tracing::instrument;

const WORKSPACE_HEAD: &str = "Workspace Head";

/// Merges the tree of the workspace with the tree of the worktree, agnostic to which branch HEAD is pointing to
pub fn merge_worktree_with_workspace<'a>(
    ctx: &Context,
    gix_repo: &'a gix::Repository,
    ws: &but_graph::Workspace,
) -> Result<(gix::merge::tree::Outcome<'a>, TreatAsUnresolved)> {
    let mut head = gix_repo.head()?;

    // The uncommitted changes
    #[expect(deprecated)]
    let workdir_tree = gix_repo.create_wd_tree(0)?;

    // The tree of where the gitbutler workspace is at
    let workspace_tree = gix_repo
        .find_commit(super::remerged_workspace_commit_v2(ctx, ws)?)?
        .tree_id()?
        .detach();

    let (merge_options_fail_fast, _conflict_kind) =
        gix_repo.merge_options_no_rewrites_fail_fast()?;

    let conflict_kind = TreatAsUnresolved::git();
    let outcome = gix_repo.merge_trees(
        head.peel_to_commit()?.tree_id()?,
        workdir_tree,
        workspace_tree,
        gix_repo.default_merge_labels(),
        merge_options_fail_fast.with_fail_on_conflict(Some(conflict_kind)),
    )?;
    Ok((outcome, conflict_kind))
}

/// Merge the projected workspace stacks into a new tree and return
/// `(merged_tree, stack_heads, target_commit)`.
/// `gix_repo` should be optimised for merging.
pub fn remerged_workspace_tree_v2(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
) -> Result<(gix::ObjectId, Vec<gix::ObjectId>, gix::ObjectId)> {
    let target_base_oid = ws
        .stored_target_commit_id()
        .context("failed to get target base oid")?;
    let heads = ws
        .stacks
        .iter()
        .map(|stack| stack.tip_skip_empty().unwrap_or(target_base_oid))
        .collect::<Vec<_>>();
    let workspace_tree_id = if heads.is_empty() {
        but_core::Commit::try_from(repo.find_commit(target_base_oid)?)?
            .tree_id_or_auto_resolution()?
            .detach()
    } else if heads.len() == 1 {
        let commit = but_core::Commit::try_from(
            repo.find_commit(*heads.first().expect("Heads is length 1"))?,
        )?;
        commit.tree_id_or_auto_resolution()?.detach()
    } else {
        let base_tree_id = but_core::Commit::try_from(
            repo.find_commit(repo.merge_base_octopus(heads.iter().copied())?)?,
        )?
        .tree_id_or_auto_resolution()?
        .detach();
        let mut workspace_tree_id = base_tree_id;

        let (merge_options_fail_fast, conflict_kind) = repo.merge_options_fail_fast()?;
        for head in &heads {
            let stack_head = but_core::Commit::try_from(repo.find_commit(*head)?)?;
            let branch_tree_id = stack_head.tree_id_or_auto_resolution()?.detach();

            let mut merge = repo.merge_trees(
                base_tree_id,
                workspace_tree_id,
                branch_tree_id,
                repo.default_merge_labels(),
                merge_options_fail_fast.clone(),
            )?;

            if !merge.has_unresolved_conflicts(conflict_kind) {
                workspace_tree_id = merge.tree.write()?.detach();
            } else {
                bail!(
                    "BUG: Merge conflict between projected workspace stacks: This branch should have already been unapplied during the 'update' command but for some reason that failed"
                );
            }
        }

        workspace_tree_id
    };

    Ok((workspace_tree_id, heads, target_base_oid))
}

/// Creates and returns a merge commit of all active branch heads.
///
/// This is the base against which we diff the working directory to understand
/// what files have been modified.
///
/// This should be used to update the `gitbutler/workspace` ref with, which is usually
/// done from `update_workspace_commit()`, after any of its input changes.
/// This is namely the conflicting state, or any head of the virtual branches.
#[instrument(level = "debug", skip(ctx, ws))]
pub fn remerged_workspace_commit_v2(
    ctx: &Context,
    ws: &but_graph::Workspace,
) -> Result<gix::ObjectId> {
    let repo = ctx.clone_repo_for_merging()?;
    let (workspace_tree_id, mut heads, target_commit) = remerged_workspace_tree_v2(&repo, ws)?;

    let committer = signature_gix(SignaturePurpose::Committer);
    let author = signature_gix(SignaturePurpose::Author);
    if heads.is_empty() {
        heads = vec![target_commit]
    }

    let workspace_head_id = commit_without_signature_gix(
        &repo,
        None,
        author,
        committer,
        WORKSPACE_HEAD.into(),
        workspace_tree_id,
        &heads,
        None,
    )?;
    Ok(workspace_head_id)
}
