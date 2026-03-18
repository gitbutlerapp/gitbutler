use anyhow::{Context as _, Result};
use bstr::BStr;
use but_core::{DiffSpec, diff::tree_changes};
use but_ctx::Context;
use gitbutler_branch_actions::update_workspace_commit;

use crate::utils::OutputChannel;

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

    but_api::commit::move_changes::commit_move_changes_between_only(
        ctx,
        source_id,
        target_id,
        relevant_changes,
    )?;

    update_workspace_commit(ctx, false)?;

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
    let assign_to = find_stack_id_for_branch(ctx, target_branch)?;

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

    but_api::commit::uncommit_changes::commit_uncommit_changes_only(
        ctx,
        source_id,
        relevant_changes,
        assign_to,
    )?;

    update_workspace_commit(ctx, false)?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Uncommitted changes")?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }

    Ok(())
}

/// Determine which stack contains the target branch, if any.
fn find_stack_id_for_branch(
    ctx: &Context,
    target_branch: Option<&str>,
) -> Result<Option<but_core::Id<'S'>>, anyhow::Error> {
    let (_guard, _, workspace, _) = ctx.workspace_and_db()?;
    let target_branch_full_name = target_branch
        .map(|branch| gix::refs::FullName::try_from(format!("refs/heads/{branch}")))
        .transpose()?;
    let assign_to = target_branch_full_name
        .and_then(|full_name| workspace.find_segment_and_stack_by_refname(full_name.as_ref()))
        .and_then(|(stack, _)| stack.id);
    Ok(assign_to)
}
