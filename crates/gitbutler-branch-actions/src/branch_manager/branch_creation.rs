use std::collections::HashSet;

use anyhow::{Context as _, Result};
use but_ctx::{Context, access::RepoExclusive};
use gitbutler_branch::{self, BranchCreateRequest, dedup};
use gitbutler_oplog::SnapshotExt;
use gitbutler_stack::Stack;
use tracing::instrument;

use super::BranchManager;
use crate::VirtualBranchesExt;

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

        // Empties must sit on distinct commits so they map to distinct workspace-commit parents —
        // sharing one collapses them into a single lane. Collect the commits already taken as
        // workspace-commit parents (each stack's head). If the target base is already one of them,
        // walk back through its ancestry to the first commit still free. The new branch then sits
        // on an older base; we only log a warning about it (not yet surfaced to the client).
        let repo = self.ctx.repo.get()?;
        // Best-effort: a stack whose head can't be resolved just doesn't reserve a commit here.
        let used_parent_commits: HashSet<gix::ObjectId> = all_stacks
            .iter()
            .filter_map(|s| s.head_oid(self.ctx).ok())
            .collect();
        let mut base_oid = target_base_oid;
        while used_parent_commits.contains(&base_oid) {
            base_oid = repo
                .find_commit(base_oid)?
                .parent_ids()
                .next()
                .context("Not enough commits to create a new branch")?
                .detach();
        }
        if base_oid != target_base_oid {
            tracing::warn!(
                %base_oid,
                "target base commit is already a workspace-commit parent; placing the new branch on an older base"
            );
        }

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

        let branch = Stack::new_empty(self.ctx, name, base_oid, order)?;

        vb_state.set_stack(branch.clone())?;
        ensure_stack_reference(self.ctx, &branch)?;

        crate::integration::update_workspace_commit_with_vb_state(&vb_state, self.ctx, false)?;

        Ok(branch)
    }
}

fn ensure_stack_reference(ctx: &Context, stack: &Stack) -> Result<()> {
    let repo = ctx.repo.get()?;
    let refname = stack.refname()?.to_string();
    let head_oid = stack.head_oid(ctx)?;
    let previous = match repo
        .try_find_reference(&refname)
        .context("failed to lookup reference")?
    {
        Some(reference) => {
            if reference.id() == head_oid {
                return Ok(());
            }
            gix::refs::transaction::PreviousValue::Any
        }
        None => gix::refs::transaction::PreviousValue::MustNotExist,
    };

    let refname: gix::refs::FullName = refname.as_str().try_into()?;
    repo.reference(refname, head_oid, previous, "new vbranch")
        .context("failed to create branch reference")?;

    Ok(())
}
