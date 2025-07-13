use anyhow::Result;
use but_core::Reference;
use but_core::TreeChange;
use but_rebase::Rebase;
use but_rebase::RebaseStep;
use but_rebase::ReferenceSpec;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_oxidize::OidExt;
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gix::ObjectId;

use crate::DiffSpec;
use crate::MoveChangesResult;
use crate::remove_changes_from_commit_in_stack;
use crate::tree_manipulation::remove_changes_from_commit_in_stack::remove_changes_from_commit;

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
) -> Result<(ReferenceSpec, Option<MoveChangesResult>)> {
    let repository = ctx.gix_repo()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    let default_target = vb_state.get_default_target()?;
    let stack = vb_state.get_stack_in_workspace(stack_id)?;
    let merge_base = ctx
        .repo()
        .merge_base(stack.head_oid(&repository)?.to_git2(), default_target.sha)?;

    let push_details = stack.push_details(ctx, source_branch_name.clone())?;
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

    // Remove all the specified changes from the source branch
    let source_branch_commits = ctx
        .repo()
        .l(branch_head, LogUntil::Commit(merge_base), false)?;

    let mut move_changes_result: Option<MoveChangesResult> = None;

    for commit in source_branch_commits {
        let commit_id = commit.to_gix();

        let result = remove_file_changes_from_commit(
            ctx,
            stack_id,
            commit_id,
            file_changes_to_split_off,
            context_lines,
        )?;

        if let Some(ref mut res) = move_changes_result {
            res.merge(result);
        } else {
            move_changes_result = Some(result);
        }
    }

    // Remove all but the specified changes from the new branch
    let new_branch_commits = ctx
        .repo()
        .l(branch_head, LogUntil::Commit(merge_base), false)?;

    // Branch as rebase steps
    let mut steps: Vec<RebaseStep> = Vec::new();

    let reference_step = RebaseStep::Reference(but_core::Reference::Git(new_ref.name().to_owned()));
    steps.push(reference_step);

    for commit in new_branch_commits {
        let commit_id = commit.to_gix();
        let new_commit_id = keep_only_file_changes_in_commit(
            ctx,
            commit_id,
            file_changes_to_split_off,
            context_lines,
        )?;
        let pick_step = RebaseStep::Pick {
            commit_id: new_commit_id,
            new_message: None,
        };
        steps.push(pick_step);
    }
    steps.reverse();

    let mut rebase = Rebase::new(&repository, merge_base.to_gix(), None)?;
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

fn remove_file_changes_from_commit(
    ctx: &CommandContext,
    source_stack_id: StackId,
    source_commit_id: gix::ObjectId,
    file_changes_to_split_off: &[String],
    context_lines: u32,
) -> Result<MoveChangesResult> {
    let repository = ctx.gix_repo()?;
    let commit_changes =
        but_core::diff::ui::commit_changes_by_worktree_dir(&repository, source_commit_id)?;
    let changes_to_remove: Vec<TreeChange> = commit_changes
        .changes
        .into_iter()
        .filter(|change| file_changes_to_split_off.contains(&change.path.to_string()))
        .map(|change| change.into())
        .collect();
    let diff_specs: Vec<DiffSpec> = changes_to_remove
        .into_iter()
        .map(|change| change.into())
        .collect();

    remove_changes_from_commit_in_stack(
        ctx,
        source_stack_id,
        source_commit_id,
        diff_specs,
        context_lines,
    )
}

fn keep_only_file_changes_in_commit(
    ctx: &CommandContext,
    source_commit_id: gix::ObjectId,
    file_changes_to_keep: &[String],
    context_lines: u32,
) -> Result<ObjectId> {
    let repository = ctx.gix_repo()?;
    let commit_changes =
        but_core::diff::ui::commit_changes_by_worktree_dir(&repository, source_commit_id)?;
    let changes_to_remove: Vec<TreeChange> = commit_changes
        .changes
        .into_iter()
        .filter(|change| !file_changes_to_keep.contains(&change.path.to_string()))
        .map(|change| change.into())
        .collect();
    let diff_specs: Vec<DiffSpec> = changes_to_remove
        .into_iter()
        .map(|change| change.into())
        .collect();

    remove_changes_from_commit(ctx, source_commit_id, diff_specs, context_lines)
}
