use anyhow::{Context, Result};
use bstr::ByteSlice;
use but_core::diff::tree_changes;
use but_workspace::DiffSpec;
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_command_context::CommandContext;
use gitbutler_stack::VirtualBranchesHandle;

use crate::rub::undo::stack_id_by_commit_id;

pub fn commited_file_to_another_commit(
    ctx: &mut CommandContext,
    path: &str,
    source_id: gix::ObjectId,
    target_id: gix::ObjectId,
) -> Result<()> {
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
    update_workspace_commit(&vb_state, &ctx)?;

    println!("Moved files between commits!");

    Ok(())
}
