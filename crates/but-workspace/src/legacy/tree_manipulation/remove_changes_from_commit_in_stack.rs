use anyhow::{Result, bail};
use but_core::TreeChange;
use but_rebase::{Rebase, replace_commit_tree};
use gitbutler_command_context::CommandContext;
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gix::ObjectId;

use super::MoveChangesResult;
use crate::{
    DiffSpec,
    legacy::stack_ext::StackExt,
    legacy::tree_manipulation::utils::{
        ChangesSource, create_tree_without_diff, rebase_mapping_with_overrides,
        replace_pick_with_commit,
    },
};

/// Removes the specified changes from a given commit.
///
/// This only updates the specified stack. After calling you may want to call
/// `update_workspace_commit` such that the workspace commit now contains the
/// updated head of the stack.
///
/// You may want to make use of `update_uncommited_changes`. Using it will
/// cause the specified change to be dropped from the working directory. Not
/// using it will result in the change showing up as an uncommited change.
///
/// ## Assumptions
///
/// Currently this function does not take into consideration the possiblity
/// that the commit _might_ be part of two different stacks. As such, the
/// other stacks may end up referring to stale commits and potentially cause
/// a merge conflict when combining them in the workspace.
pub fn remove_changes_from_commit_in_stack(
    ctx: &CommandContext,
    source_stack_id: StackId,
    source_commit_id: gix::ObjectId,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<MoveChangesResult> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let source_stack = vb_state.get_stack(source_stack_id)?;
    let repository = ctx.gix_repo()?;

    let rewritten_source_commit =
        remove_changes_from_commit(ctx, source_commit_id, changes, context_lines)?;

    let mut steps = source_stack.as_rebase_steps(ctx, &repository)?;
    replace_pick_with_commit(&mut steps, source_commit_id, rewritten_source_commit)?;
    let base = source_stack.merge_base(ctx)?;
    let mut rebase = Rebase::new(&repository, base, None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let result = rebase.rebase()?;
    let commit_mapping =
        rebase_mapping_with_overrides(&result, [(source_commit_id, rewritten_source_commit)]);

    let mut source_stack = source_stack;
    source_stack.set_heads_from_rebase_output(ctx, result.references)?;

    Ok(MoveChangesResult {
        replaced_commits: commit_mapping.into_iter().collect(),
    })
}

/// Removes the specified changes from a commit.
///
/// This function does not update the stack or the workspace commit. Only generates a new commit
/// that has the specified changes removed.
pub fn remove_changes_from_commit(
    ctx: &CommandContext,
    source_commit_id: gix::ObjectId,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<ObjectId> {
    let repository = ctx.gix_repo()?;

    let (source_tree_without_changes, rejected_specs) = create_tree_without_diff(
        &repository,
        ChangesSource::Commit {
            id: source_commit_id,
        },
        changes,
        context_lines,
    )?;

    if !rejected_specs.is_empty() {
        bail!("Failed to remove certain changes");
    }

    let rewritten_source_commit =
        replace_commit_tree(&repository, source_commit_id, source_tree_without_changes)?;
    Ok(rewritten_source_commit)
}

/// Keeps only the specified file changes in a commit, removing all others.
pub fn keep_only_file_changes_in_commit(
    ctx: &CommandContext,
    source_commit_id: gix::ObjectId,
    file_changes_to_keep: &[String],
    context_lines: u32,
    skip_if_empty: bool,
) -> Result<Option<gix::ObjectId>> {
    let repository = ctx.gix_repo()?;
    let commit_changes =
        but_core::diff::ui::commit_changes_by_worktree_dir(&repository, source_commit_id)?;
    let changes_to_remove: Vec<TreeChange> = commit_changes
        .changes
        .clone()
        .into_iter()
        .filter(|change| !file_changes_to_keep.contains(&change.path.to_string()))
        .map(|change| change.into())
        .collect();
    if skip_if_empty && changes_to_remove.len() == commit_changes.changes.len() {
        // If we are skipping if empty and all changes are to be removed, return None
        return Ok(None);
    }

    let diff_specs: Vec<DiffSpec> = changes_to_remove
        .into_iter()
        .map(|change| change.into())
        .collect();

    remove_changes_from_commit(ctx, source_commit_id, diff_specs, context_lines).map(Some)
}

pub fn remove_file_changes_from_commit(
    ctx: &CommandContext,
    source_commit_id: gix::ObjectId,
    file_changes_to_split_off: &[String],
    context_lines: u32,
    skip_if_empty: bool,
) -> Result<Option<gix::ObjectId>> {
    let repository = ctx.gix_repo()?;
    let commit_changes =
        but_core::diff::ui::commit_changes_by_worktree_dir(&repository, source_commit_id)?;
    let changes_to_remove: Vec<TreeChange> = commit_changes
        .changes
        .clone()
        .into_iter()
        .filter(|change| file_changes_to_split_off.contains(&change.path.to_string()))
        .map(|change| change.into())
        .collect();
    if skip_if_empty && changes_to_remove.len() == commit_changes.changes.len() {
        // If we are skipping if empty and all changes are to be removed, return None
        return Ok(None);
    }
    let diff_specs: Vec<DiffSpec> = changes_to_remove
        .into_iter()
        .map(|change| change.into())
        .collect();

    remove_changes_from_commit(ctx, source_commit_id, diff_specs, context_lines).map(Some)
}
