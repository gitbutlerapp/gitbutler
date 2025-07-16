use anyhow::Result;
use but_core::Reference;
use but_rebase::Rebase;
use but_rebase::RebaseStep;
use but_rebase::ReferenceSpec;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_oxidize::OidExt;
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gitbutler_stack::{StackId, VirtualBranchesHandle};

use crate::MoveChangesResult;
use crate::stack_ext::StackExt;
use crate::tree_manipulation::remove_changes_from_commit_in_stack::keep_only_file_changes_in_commit;
use crate::tree_manipulation::remove_changes_from_commit_in_stack::remove_file_changes_from_commit;

/// Splits a branch by creating a new branch with the specified changes.
///
/// This function creates a new branch from the specified source branch
/// and applies the specified changes to it. The new branch will contain only
/// the changes specified in `file_changes_to_split_off`, effectively removing
/// those changes from the source branch.
///
///
/// In steps:
/// 1. Create a new branch from the source branch's head.
/// 2. Remove all the specified changes from the source branch.
/// 3. Remove all but the specified changes from the new branch.
/// 4. Create a stack from out of the new branch.
pub fn split_branch(
    ctx: &CommandContext,
    stack_id: StackId,
    source_branch_name: String,
    new_branch_name: String,
    file_changes_to_split_off: &[String],
    context_lines: u32,
) -> Result<(ReferenceSpec, MoveChangesResult)> {
    let repository = ctx.gix_repo()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    let source_stack = vb_state.get_stack_in_workspace(stack_id)?;
    let merge_base = source_stack.merge_base(ctx)?;

    let push_details = source_stack.push_details(ctx, source_branch_name.clone())?;
    let branch_head = push_details.head;

    // Create a new branch from the source branch's head
    let new_branch_ref_name = format!("refs/heads/{}", new_branch_name);
    let new_branch_log_message = format!(
        "Split off changes from branch '{}' into new branch '{}'",
        source_branch_name, new_branch_name
    );

    let mut new_ref = repository.reference(
        new_branch_ref_name.clone(),
        branch_head.to_gix(),
        gix::refs::transaction::PreviousValue::Any,
        new_branch_log_message.clone(),
    )?;

    // Remove all the specified changes from the source branch, dropping empty rewritten commits
    let source_result = filter_file_changes_in_branch(
        ctx,
        &repository,
        file_changes_to_split_off,
        source_stack,
        source_branch_name,
        merge_base,
        context_lines,
    )?;

    let replaced_commits = source_result
        .commit_mapping
        .iter()
        .filter(|(_, old, new)| old != new)
        .map(|(_, old, new)| (*old, *new))
        .collect();

    let move_changes_result = MoveChangesResult { replaced_commits };

    // Remove all but the specified changes from the new branch
    let new_branch_commits =
        ctx.repo()
            .l(branch_head, LogUntil::Commit(merge_base.to_git2()), false)?;

    // Branch as rebase steps
    let mut steps: Vec<RebaseStep> = Vec::new();

    let reference_step = RebaseStep::Reference(but_core::Reference::Git(new_ref.name().to_owned()));
    steps.push(reference_step);

    for commit in new_branch_commits {
        let commit_id = commit.to_gix();
        if let Some(new_commit_id) = keep_only_file_changes_in_commit(
            ctx,
            commit_id,
            file_changes_to_split_off,
            context_lines,
            true,
        )? {
            let pick_step = RebaseStep::Pick {
                commit_id: new_commit_id,
                new_message: None,
            };
            steps.push(pick_step);
        }
    }
    steps.reverse();

    let mut rebase = Rebase::new(&repository, merge_base, None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let result = rebase.rebase()?;

    let new_branch_full_name: gix::refs::FullName = new_branch_ref_name.try_into()?;

    let new_branch_ref = result
        .references
        .into_iter()
        .find(|r| match r.reference.clone() {
            Reference::Git(full_name) => full_name == new_branch_full_name,
            Reference::Virtual(name) => name == new_branch_name,
        })
        .ok_or_else(|| anyhow::anyhow!("New branch reference not found in rebase output"))?;

    new_ref.set_target_id(new_branch_ref.commit_id, new_branch_log_message)?;

    Ok((new_branch_ref, move_changes_result))
}

/// Filters out the specified file changes from the branch.
///
/// All commits that end up empty after removing the specified file changes will be dropped.
fn filter_file_changes_in_branch(
    ctx: &CommandContext,
    repository: &gix::Repository,
    file_changes_to_split_off: &[String],
    source_stack: gitbutler_stack::Stack,
    source_branch_name: String,
    merge_base: gix::ObjectId,
    context_lines: u32,
) -> Result<but_rebase::RebaseOutput, anyhow::Error> {
    let source_steps = source_stack.as_rebase_steps_rev(ctx, repository)?;
    let mut new_source_steps = Vec::new();
    let mut inside_branch = false;
    let branch_ref = repository
        .try_find_reference(&source_branch_name)?
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

        if let RebaseStep::Pick { commit_id, .. } = &step {
            match remove_file_changes_from_commit(
                ctx,
                *commit_id,
                file_changes_to_split_off,
                context_lines,
                true,
            )? {
                Some(rewritten_commit_id) if *commit_id != rewritten_commit_id => {
                    // Commit was rewritten, add updated step
                    let mut new_step = step.clone();
                    if let RebaseStep::Pick { commit_id, .. } = &mut new_step {
                        *commit_id = rewritten_commit_id;
                    }
                    new_source_steps.push(new_step);
                }
                Some(_) => {
                    // No changes, keep original step
                    new_source_steps.push(step);
                }
                None => {
                    // Commit became empty, drop it
                    // Do nothing
                }
            }
        } else {
            // Not a Pick step, keep as is
            new_source_steps.push(step);
        }
    }

    new_source_steps.reverse();

    let mut source_rebase = Rebase::new(repository, merge_base, None)?;
    source_rebase.steps(new_source_steps)?;
    source_rebase.rebase_noops(false);
    let source_result = source_rebase.rebase()?;

    let mut source_stack = source_stack;
    source_stack.set_heads_from_rebase_output(ctx, source_result.clone().references)?;

    Ok(source_result)
}
