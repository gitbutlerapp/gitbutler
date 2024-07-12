use crate::{
    conflicts::{self},
    ensure_selected_for_changes, get_applied_status,
    integration::get_integration_commiter,
    write_tree, NameConflitResolution, VirtualBranchesExt,
};
use anyhow::{anyhow, Context, Result};
use git2::build::TreeUpdateBuilder;
use gitbutler_branch::{
    branch::{self, BranchId},
    branch_ext::BranchExt,
};
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_oplog::snapshot::Snapshot;
use gitbutler_reference::ReferenceName;
use gitbutler_reference::{normalize_branch_name, Refname};
use gitbutler_repo::{RepoActions, RepositoryExt};

use super::BranchManager;

impl BranchManager<'_> {
    // to unapply a branch, we need to write the current tree out, then remove those file changes from the wd
    pub fn convert_to_real_branch(
        &self,
        branch_id: BranchId,
        name_conflict_resolution: NameConflitResolution,
    ) -> Result<ReferenceName> {
        let vb_state = self.project_repository.project().virtual_branches();

        let mut target_branch = vb_state.get_branch(branch_id)?;

        // Convert the vbranch to a real branch
        let real_branch = self.build_real_branch(&mut target_branch, name_conflict_resolution)?;

        self.delete_branch(branch_id)?;

        // If we were conflicting, it means that it was the only branch applied. Since we've now unapplied it we can clear all conflicts
        if conflicts::is_conflicting(self.project_repository, None)? {
            conflicts::clear(self.project_repository)?;
        }

        vb_state.update_ordering()?;

        // Ensure we still have a default target
        ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;

        crate::integration::update_gitbutler_integration(&vb_state, self.project_repository)?;

        real_branch.reference_name()
    }

    pub(crate) fn delete_branch(&self, branch_id: BranchId) -> Result<()> {
        let vb_state = self.project_repository.project().virtual_branches();
        let Some(branch) = vb_state.try_branch(branch_id)? else {
            return Ok(());
        };

        // We don't want to try unapplying branches which are marked as not in workspace by the new metric
        if !branch.in_workspace {
            return Ok(());
        }

        _ = self
            .project_repository
            .project()
            .snapshot_branch_deletion(branch.name.clone());

        let repo = self.project_repository.repo();

        let integration_commit = repo.integration_commit()?;
        let target_commit = repo.target_commit()?;
        let base_tree = target_commit.tree().context("failed to get target tree")?;

        let virtual_branches = vb_state
            .list_branches_in_workspace()
            .context("failed to read virtual branches")?;

        let (applied_statuses, _) = get_applied_status(
            self.project_repository,
            &integration_commit.id(),
            virtual_branches,
        )
        .context("failed to get status by branch")?;

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
                    let tree_oid = write_tree(self.project_repository, &branch.head, status.1)?;
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

        vb_state
            .mark_as_not_in_workspace(branch.id)
            .context("Failed to remove branch")?;

        self.project_repository.delete_branch_reference(&branch)?;

        ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;

        Ok(())
    }
}

impl BranchManager<'_> {
    fn build_real_branch(
        &self,
        vbranch: &mut branch::Branch,
        name_conflict_resolution: NameConflitResolution,
    ) -> Result<git2::Branch<'_>> {
        let repo = self.project_repository.repo();
        let target_commit = repo.find_commit(vbranch.head)?;
        let branch_name = vbranch.name.clone();
        let branch_name = normalize_branch_name(&branch_name);

        // Is there a name conflict?
        let branch_name = if repo
            .find_branch(branch_name.as_str(), git2::BranchType::Local)
            .is_ok()
        {
            match name_conflict_resolution {
                NameConflitResolution::Suffix => {
                    let mut suffix = 1;
                    loop {
                        let new_branch_name = format!("{}-{}", branch_name, suffix);
                        if repo
                            .find_branch(new_branch_name.as_str(), git2::BranchType::Local)
                            .is_err()
                        {
                            break new_branch_name;
                        }
                        suffix += 1;
                    }
                }
                NameConflitResolution::Rename(new_name) => {
                    if repo
                        .find_branch(new_name.as_str(), git2::BranchType::Local)
                        .is_ok()
                    {
                        Err(anyhow!("Branch with name {} already exists", new_name))?
                    } else {
                        new_name
                    }
                }
                NameConflitResolution::Overwrite => branch_name,
            }
        } else {
            branch_name
        };

        let vb_state = self.project_repository.project().virtual_branches();
        let branch = repo.branch(&branch_name, &target_commit, true)?;
        vbranch.source_refname = Some(Refname::try_from(&branch)?);
        vb_state.set_branch(vbranch.clone())?;

        self.build_metadata_commit(vbranch, &branch)?;

        Ok(branch)
    }

    fn build_metadata_commit(
        &self,
        vbranch: &mut branch::Branch,
        branch: &git2::Branch<'_>,
    ) -> Result<git2::Oid> {
        let repo = self.project_repository.repo();

        // Build wip tree as either any uncommitted changes or an empty tree
        let vbranch_wip_tree = repo.find_tree(vbranch.tree)?;
        let vbranch_head_tree = repo.find_commit(vbranch.head)?.tree()?;

        let tree = if vbranch_head_tree.id() != vbranch_wip_tree.id() {
            vbranch_wip_tree
        } else {
            repo.find_tree(TreeUpdateBuilder::new().create_updated(repo, &vbranch_head_tree)?)?
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

        let vb_state = self.project_repository.project().virtual_branches();
        // vbranch.head = commit_oid;
        vbranch.not_in_workspace_wip_change_id = Some(commit_headers.change_id);
        vb_state.set_branch(vbranch.clone())?;

        Ok(commit_oid)
    }
}
