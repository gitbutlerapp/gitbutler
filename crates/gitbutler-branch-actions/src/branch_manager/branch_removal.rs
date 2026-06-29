use anyhow::{Context as _, Result};
use but_core::{DiffSpec, ref_metadata::StackId};
use but_ctx::access::RepoExclusive;
use but_rebase::graph_rebase::{
    Editor,
    mutate::{InsertSide, RelativeToRef},
};
use gitbutler_repo_actions::RepoActionsExt;
use tracing::instrument;

use super::{BranchManager, checkout_remerged_head};
use crate::VirtualBranchesExt;

impl BranchManager<'_> {
    #[instrument(level = "debug", skip(self, perm), err(Debug))]
    pub fn unapply(
        &self,
        stack_id: StackId,
        perm: &mut RepoExclusive,
        delete_vb_state: bool,
        assigned_diffspec: Vec<DiffSpec>,
    ) -> Result<String> {
        let mut vb_state = self.ctx.virtual_branches();
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

        // Commit any assigned diffspecs if such exist so that it will be part of the unapplied branch.
        if !assigned_diffspec.is_empty()
            && let Some(head) = stack.heads.last()
        {
            let full_ref_name = head.full_name()?;
            let mut meta = self.ctx.meta()?;
            let (repo, mut ws, _) = self.ctx.workspace_mut_and_db_with_perm(perm)?;
            let editor = Editor::create(&mut ws, &mut meta, &repo)?;
            let outcome = but_workspace::commit::commit_create(
                editor,
                assigned_diffspec,
                RelativeToRef::Reference(full_ref_name.as_ref()),
                InsertSide::Below,
                "WIP Assignments",
                self.ctx.settings.context_lines,
            )?;
            if !outcome.rejected_specs.is_empty() {
                tracing::warn!(
                    ?outcome.rejected_specs,
                    "Failed to commit at least one hunk"
                );
            }
            if outcome.commit_selector.is_some() {
                outcome.rebase.materialize()?;
                stack.sync_heads_with_references(&mut vb_state, &repo)?;
            }
        }

        // doing this earlier in the flow, in case any of the steps that follow fail
        stack.in_workspace = false;
        vb_state.set_stack(stack.clone())?;

        let repo = self.ctx.clone_repo_for_merging()?;
        // This reads the just stored data and re-merges it all stacks, excluding the unapplied one.
        let res = checkout_remerged_head(self.ctx, &repo);
        // Undo the removal, stack is still there officially now.
        if res.is_err() {
            stack.in_workspace = true;
            vb_state.set_stack(stack.clone())?;
        }
        res?;

        if delete_vb_state {
            self.ctx.delete_branch_reference(&stack)?;
            vb_state.delete_branch_entry(&stack_id)?;
        }

        vb_state.update_ordering()?;

        crate::integration::update_workspace_commit_with_vb_state(&vb_state, self.ctx, false)
            .context("failed to update gitbutler workspace")?;

        Ok(stack
            .heads
            .first()
            .expect("Stacks always have one branch")
            .full_name()?
            .to_string())
    }
}
