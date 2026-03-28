use anyhow::{Context as _, Result};
use but_core::ref_metadata::StackId;
use but_ctx::{Context, access::RepoExclusive};
use but_rebase::{Rebase, RebaseStep};
use but_workspace::legacy::stack_ext::StackExt;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::StackBranch;
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommitted_changes};
use gix::refs::transaction::PreviousValue;
use serde::Serialize;

use crate::{BranchManagerExt, VirtualBranchesExt as _, move_commits::bail_on_new_conflicts};
use anyhow::bail;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBranchResult {
    /// The stacks that were deleted as a result of the move.
    /// This happens in the case of moving the last branch out of a stack.
    pub deleted_stacks: Vec<StackId>,
    /// These are the stacks that were unapplied as a result of the move.
    pub unapplied_stacks: Vec<StackId>,
}

pub(crate) fn move_branch(
    ctx: &mut Context,
    target_stack_id: StackId,
    target_branch_name: &str,
    source_stack_id: StackId,
    subject_branch_name: &str,
    perm: &mut RepoExclusive,
) -> Result<MoveBranchResult> {
    if source_stack_id == target_stack_id {
        bail!("Cannot move a branch within the same stack; use the reorder operation instead.");
    }
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;

    let (source_merge_base, dest_merge_base, source_branch_pr_number) = {
        let vb_state = ctx.virtual_branches();
        let source_stack = vb_state.get_stack_in_workspace(source_stack_id)?;
        let dest_stack = vb_state.get_stack_in_workspace(target_stack_id)?;
        let pr_number = source_stack
            .branches()
            .into_iter()
            .find(|b| b.name == subject_branch_name)
            .context("Subject branch not found in source stack")?
            .pr_number;
        (
            source_stack.merge_base(ctx)?,
            dest_stack.merge_base(ctx)?,
            pr_number,
        )
    };

    // Cross-stack move: compute both rebases (ODB-only writes), check for
    // conflicts, then snapshot, then apply state changes.
    let (source_output, dest_output, source_will_be_deleted) = {
        let repo = ctx.repo.get()?;
        let vb_state = ctx.virtual_branches();
        let source_stack = vb_state.get_stack_in_workspace(source_stack_id)?;
        let dest_stack = vb_state.get_stack_in_workspace(target_stack_id)?;

        let (subject_branch_steps, remaining_steps) =
            extract_branch_steps(ctx, &repo, &source_stack, subject_branch_name)?;

        let source_will_be_deleted = remaining_steps.is_empty();

        // Source: rebase remaining commits without the moved branch (if any remain).
        let source_output = if !source_will_be_deleted {
            let mut src_rebase = Rebase::new(&repo, source_merge_base, None)?;
            src_rebase.steps(remaining_steps)?;
            src_rebase.rebase_noops(false);
            Some(src_rebase.rebase(&*ctx.cache.get_cache()?)?)
        } else {
            None
        };

        // Dest: rebase dest stack with the moved branch injected.
        let new_dest_steps = inject_branch_steps(
            ctx,
            &repo,
            &dest_stack,
            target_branch_name,
            subject_branch_steps,
        )?;
        let mut dst_rebase = Rebase::new(&repo, dest_merge_base, None)?;
        dst_rebase.steps(new_dest_steps)?;
        dst_rebase.rebase_noops(false);
        let dest_output = dst_rebase.rebase(&*ctx.cache.get_cache()?)?;

        // Conflict check — bail before any state is written.
        if let Some(ref src_out) = source_output {
            bail_on_new_conflicts(
                &repo,
                src_out,
                "This move would cause a conflict in the source stack: \
                 other commits depend on the changes being moved.",
            )?;
        }
        bail_on_new_conflicts(
            &repo,
            &dest_output,
            "This move would cause a conflict in the destination stack: \
             the branch does not apply cleanly at the target location.",
        )?;

        (source_output, dest_output, source_will_be_deleted)
    };

    // Snapshot after the conflict check, but before any state writes.
    let _ = ctx.create_snapshot(SnapshotDetails::new(OperationKind::MoveBranch), perm);

    // Apply source changes.
    let mut deleted_stacks = Vec::new();
    {
        let repo = ctx.repo.get()?;
        let mut vb_state = ctx.virtual_branches();
        if source_will_be_deleted {
            vb_state.delete_branch_entry(&source_stack_id)?;
            deleted_stacks.push(source_stack_id);
        } else if let Some(src_out) = source_output {
            let mut source_stack = vb_state.get_stack_in_workspace(source_stack_id)?;
            let new_source_head = repo.find_commit(src_out.top_commit)?;
            source_stack.remove_branch(ctx, subject_branch_name)?;
            source_stack.set_stack_head(&mut vb_state, &repo, new_source_head.id().detach())?;
            source_stack.set_heads_from_rebase_output(ctx, src_out.references)?;
        }
    }

    // Apply dest changes.
    {
        let repo = ctx.repo.get()?;
        let mut vb_state = ctx.virtual_branches();
        let mut destination_stack = vb_state.get_stack_in_workspace(target_stack_id)?;
        let new_destination_head = repo.find_commit(dest_output.top_commit)?;

        // StackBranch::new validates that the supplied commit is within the current stack
        // range. The rebased subject head isn't "in range" yet because the stack head hasn't
        // been updated, so we seed the new branch with the anchor branch's current head as a
        // placeholder. set_heads_from_rebase_output corrects it to the proper commit below.
        let anchor_ref = dest_output
            .references
            .iter()
            .find(|r| r.reference.to_string() == target_branch_name)
            .context("target branch not found in dest rebase output")?;

        let mut new_head =
            StackBranch::new(anchor_ref.commit_id, subject_branch_name.to_string(), &repo)?;
        new_head.pr_number = source_branch_pr_number;

        destination_stack.add_series(ctx, new_head, Some(target_branch_name.to_string()))?;
        destination_stack.set_stack_head(
            &mut vb_state,
            &repo,
            new_destination_head.id().detach(),
        )?;
        destination_stack.set_heads_from_rebase_output(ctx, dest_output.references)?;
    }

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let _ = update_uncommitted_changes(ctx, old_workspace, new_workspace, perm);
    crate::integration::update_workspace_commit_with_vb_state(&ctx.virtual_branches(), ctx, false)
        .context("failed to update gitbutler workspace")?;

    Ok(MoveBranchResult {
        deleted_stacks,
        unapplied_stacks: vec![],
    })
}

