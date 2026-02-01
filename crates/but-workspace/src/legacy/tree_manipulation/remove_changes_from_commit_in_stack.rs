use anyhow::{Result, bail};
use but_core::{DiffSpec, TreeChange, sync::RepoExclusive};
use but_ctx::Context;
use but_rebase::{Rebase, replace_commit_tree};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gix::ObjectId;

use super::MoveChangesResult;
use crate::legacy::{
    stack_ext::StackExt,
    tree_manipulation::utils::{
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
/// You may want to make use of `update_uncommitted_changes`. Using it will
/// cause the specified change to be dropped from the working directory. Not
/// using it will result in the change showing up as an uncommitted change.
///
/// ## Assumptions
///
/// Currently this function does not take into consideration the possibility
/// that the commit _might_ be part of two different stacks. As such, the
/// other stacks may end up referring to stale commits and potentially cause
/// a merge conflict when combining them in the workspace.
pub fn remove_changes_from_commit_in_stack(
    ctx: &mut Context,
    source_stack_id: StackId,
    source_commit_id: gix::ObjectId,
    changes: impl IntoIterator<Item = DiffSpec>,
    perm: &mut RepoExclusive,
) -> Result<MoveChangesResult> {
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let source_stack = vb_state.get_stack(source_stack_id)?;

    let rewritten_source_commit = remove_changes_from_commit(ctx, source_commit_id, changes, perm)?;

    let mut steps = source_stack.as_rebase_steps(ctx)?;
    replace_pick_with_commit(&mut steps, source_commit_id, rewritten_source_commit)?;
    let base = source_stack.merge_base(ctx)?;

    let result = {
        let repo = ctx.repo.get()?;
        let mut rebase = Rebase::new(&repo, base, None)?;
        rebase.steps(steps)?;
        rebase.rebase_noops(false);
        rebase.rebase()?
    };
    let commit_mapping =
        rebase_mapping_with_overrides(&result, [(source_commit_id, rewritten_source_commit)]);

    let mut source_stack = source_stack;
    source_stack.set_heads_from_rebase_output(ctx, result.references)?;

    let meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    ws.refresh_from_head(&repo, &meta)?;

    Ok(MoveChangesResult {
        replaced_commits: commit_mapping.into_iter().collect(),
    })
}

/// Removes the specified changes from a commit.
///
/// This function does not update the stack or the workspace commit. Only generates a new commit
/// that has the specified changes removed.
/// # IMPORTANT: expects the caller to write ws back!
fn remove_changes_from_commit(
    ctx: &Context,
    source_commit_id: gix::ObjectId,
    changes: impl IntoIterator<Item = DiffSpec>,
    _perm: &mut RepoExclusive,
) -> Result<ObjectId> {
    let repo = ctx.repo.get()?;
    let (source_tree_without_changes, rejected_specs) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit {
            id: source_commit_id,
        },
        changes,
        ctx.settings.context_lines,
    )?;

    if !rejected_specs.is_empty() {
        bail!("Failed to remove certain changes");
    }

    let rewritten_source_commit =
        replace_commit_tree(&repo, source_commit_id, source_tree_without_changes)?;
    Ok(rewritten_source_commit)
}

/// Keeps only the specified file changes in a commit, removing all others.
pub(crate) fn keep_only_file_changes_in_commit(
    ctx: &Context,
    source_commit_id: gix::ObjectId,
    file_changes_to_keep: &[String],
    skip_if_empty: bool,
    perm: &mut RepoExclusive,
) -> Result<Option<gix::ObjectId>> {
    let commit_changes = but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(
        &*ctx.repo.get()?,
        source_commit_id,
    )?;
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

    remove_changes_from_commit(ctx, source_commit_id, diff_specs, perm).map(Some)
}

pub(crate) fn remove_file_changes_from_commit(
    ctx: &Context,
    source_commit_id: gix::ObjectId,
    file_changes_to_split_off: &[String],
    skip_if_empty: bool,
    perm: &mut RepoExclusive,
) -> Result<Option<gix::ObjectId>> {
    let commit_changes = but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(
        &*ctx.repo.get()?,
        source_commit_id,
    )?;
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

    remove_changes_from_commit(ctx, source_commit_id, diff_specs, perm).map(Some)
}
