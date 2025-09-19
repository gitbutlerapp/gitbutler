use anyhow::{Context, Result};
use but_graph::virtual_branches_legacy_types::CommitOrChangeId;
use but_rebase::{Rebase, RebaseStep};
use but_workspace::stack_ext::StackExt;
use but_workspace::StackId;
use gitbutler_cherry_pick::GixRepositoryExt;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{StackBranch, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{update_uncommited_changes, WorkspaceState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBranchResult {
    /// The stacks that were deleted as a result of the move.
    /// This happens in the case of moving the last branch out of a stack.
    pub deleted_stacks: Vec<StackId>,
}

pub(crate) fn move_branch(
    ctx: &CommandContext,
    target_stack_id: StackId,
    target_branch_name: &str,
    source_stack_id: StackId,
    subject_branch_name: &str,
    perm: &mut WorktreeWritePermission,
) -> Result<MoveBranchResult> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let repository = ctx.gix_repo()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    // let default_target = vb_state.get_default_target()?;
    // let applied_stacks = vb_state
    //     .list_stacks_in_workspace()
    //     .context("failed to read virtual branches")?;

    let source_stack = vb_state.get_stack_in_workspace(source_stack_id)?;
    let source_merge_base = source_stack.merge_base(ctx)?;

    let destination_stack = vb_state.get_stack_in_workspace(target_stack_id)?;
    let destination_merge_base = destination_stack.merge_base(ctx)?;

    // let workspace_dependencies =
    //     compute_workspace_dependencies(ctx, &default_target.sha, &HashMap::new(), &applied_stacks)?;

    // Extract the branch steps from the source stack and rebase the stack
    let (source_branch_steps, new_source_steps) =
        extract_branch_steps(ctx, &repository, &source_stack, subject_branch_name)?;

    let mut deleted_stacks = Vec::new();

    let mut source_stack = source_stack;
    if new_source_steps.is_empty() {
        vb_state.delete_branch_entry(&source_stack_id)?;
        deleted_stacks.push(source_stack_id);
    } else {
        let mut source_stack_rebase = Rebase::new(&repository, source_merge_base, None)?;
        source_stack_rebase.steps(new_source_steps)?;
        source_stack_rebase.rebase_noops(false);
        let source_rebase_result = source_stack_rebase.rebase()?;
        let new_source_head = repository.find_commit(source_rebase_result.top_commit)?;

        source_stack.remove_branch(ctx, subject_branch_name.to_string())?;

        source_stack.set_stack_head(
            &vb_state,
            &repository,
            new_source_head.id().to_git2(),
            Some(
                repository
                    .find_real_tree(&new_source_head.id(), Default::default())?
                    .to_git2(),
            ),
        )?;

        source_stack.set_heads_from_rebase_output(ctx, source_rebase_result.clone().references)?;
    }

    // Inject the extracted branch steps into the destination stack and rebase the stack
    let new_destination_steps = inject_branch_steps(
        ctx,
        &repository,
        &destination_stack,
        target_branch_name,
        source_branch_steps,
    )?;

    let mut destination_stack_rebase = Rebase::new(&repository, destination_merge_base, None)?;
    destination_stack_rebase.steps(new_destination_steps)?;
    destination_stack_rebase.rebase_noops(false);
    let destination_rebase_result = destination_stack_rebase.rebase()?;
    let new_destination_head = repository.find_commit(destination_rebase_result.top_commit)?;
    let mut destination_stack = destination_stack;

    let target_branch_reference = destination_rebase_result
        .clone()
        .references
        .into_iter()
        .find(|r| r.reference.to_string() == target_branch_name)
        .context("subject branch not found in rebase output")?;

    let target_branch_head = target_branch_reference.commit_id;

    destination_stack.add_series(
        ctx,
        StackBranch::new(
            CommitOrChangeId::CommitId(target_branch_head.to_string()),
            subject_branch_name.to_string(),
            None,
            &repository,
        )?,
        Some(target_branch_name.to_string()),
    )?;

    destination_stack.set_stack_head(
        &vb_state,
        &repository,
        new_destination_head.id().to_git2(),
        Some(
            repository
                .find_real_tree(&new_destination_head.id(), Default::default())?
                .to_git2(),
        ),
    )?;
    destination_stack
        .set_heads_from_rebase_output(ctx, destination_rebase_result.clone().references)?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let _ = update_uncommited_changes(ctx, old_workspace, new_workspace, perm);
    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(MoveBranchResult { deleted_stacks })
}

fn extract_branch_steps(
    ctx: &CommandContext,
    repository: &gix::Repository,
    source_stack: &gitbutler_stack::Stack,
    source_branch_name: &str,
) -> Result<(Vec<RebaseStep>, Vec<RebaseStep>)> {
    let source_steps = source_stack.as_rebase_steps_rev(ctx, repository)?;
    let mut new_source_steps = Vec::new();
    let mut source_branch_steps = Vec::new();
    let mut inside_branch = false;
    let branch_ref = repository
        .try_find_reference(source_branch_name)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Source branch '{}' not found in repository",
                source_branch_name
            )
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
            if *name == source_branch_name {
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

        source_branch_steps.push(step);
    }

    new_source_steps.reverse();
    source_branch_steps.reverse();

    Ok((source_branch_steps, new_source_steps))
}

fn inject_branch_steps(
    ctx: &CommandContext,
    repository: &gix::Repository,
    destination_stack: &gitbutler_stack::Stack,
    destination_branch_name: &str,
    branch_steps: Vec<RebaseStep>,
) -> Result<Vec<RebaseStep>> {
    let destination_steps = destination_stack.as_rebase_steps_rev(ctx, repository)?;
    let mut branch_steps = branch_steps.clone();
    branch_steps.reverse();

    let mut new_destination_steps = Vec::new();
    let branch_ref = repository
        .try_find_reference(destination_branch_name)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Destination branch '{}' not found in repository",
                destination_branch_name
            )
        })?;
    let branch_ref_name = branch_ref.name().to_owned();

    for step in destination_steps {
        if let RebaseStep::Reference(but_core::Reference::Git(name)) = &step {
            if *name == branch_ref_name {
                new_destination_steps.extend(branch_steps.clone());
            }
        }

        if let RebaseStep::Reference(but_core::Reference::Virtual(name)) = &step {
            if *name == destination_branch_name {
                new_destination_steps.extend(branch_steps.clone());
            }
        }

        new_destination_steps.push(step);
    }

    new_destination_steps.reverse();
    Ok(new_destination_steps)
}
