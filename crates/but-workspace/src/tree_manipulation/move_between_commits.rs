use std::collections::HashMap;

use anyhow::{Result, bail};
use but_rebase::{Rebase, RebaseStep, replace_commit_tree};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::GixRepositoryExt;
use gitbutler_stack::{StackId, VirtualBranchesHandle};

use crate::{
    DiffSpec,
    stack_ext::StackExt,
    tree_manipulation::function::{ChangesSource, create_tree_without_diff},
};

/// Move changes between to commits.
///
/// The commits may either be in the same branch or two different branches.
///
/// ## Limitations / Assumptions
///
/// Currently this function does not take into consideration the possiblity
/// that the commit _might_ be part of two different stacks. As such, the
/// other stacks may end up referring to stale commits and potentially cause
/// a merge conflict when combining them in the workspace.
///
/// This function updates the stacks in question, but does not touch the working
/// directory. After calling this function on stacks in the workspace, you may
/// need to list_virtual_branches in v2, and in both v2 and v3 call
/// `update_workspace_commit`.
///
/// ## `changes_to_remove_from_source`
///
/// The `DiffSpecs` provided to this function are expected to be the
/// "subtraction" specs, same as what gets provided to the [`crate::discard_workspace_changes`].
/// The tests in `tests/workspace/tree_manipulation/hunks.rs` are great
/// reference.
///
/// ## Theory behind this operation
///
/// This is more of an implementation detail, but I think it's pretty important
/// to explain given that this is both a combination of merges and direct tree
/// edits.
///
/// The naive approach to implement this operation is as follows:
/// 1. Take the diff you want to move and apply the inverse to the source
///    commit.
/// 2. Rebase the branch, updating the source commit to the new tree without
///    the specified changes.
/// 3. Apply the diff to the destination commit, making sure to use use the
///    re-based version if needed
/// 4. Re-rebase the branch with the diff applied to the destination commit.
///
/// This implementation does a three way merge to update the destionation commit. This gives
/// us the potential to handle the case where the patch doesn't apply well to
/// destionation commit well.
pub fn move_changes_between_commits(
    ctx: &CommandContext,
    source_stack_id: StackId,
    source_commit_id: gix::ObjectId,
    destination_stack_id: StackId,
    destination_commit_id: gix::ObjectId,
    changes_to_remove_from_source: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<()> {
    if source_commit_id == destination_commit_id {
        return Ok(());
    }

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let repository = ctx.gix_repo()?;
    let git2_repository = ctx.gix_repo()?;

    let source_commit = repository.find_commit(source_commit_id)?;
    let source_tree_id = source_commit.tree_id()?;

    let (source_tree_without_changes_id, dropped_diffs) = create_tree_without_diff(
        &repository,
        ChangesSource::Commit {
            id: source_commit_id,
        },
        changes_to_remove_from_source,
        context_lines,
    )?;
    if !dropped_diffs.is_empty() {
        bail!("Failed to extract described changes from source commit");
    }

    let source_stack = vb_state.get_stack_in_workspace(source_stack_id)?;
    let mut source_stack_steps = source_stack.as_rebase_steps(ctx, &git2_repository)?;

    for step in &mut source_stack_steps {
        if step.commit_id() != Some(&source_commit_id) {
            continue;
        }
        let RebaseStep::Pick { commit_id, .. } = step else {
            continue;
        };
        *commit_id = replace_commit_tree(&repository, *commit_id, source_tree_without_changes_id)?;
    }

    let mut rebase = Rebase::new(&git2_repository, source_stack.merge_base(ctx)?, None)?;
    rebase.steps(source_stack_steps.clone())?;
    rebase.rebase_noops(false);
    let source_stack_result = rebase.rebase()?;

    let source_stack_mapping = source_stack_result
        .commit_mapping
        .into_iter()
        .map(|(_, old, new)| (old, new))
        .collect::<HashMap<_, _>>();

    let destination_commit_id: gix::ObjectId = if source_stack_id == destination_stack_id {
        *source_stack_mapping
            .get(&destination_commit_id)
            .unwrap_or(&destination_commit_id)
    } else {
        destination_commit_id
    };

    let destination_tree_id = repository.find_commit(destination_commit_id)?.tree_id()?;

    // For now, we shall just fail fast and not worry about creating conflicted commits.
    let (fail_fast_options, conflict_kind) = repository.merge_options_fail_fast()?;
    let mut final_destination = repository.merge_trees(
        source_tree_without_changes_id,
        source_tree_id,
        destination_tree_id,
        Default::default(),
        fail_fast_options,
    )?;
    if final_destination.has_unresolved_conflicts(conflict_kind) {
        bail!("Failed to update destination commit to include the changes");
    }
    let final_destination_tree = final_destination.tree.write()?;

    if source_stack_id == destination_stack_id {
        // We need to rebase the source stack a second time. This loop both
        // updates the steps to consider the first rebase, and also injects the
        // new destination commit's tree.
        for step in &mut source_stack_steps {
            let RebaseStep::Pick { commit_id, .. } = step else {
                continue;
            };

            *commit_id = *source_stack_mapping.get(commit_id).unwrap_or(commit_id);

            if *commit_id == destination_commit_id {
                *commit_id =
                    replace_commit_tree(&repository, *commit_id, final_destination_tree.detach())?;
            }
        }

        let mut rebase = Rebase::new(&git2_repository, source_stack.merge_base(ctx)?, None)?;
        rebase.steps(source_stack_steps.clone())?;
        rebase.rebase_noops(false);
        let result = rebase.rebase()?;
        let mut source_stack = source_stack;
        source_stack.set_heads_from_rebase_output(ctx, result.references)?;
    } else {
        let destination_stack = vb_state.get_stack_in_workspace(destination_stack_id)?;
        let mut destination_stack_steps =
            destination_stack.as_rebase_steps(ctx, &git2_repository)?;

        for step in &mut destination_stack_steps {
            if step.commit_id() != Some(&destination_commit_id) {
                continue;
            }
            let RebaseStep::Pick { commit_id, .. } = step else {
                continue;
            };
            *commit_id =
                replace_commit_tree(&repository, *commit_id, final_destination_tree.detach())?;
        }

        let mut rebase = Rebase::new(&git2_repository, destination_stack.merge_base(ctx)?, None)?;
        rebase.steps(destination_stack_steps.clone())?;
        rebase.rebase_noops(false);
        let result = rebase.rebase()?;
        let (mut source_stack, mut destination_stack) = (source_stack, destination_stack);
        source_stack.set_heads_from_rebase_output(ctx, source_stack_result.references)?;
        destination_stack.set_heads_from_rebase_output(ctx, result.references)?;
    }

    Ok(())
}
