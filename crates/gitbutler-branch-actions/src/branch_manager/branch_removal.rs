use std::path::PathBuf;

use anyhow::{Context, Result};
use git2::Commit;
use gitbutler_branch::{BranchExt, SignaturePurpose};
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_oplog::SnapshotExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_reference::{normalize_branch_name, ReferenceName, Refname};
use gitbutler_repo::RepositoryExt;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{Stack, StackId};
use tracing::instrument;

use super::BranchManager;
use crate::r#virtual as vbranch;
use crate::{
    conflicts::{self},
    get_applied_status,
    hunk::VirtualBranchHunk,
    VirtualBranchesExt,
};

impl BranchManager<'_> {
    // to unapply a branch, we need to write the current tree out, then remove those file changes from the wd
    #[instrument(level = tracing::Level::DEBUG, skip(self, perm), err(Debug))]
    pub fn save_and_unapply(
        &self,
        branch_id: StackId,
        perm: &mut WorktreeWritePermission,
    ) -> Result<ReferenceName> {
        let vb_state = self.ctx.project().virtual_branches();
        let target_commit = self
            .ctx
            .repository()
            .find_commit(vb_state.get_default_target()?.sha)?;

        let mut target_branch = vb_state.get_branch(branch_id)?;

        // Convert the vbranch to a real branch
        let real_branch = self.build_real_branch(&mut target_branch)?;

        self.unapply(branch_id, perm, &target_commit, false)?;

        vb_state.update_ordering()?;

        // Ensure we still have a default target
        vbranch::ensure_selected_for_changes(&vb_state)
            .context("failed to ensure selected for changes")?;

        crate::integration::update_workspace_commit(&vb_state, self.ctx)?;

        real_branch.reference_name()
    }

    #[instrument(level = tracing::Level::DEBUG, skip(self, perm), err(Debug))]
    pub(crate) fn unapply(
        &self,
        branch_id: StackId,
        perm: &mut WorktreeWritePermission,
        target_commit: &Commit,
        delete_vb_state: bool,
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

        let repo = self.ctx.repository();

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
        let final_tree = {
            let _span = tracing::debug_span!(
                "new tree without deleted branch",
                num_branches = applied_statuses.len() - 1
            )
            .entered();
            applied_statuses
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
                            gitbutler_diff::write::hunks_onto_oid(self.ctx, branch.head(), files)?;
                        let branch_tree = repo.find_tree(tree_oid)?;
                        let mut result =
                            repo.merge_trees(&base_tree, &final_tree, &branch_tree, None)?;
                        let final_tree_oid = result.write_tree_to(repo)?;
                        repo.find_tree(final_tree_oid)
                            .context("failed to find tree")
                    },
                )?
        };

        let _span = tracing::debug_span!("checkout final tree").entered();
        // checkout final_tree into the working directory
        repo.checkout_tree_builder(&final_tree)
            .force()
            .remove_untracked()
            .checkout()
            .context("failed to checkout tree")?;

        if delete_vb_state {
            self.ctx.delete_branch_reference(&branch)?;
        }

        vbranch::ensure_selected_for_changes(&vb_state)
            .context("failed to ensure selected for changes")?;

        // If we were conflicting, it means that it was the only branch applied. Since we've now unapplied it we can clear all conflicts
        if conflicts::is_conflicting(self.ctx, None)? {
            conflicts::clear(self.ctx)?;
        }
        crate::integration::update_workspace_commit(&vb_state, self.ctx)
            .context("failed to update gitbutler workspace")?;

        Ok(())
    }
}

impl BranchManager<'_> {
    #[instrument(level = tracing::Level::DEBUG, skip(self, vbranch), err(Debug))]
    fn build_real_branch(&self, vbranch: &mut Stack) -> Result<git2::Branch<'_>> {
        let repo = self.ctx.repository();
        let target_commit = repo.find_commit(vbranch.head())?;
        let branch_name = vbranch.name.clone();
        let branch_name = normalize_branch_name(&branch_name)?;

        let vb_state = self.ctx.project().virtual_branches();
        let branch = repo.branch(&branch_name, &target_commit, true)?;
        vbranch.source_refname = Some(Refname::try_from(&branch)?);
        vb_state.set_branch(vbranch.clone())?;

        self.build_wip_commit(vbranch, &branch)?;

        Ok(branch)
    }

    fn build_wip_commit(
        &self,
        vbranch: &mut Stack,
        branch: &git2::Branch<'_>,
    ) -> Result<Option<git2::Oid>> {
        let repo = self.ctx.repository();

        // Build wip tree as either any uncommitted changes or an empty tree
        let vbranch_wip_tree = repo.find_tree(vbranch.tree)?;
        let vbranch_head_tree = repo.find_commit(vbranch.head())?.tree()?;

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
        let committer = gitbutler_branch::signature(SignaturePurpose::Committer)?;
        let author = gitbutler_branch::signature(SignaturePurpose::Author)?;
        let parent = branch.get().peel_to_commit()?;

        let commit_headers = CommitHeadersV2::new();

        let commit_oid = repo.commit_with_signature(
            Some(&branch.try_into()?),
            &author,
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