/// Tears off a branch from the source stack, creating a new stack for it.
pub(crate) fn tear_off_branch(
    ctx: &Context,
    source_stack_id: StackId,
    subject_branch_name: &str,
    perm: &mut RepoExclusive,
) -> Result<MoveBranchResult> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let repo = ctx.repo.get()?;

    let source_stack = ctx
        .virtual_branches()
        .get_stack_in_workspace(source_stack_id)?;
    let source_merge_base = source_stack.merge_base(ctx)?;

    let (subject_branch_steps, deleted_stacks) = extract_and_rebase_source_branch(
        ctx,
        source_stack_id,
        subject_branch_name,
        &repo,
        source_stack,
        source_merge_base,
    )?;

    // Create a new stack for the torn-off branch
    let mut new_stack_rebase = Rebase::new(&repo, source_merge_base, None)?;
    new_stack_rebase.steps(subject_branch_steps)?;
    new_stack_rebase.rebase_noops(false);
    let new_stack_rebase_output = new_stack_rebase.rebase(&*ctx.cache.get_cache()?)?;

    let subject_branch_reference_spec = new_stack_rebase_output
        .clone()
        .references
        .into_iter()
        .find(|r| r.reference.to_string() == subject_branch_name)
        .context("subject branch not found in rebase output")?;

    let subject_branch_reference_name = format!("refs/heads/{subject_branch_name}");
    repo.reference(
        subject_branch_reference_name.clone(),
        subject_branch_reference_spec.commit_id,
        PreviousValue::Any,
        format!("Creating branch {subject_branch_name}"),
    )?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let _ = update_uncommitted_changes(ctx, old_workspace, new_workspace, perm);
    crate::integration::update_workspace_commit_with_vb_state(&ctx.virtual_branches(), ctx, false)
        .context("failed to update gitbutler workspace")?;

    let branch_manager = ctx.branch_manager();
    let (_, unapplied_stacks, _unapplied_stack_shortnames) = branch_manager
        .create_virtual_branch_from_branch(
            &Refname::Local(LocalRefname::new(subject_branch_name, None)),
            None,
            None,
            perm,
        )?;

    Ok(MoveBranchResult {
        deleted_stacks,
        unapplied_stacks,
    })
}

