use anyhow::{Context as _, Result};
use bstr::BStr;
use but_core::{DiffSpec, diff::tree_changes};
use but_ctx::Context;
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_stack::VirtualBranchesHandle;

use crate::{command::legacy::rub::assign::branch_name_to_stack_id, utils::OutputChannel};

pub fn commited_file_to_another_commit(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    target_id: gix::ObjectId,
    out: &mut OutputChannel,
) -> Result<()> {
    let relevant_changes = {
        let repo = ctx.repo.get()?;
        let source_commit = repo.find_commit(source_id)?;
        let source_commit_parent_id = source_commit.parent_ids().next().context("First parent")?;

        let tree_changes = tree_changes(&repo, Some(source_commit_parent_id.detach()), source_id)?;
        tree_changes
            .into_iter()
            .filter(|tc| tc.path == path)
            .map(Into::into)
            .collect::<Vec<DiffSpec>>()
    };

    but_api::commit::commit_move_changes_between_only(ctx, source_id.into(), target_id.into(), relevant_changes)?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    update_workspace_commit(&vb_state, ctx, false)?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Moved files between commits!")?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }

    Ok(())
}

pub fn uncommit_file(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    target_branch: Option<&str>,
    out: &mut OutputChannel,
) -> Result<()> {
    // Convert target_branch to StackId if provided (for hunk assignment after uncommit)
    let assign_to = target_branch
        .map(|branch| branch_name_to_stack_id(ctx, Some(branch)))
        .transpose()?
        .flatten();

    let relevant_changes = {
        let repo = ctx.repo.get()?;

        let source_commit = repo.find_commit(source_id)?;
        let source_commit_parent_id = source_commit.parent_ids().next().context("First parent")?;

        let tree_changes = tree_changes(&repo, Some(source_commit_parent_id.detach()), source_id)?;
        tree_changes
            .into_iter()
            .filter(|tc| tc.path == path)
            .map(Into::into)
            .collect::<Vec<DiffSpec>>()
    };

    but_api::commit::commit_uncommit_changes_only(ctx, source_id.into(), relevant_changes, assign_to)?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    update_workspace_commit(&vb_state, ctx, false)?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Uncommitted changes")?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }

    Ok(())
}
