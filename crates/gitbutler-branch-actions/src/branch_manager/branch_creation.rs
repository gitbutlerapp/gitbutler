use anyhow::{Context as _, Result};
use but_core::{RefMetadata, ref_metadata::StackId, worktree::checkout::UncommitedWorktreeChanges};
use but_ctx::access::RepoExclusive;
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
};
use gitbutler_branch::{self, BranchCreateRequest, dedup};
use gitbutler_oplog::SnapshotExt;
use gitbutler_reference::Refname;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::Stack;
use serde::Serialize;
use tracing::instrument;

use super::BranchManager;
use crate::VirtualBranchesExt;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchFromBranchOutcome {
    pub stack_id: StackId,
    pub unapplied_stacks: Vec<StackId>,
    pub unapplied_stacks_short_names: Vec<String>,
}

impl From<(StackId, Vec<StackId>, Vec<String>)> for CreateBranchFromBranchOutcome {
    fn from(
        (stack_id, unapplied_stacks, unapplied_stacks_short_names): (
            StackId,
            Vec<StackId>,
            Vec<String>,
        ),
    ) -> Self {
        Self {
            stack_id,
            unapplied_stacks,
            unapplied_stacks_short_names,
        }
    }
}

impl BranchManager<'_> {
    #[instrument(level = "debug", skip(self, perm), err(Debug))]
    pub fn create_virtual_branch(
        &self,
        create: &BranchCreateRequest,
        perm: &mut RepoExclusive,
    ) -> Result<Stack> {
        let mut vb_state = self.ctx.virtual_branches();
        let target_base_oid = self.ctx.project_meta()?.target_commit_id_or_err()?;

        let mut all_stacks = vb_state
            .list_stacks_in_workspace()
            .context("failed to read virtual branches")?;

        let stack_names: Vec<String> = all_stacks.iter().map(|b| b.name()).collect();
        let stack_name_refs: Vec<&str> = stack_names.iter().map(|s| s.as_str()).collect();
        let name = dedup(
            &stack_name_refs,
            create.name.as_ref().unwrap_or(&"Lane".to_string()),
        );

        _ = self.ctx.snapshot_branch_creation(name.clone(), perm);

        all_stacks.sort_by_key(|branch| branch.order);

        let order = create.order.unwrap_or(vb_state.next_order_index()?);

        // make space for the new branch
        for (i, branch) in all_stacks.iter().enumerate() {
            let mut branch = branch.clone();
            let new_order = if i < order { i } else { i + 1 };
            if branch.order != new_order {
                branch.order = new_order;
                vb_state.set_stack(branch.clone())?;
            }
        }

        let branch = Stack::new_empty(self.ctx, name, target_base_oid, order)?;

        vb_state.set_stack(branch.clone())?;
        self.ctx.add_branch_reference(&branch)?;

        crate::integration::update_workspace_commit_with_vb_state(&vb_state, self.ctx, false)?;

        Ok(branch)
    }

    pub fn create_virtual_branch_from_branch(
        &self,
        target: &Refname,
        pr_number: Option<usize>,
        perm: &mut RepoExclusive,
    ) -> Result<(StackId, Vec<StackId>, Vec<String>)> {
        let branch_name = target
            .branch()
            .expect("always a branch reference")
            .to_string();
        let _ = self.ctx.snapshot_branch_creation(branch_name.clone(), perm);

        // Assume that this is always about 'apply' and hijack the entire method.
        // That way we'd learn what's missing.
        #[expect(deprecated)] // should have no need for this in modern code anymore
        let (mut meta, ws) = self.ctx.workspace_and_meta_from_head(perm)?;
        let repo = self.ctx.repo.get()?;

        let target_ref = target.to_string();
        let branch_to_apply = target_ref.as_str().try_into()?;
        let mut out = but_workspace::branch::apply(
            branch_to_apply,
            &ws,
            &repo,
            &mut meta,
            but_workspace::branch::apply::Options {
                workspace_merge: WorkspaceMerge::AlwaysMerge,
                on_workspace_conflict:
                    OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
                workspace_reference_naming: WorkspaceReferenceNaming::Default,
                uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                order: None,
                new_stack_id: None,
            },
        )?;
        let ws = out.workspace.into_owned();
        let applied_branch_ref = out
            .applied_branches
            .pop()
            .context("BUG: must mention the actually applied branch last")?;
        let applied_branch_stack_id = ws
            .find_segment_and_stack_by_refname(applied_branch_ref.as_ref())
            .context(
                "BUG: Can't find the branch to apply in workspace, but the 'apply' function should have failed instead",
            )?
            .0
            .id
            .context("BUG: newly applied stacks should always have a stack id")?;
        let conflicted_stack_ids = out
            .conflicting_stacks
            .iter()
            .map(|stack| stack.id)
            .collect::<Vec<_>>();
        let conflicted_stack_short_names_for_display = out
            .conflicting_stacks
            .iter()
            .map(|stack| stack.ref_name.shorten().to_string())
            .collect::<Vec<_>>();
        if let Some(pr_number) = pr_number {
            let mut branch = meta.branch(applied_branch_ref.as_ref())?;
            branch.review.pull_request = Some(pr_number);
            meta.set_branch(&branch)?;
        }
        Ok((
            applied_branch_stack_id,
            conflicted_stack_ids,
            conflicted_stack_short_names_for_display,
        ))
    }
}