/// Extracts the steps corresponding to the branch to move, and rebases the source stack without those steps.
fn extract_and_rebase_source_branch(
    ctx: &Context,
    source_stack_id: StackId,
    subject_branch_name: &str,
    repository: &gix::Repository,
    source_stack: gitbutler_stack::Stack,
    source_merge_base: gix::ObjectId,
) -> Result<(Vec<RebaseStep>, Vec<StackId>), anyhow::Error> {
    let (subject_branch_steps, new_source_steps) =
        extract_branch_steps(ctx, repository, &source_stack, subject_branch_name)?;
    let mut deleted_stacks = Vec::new();
    let mut source_stack = source_stack;

    if new_source_steps.is_empty() {
        // If there are no other branches left in the source stack, delete the stack.
        ctx.virtual_branches()
            .delete_branch_entry(&source_stack_id)?;
        deleted_stacks.push(source_stack_id);
    } else {
        // Rebase the source stack without the extracted branch steps
        let mut source_stack_rebase = Rebase::new(repository, source_merge_base, None)?;
        source_stack_rebase.steps(new_source_steps)?;
        source_stack_rebase.rebase_noops(false);
        let source_rebase_result = source_stack_rebase.rebase(&*ctx.cache.get_cache()?)?;
        let new_source_head = repository.find_commit(source_rebase_result.top_commit)?;

        source_stack.remove_branch(ctx, subject_branch_name)?;

        source_stack.set_stack_head(
            &mut ctx.virtual_branches(),
            repository,
            new_source_head.id().detach(),
        )?;

        source_stack.set_heads_from_rebase_output(ctx, source_rebase_result.clone().references)?;
    }
    Ok((subject_branch_steps, deleted_stacks))
}

/// Splits the source stack's rebase steps into two groups: those belonging to
/// `subject_branch_name` and those that remain.
///
/// Steps are partitioned by scanning for a `Reference` marker whose name matches
/// the subject branch (either as a Git ref or a virtual ref). All steps between
/// consecutive Reference markers are considered part of that branch. Returns
/// `(subject_steps, remaining_steps)`, both in execution order (oldest first).
fn extract_branch_steps(
    ctx: &Context,
    repository: &gix::Repository,
    source_stack: &gitbutler_stack::Stack,
    subject_branch_name: &str,
) -> Result<(Vec<RebaseStep>, Vec<RebaseStep>)> {
    let source_steps = source_stack.as_rebase_steps_rev(ctx)?;
    let mut new_source_steps = Vec::new();
    let mut subject_branch_steps = Vec::new();
    let mut inside_branch = false;
    let branch_ref = repository
        .try_find_reference(subject_branch_name)?
        .ok_or_else(|| {
            anyhow::anyhow!("Source branch '{subject_branch_name}' not found in repository")
        })?;
    let branch_ref_name = branch_ref.name().to_owned();

    for step in source_steps {
        if let RebaseStep::Reference(but_core::Reference::Git(name)) = &step {
            if *name == branch_ref_name {
                inside_branch = true;
            } else if inside_branch {
                inside_branch = false;
            }
        }

        if let RebaseStep::Reference(but_core::Reference::Virtual(name)) = &step {
            if *name == subject_branch_name {
                inside_branch = true;
            } else if inside_branch {
                inside_branch = false;
            }
        }

        if !inside_branch {
            // Not inside the source branch, keep the step as is
            new_source_steps.push(step);
            continue;
        }

        subject_branch_steps.push(step);
    }

    new_source_steps.reverse();
    subject_branch_steps.reverse();

    Ok((subject_branch_steps, new_source_steps))
}

fn inject_branch_steps(
    ctx: &Context,
    repository: &gix::Repository,
    destination_stack: &gitbutler_stack::Stack,
    destination_branch_name: &str,
    branch_steps: Vec<RebaseStep>,
) -> Result<Vec<RebaseStep>> {
    let destination_steps = destination_stack.as_rebase_steps_rev(ctx)?;
    let mut branch_steps = branch_steps;
    branch_steps.reverse();

    let mut new_destination_steps = Vec::new();
    let branch_ref = repository
        .try_find_reference(destination_branch_name)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Destination branch '{destination_branch_name}' not found in repository"
            )
        })?;
    let branch_ref_name = branch_ref.name().to_owned();

    for step in destination_steps {
        if let RebaseStep::Reference(but_core::Reference::Git(name)) = &step
            && *name == branch_ref_name
        {
            new_destination_steps.extend(branch_steps.clone());
        }

        if let RebaseStep::Reference(but_core::Reference::Virtual(name)) = &step
            && *name == destination_branch_name
        {
            new_destination_steps.extend(branch_steps.clone());
        }

        new_destination_steps.push(step);
    }

    new_destination_steps.reverse();
    Ok(new_destination_steps)
}
