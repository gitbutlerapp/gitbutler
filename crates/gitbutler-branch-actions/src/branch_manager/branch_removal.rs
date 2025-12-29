use anyhow::{Context as _, Result};
use but_core::{DiffSpec, RepositoryExt};
use but_ctx::access::WorktreeWritePermission;
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_cherry_pick::GixRepositoryExt as _;
use gitbutler_oplog::SnapshotExt;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::StackId;
use gitbutler_workspace::workspace_base;
use tracing::instrument;

use super::{BranchManager, checkout_remerged_head};
use crate::{VirtualBranchesExt, r#virtual as vbranch};

impl BranchManager<'_> {
    #[instrument(level = tracing::Level::DEBUG, skip(self, perm), err(Debug))]
    pub fn unapply(
        &self,
        stack_id: StackId,
        perm: &mut WorktreeWritePermission,
        delete_vb_state: bool,
        assigned_diffspec: Vec<DiffSpec>,
        safe_checkout: bool,
    ) -> Result<String> {
        let vb_state = self.ctx.legacy_project.virtual_branches();
        let mut stack = vb_state.get_stack(stack_id)?;

        // We don't want to try unapplying branches which are marked as not in workspace by the new metric
        if !stack.in_workspace {
            return Ok(stack
                .heads
                .first()
                .expect("Stacks always have one branch")
                .full_name()?
                .to_string());
        }

        _ = self.ctx.snapshot_branch_deletion(stack.name.clone(), perm);

        let repo = self.ctx.git2_repo.get()?;

        // Commit any assigned diffspecs if such exist so that it will be part of the unapplied branch.
        if !assigned_diffspec.is_empty()
            && let Some(head) = stack.heads.last().map(|h| h.name.to_string())
        {
            but_workspace::legacy::commit_engine::create_commit_simple(
                self.ctx,
                stack_id,
                None,
                assigned_diffspec,
                "WIP Assignments".to_string(),
                head.to_owned(),
                perm,
            )?;
        }

        // doing this earlier in the flow, in case any of the steps that follow fail
        stack.in_workspace = false;
        vb_state.set_stack(stack.clone())?;

        let gix_repo = self.ctx.clone_repo_for_merging()?;
        if safe_checkout {
            // This reads the just stored data and re-merges it all stacks, excluding the unapplied one.
            let res = checkout_remerged_head(self.ctx, &gix_repo);
            // Undo the removal, stack is still there officially now.
            if res.is_err() {
                stack.in_workspace = true;
                vb_state.set_stack(stack.clone())?;
            }
            res?
        } else {
            // On v3 we want to take the `current_wd_tree` and try to extract
            // whatever branch we want to unapply. There are a handful of ways
            // to achieve this, including calculating the inverse diff and
            // applying that.
            //
            // We can however do more or less what `git revert` does, and
            // perform a three-way merge where the `ours` side is the cwdt, the
            // `theirs` side is the workspace root, and the `base` is the head
            // of the branch we want to unapply.
            //
            // In order to handle locked files, I'm going to choose to
            // resolve conflicts in the favor of `ours` (the cwdt) which will
            // keep any locked changes in the cwdt.

            // dump current assignments into a WIP commit
            let merge_options = gix_repo
                .tree_merge_options()?
                .with_file_favor(Some(gix::merge::tree::FileFavor::Ours))
                .with_tree_favor(Some(gix::merge::tree::TreeFavor::Ours));

            let cwdt = repo.create_wd_tree(0)?.id().to_gix();
            let workspace_base = gix_repo
                .find_commit(workspace_base(self.ctx, perm.read_permission())?)?
                .tree_id()?;
            let stack_head =
                gix_repo.find_real_tree(&stack.head_oid(&gix_repo)?, Default::default())?;

            let mut merge = gix_repo.merge_trees(
                stack_head,
                cwdt,
                workspace_base,
                gix_repo.default_merge_labels(),
                merge_options,
            )?;
            let new_workspace_tree_with_worktree_changes =
                repo.find_tree(merge.tree.write()?.to_git2())?;

            repo.checkout_tree_builder(&new_workspace_tree_with_worktree_changes)
                .force()
                .checkout()
                .context("failed to checkout tree")?;
        }

        if delete_vb_state {
            self.ctx.delete_branch_reference(&stack)?;
            vb_state.delete_branch_entry(&stack_id)?;
        }

        vb_state.update_ordering()?;

        vbranch::ensure_selected_for_changes(&vb_state)
            .context("failed to ensure selected for changes")?;

        crate::integration::update_workspace_commit(&vb_state, self.ctx, false)
            .context("failed to update gitbutler workspace")?;

        Ok(stack
            .heads
            .first()
            .expect("Stacks always have one branch")
            .full_name()?
            .to_string())
    }
}
