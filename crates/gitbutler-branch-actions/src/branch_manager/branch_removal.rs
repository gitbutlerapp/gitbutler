use std::path::PathBuf;

use anyhow::{Context, Result};
use gitbutler_branch::{Branch, BranchExt, BranchId};
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_oplog::SnapshotExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_reference::{normalize_branch_name, ReferenceName, Refname};
use gitbutler_repo::{RepoActionsExt, RepositoryExt};

use super::BranchManager;
use crate::{
    conflicts::{self},
    ensure_selected_for_changes, get_applied_status,
    hunk::VirtualBranchHunk,
    integration::get_integration_commiter,
    VirtualBranchesExt,
};

impl BranchManager<'_> {
    // to unapply a branch, we need to write the current tree out, then remove those file changes from the wd
    pub fn convert_to_real_branch(
        &self,
        branch_id: BranchId,
        perm: &mut WorktreeWritePermission,
    ) -> Result<ReferenceName> {
        let vb_state = self.ctx.project().virtual_branches();

        let mut target_branch = vb_state.get_branch(branch_id)?;

        // Convert the vbranch to a real branch
        let real_branch = self.build_real_branch(&mut target_branch)?;

        self.delete_branch(branch_id, perm)?;

        // If we were conflicting, it means that it was the only branch applied. Since we've now unapplied it we can clear all conflicts
        if conflicts::is_conflicting(self.ctx, None)? {
            conflicts::clear(self.ctx)?;
        }

        vb_state.update_ordering()?;

        // Ensure we still have a default target
        ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;

        crate::integration::update_gitbutler_integration(&vb_state, self.ctx)?;

        real_branch.reference_name()
    }

    pub(crate) fn delete_branch(
        &self,
        branch_id: BranchId,
        perm: &mut WorktreeWritePermission,
    ) -> Result<()> {
        let vb_state = self.ctx.project().virtual_branches();
        let Some(branch) = vb_state.try_branch(branch_id)? else {
            return Ok(());
        };

        // We don't want to try unapplying branches which are marked as not in workspace by the new metric
        if !branch.in_workspace {
            return Ok(());
        }

        _ = self
            .ctx
            .project()
            .snapshot_branch_deletion(branch.name.clone(), perm);

        let repo = self.ctx.repo();

        let target_commit = repo.target_commit()?;
        let base_tree = target_commit.tree().context("failed to get target tree")?;

        let applied_statuses = get_applied_status(self.ctx, None)
            .context("failed to get status by branch")?
            .branches;

        // doing this earlier in the flow, in case any of the steps that follow fail
        vb_state
            .mark_as_not_in_workspace(branch.id)
            .context("Failed to remove branch")?;

        // go through the other applied branches and merge them into the final tree
        // then check that out into the working directory
        let final_tree = applied_statuses
            .into_iter()
            .filter(|(branch, _)| branch.id != branch_id)
            .fold(
                target_commit.tree().context("failed to get target tree"),
                |final_tree, status| {
                    let final_tree = final_tree?;
                    let branch = status.0;
                    let files = status
                        .1
                        .into_iter()
                        .map(|file| (file.path, file.hunks))
                        .collect::<Vec<(PathBuf, Vec<VirtualBranchHunk>)>>();
                    let tree_oid =
                        gitbutler_diff::write::hunks_onto_oid(self.ctx, &branch.head, files)?;
                    let branch_tree = repo.find_tree(tree_oid)?;
                    let mut result =
                        repo.merge_trees(&base_tree, &final_tree, &branch_tree, None)?;
                    let final_tree_oid = result.write_tree_to(repo)?;
                    repo.find_tree(final_tree_oid)
                        .context("failed to find tree")
                },
            )?;

        // checkout final_tree into the working directory
        repo.checkout_tree_builder(&final_tree)
            .force()
            .remove_untracked()
            .checkout()
            .context("failed to checkout tree")?;

        self.ctx.delete_branch_reference(&branch)?;

        ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;

        Ok(())
    }
}

impl BranchManager<'_> {
    fn build_real_branch(&self, vbranch: &mut Branch) -> Result<git2::Branch<'_>> {
        let repo = self.ctx.repo();
        let target_commit = repo.find_commit(vbranch.head)?;
        let branch_name = vbranch.name.clone();
        let branch_name = normalize_branch_name(&branch_name);

        let vb_state = self.ctx.project().virtual_branches();
        let branch = repo.branch(&branch_name, &target_commit, true)?;
        vbranch.source_refname = Some(Refname::try_from(&branch)?);
        vb_state.set_branch(vbranch.clone())?;

        self.build_wip_commit(vbranch, &branch)?;

        Ok(branch)
    }

    fn build_wip_commit(
        &self,
        vbranch: &mut Branch,
        branch: &git2::Branch<'_>,
    ) -> Result<Option<git2::Oid>> {
        let repo = self.ctx.repo();

        // Build wip tree as either any uncommitted changes or an empty tree
        let vbranch_wip_tree = repo.find_tree(vbranch.tree)?;
        let vbranch_head_tree = repo.find_commit(vbranch.head)?.tree()?;

        let tree = if vbranch_head_tree.id() != vbranch_wip_tree.id() {
            vbranch_wip_tree
        } else {
            // Don't create the wip commit if not required
            return Ok(None);
        };

        // Build commit message
        let mut message = "GitButler WIP Commit".to_string();
        message.push_str("\n\n");

        // Commit wip commit
        let committer = get_integration_commiter()?;
        let parent = branch.get().peel_to_commit()?;

        let commit_headers = CommitHeadersV2::new();

        let commit_oid = repo.commit_with_signature(
            Some(&branch.try_into()?),
            &committer,
            &committer,
            &message,
            &tree,
            &[&parent],
            Some(commit_headers.clone()),
        )?;

        let vb_state = self.ctx.project().virtual_branches();
        // vbranch.head = commit_oid;
        vbranch.not_in_workspace_wip_change_id = Some(commit_headers.change_id);
        vb_state.set_branch(vbranch.clone())?;

        Ok(Some(commit_oid))
    }
}
