use std::collections::HashSet;
use std::io::Write;

use anyhow::{Context, Result};
use bstr::ByteSlice;
use but_core::diff::tree_changes;
use but_hunk_assignment::HunkAssignmentRequest;
use but_workspace::DiffSpec;
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_command_context::CommandContext;
use gitbutler_stack::VirtualBranchesHandle;

use crate::rub::{assign::branch_name_to_stack_id, undo::stack_id_by_commit_id};

pub fn commited_file_to_another_commit(
    ctx: &mut CommandContext,
    path: &str,
    source_id: gix::ObjectId,
    target_id: gix::ObjectId,
) -> Result<()> {
    let mut stdout = std::io::stdout();
    let source_stack = stack_id_by_commit_id(ctx, &source_id)?;
    let target_stack = stack_id_by_commit_id(ctx, &target_id)?;

    let repo = ctx.gix_repo()?;
    let source_commit = repo.find_commit(source_id)?;
    let source_commit_parent_id = source_commit.parent_ids().next().context("First parent")?;

    let (tree_changes, _) = tree_changes(&repo, Some(source_commit_parent_id.detach()), source_id)?;
    let relevant_changes = tree_changes
        .into_iter()
        .filter(|tc| tc.path.to_str_lossy() == path)
        .map(Into::into)
        .collect::<Vec<DiffSpec>>();

    but_workspace::move_changes_between_commits(
        ctx,
        source_stack,
        source_id,
        target_stack,
        target_id,
        relevant_changes,
        ctx.app_settings().context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    update_workspace_commit(&vb_state, ctx, false)?;

    writeln!(stdout, "Moved files between commits!").ok();

    Ok(())
}

pub fn uncommit_file(
    ctx: &mut CommandContext,
    path: &str,
    source_id: gix::ObjectId,
    target_branch: Option<&str>,
) -> Result<()> {
    let mut stdout = std::io::stdout();
    let source_stack = stack_id_by_commit_id(ctx, &source_id)?;

    let repo = ctx.gix_repo()?;

    let source_commit = repo.find_commit(source_id)?;
    let source_commit_parent_id = source_commit.parent_ids().next().context("First parent")?;

    let (tree_changes, _) = tree_changes(&repo, Some(source_commit_parent_id.detach()), source_id)?;
    let relevant_changes = tree_changes
        .into_iter()
        .filter(|tc| tc.path.to_str_lossy() == path)
        .map(Into::into)
        .collect::<Vec<DiffSpec>>();

    // If we want to assign the changes after uncommitting, we could try to do
    // something with the hunk headers, but this is not precise as the hunk
    // headers might have changed from what they were like when they were
    // committed.
    //
    // As such, we take all the old assignments, and all the new assignments from after the
    // uncommit, and find the ones that are not present in the old assignments.
    // We then convert those into assignment requests for the given stack.
    let before_assignments = but_hunk_assignment::assignments_with_fallback(
        ctx,
        false,
        None::<Vec<but_core::TreeChange>>,
        None,
    )?
    .0;

    but_workspace::remove_changes_from_commit_in_stack(
        ctx,
        source_stack,
        source_id,
        relevant_changes,
        ctx.app_settings().context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    update_workspace_commit(&vb_state, ctx, false)?;

    let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        false,
        None::<Vec<but_core::TreeChange>>,
        None,
    )?;

    let before_assignments = before_assignments
        .into_iter()
        .filter_map(|a| a.id)
        .collect::<HashSet<_>>();

    if let Some(target_branch) = target_branch {
        let target_stack = branch_name_to_stack_id(ctx, Some(target_branch))?;
        let to_assign = after_assignments
            .into_iter()
            .filter(|a| a.id.is_some_and(|id| !before_assignments.contains(&id)))
            .map(|a| HunkAssignmentRequest {
                hunk_header: a.hunk_header,
                path_bytes: a.path_bytes,
                stack_id: target_stack,
            })
            .collect::<Vec<_>>();

        but_hunk_assignment::assign(ctx, to_assign, None)?;
    }

    writeln!(stdout, "Uncommitted changes").ok();

    Ok(())
}
