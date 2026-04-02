use crate::utils::OutputChannel;
use anyhow::{Context as _, Result};
use bstr::BStr;
use bstr::ByteSlice;
use but_core::{DiffSpec, diff::tree_changes, sync::RepoExclusive};
use but_ctx::Context;

pub fn commited_file_to_another_commit(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    target_id: gix::ObjectId,
    out: &mut OutputChannel,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    commited_file_to_another_commit_with_perm(
        ctx,
        path,
        source_id,
        target_id,
        out,
        guard.write_permission(),
    )
}

pub fn commited_file_to_another_commit_with_perm(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    target_id: gix::ObjectId,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let relevant_changes = changes_for_path_in_commit(ctx, path, source_id)?;

    but_api::commit::move_changes::commit_move_changes_between_only_with_perm(
        ctx,
        source_id,
        target_id,
        relevant_changes,
        perm,
    )?;

    legacy_update_workspace_commit(ctx)?;

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
    let mut guard = ctx.exclusive_worktree_access();
    uncommit_file_with_perm(
        ctx,
        path,
        source_id,
        target_branch,
        out,
        guard.write_permission(),
    )
}

pub fn uncommit_file_with_perm(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    target_branch: Option<&str>,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> Result<()> {
    // Convert target_branch to StackId if provided (for hunk assignment after uncommit)
    let assign_to = find_stack_id_for_branch_with_perm(ctx, target_branch, perm)?;
    let relevant_changes = changes_for_path_in_commit(ctx, path, source_id)?;

    but_api::commit::uncommit::commit_uncommit_changes_only_with_perm(
        ctx,
        source_id,
        relevant_changes,
        assign_to,
        perm,
    )?;

    legacy_update_workspace_commit(ctx)?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Uncommitted changes")?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }

    Ok(())
}

pub fn uncommit_file_and_discard(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    out: &mut OutputChannel,
    emit_output: bool,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    uncommit_file_and_discard_with_perm(
        ctx,
        path,
        source_id,
        out,
        emit_output,
        guard.write_permission(),
    )
}

pub fn uncommit_file_and_discard_with_perm(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    out: &mut OutputChannel,
    emit_output: bool,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let relevant_changes = changes_for_path_in_commit(ctx, path, source_id)?;

    let context_lines = ctx.settings.context_lines;
    but_api::commit::uncommit::commit_uncommit_changes_with_perm(
        ctx,
        source_id,
        relevant_changes.clone(),
        None,
        perm,
    )?;

    let dropped = {
        let repo = ctx.repo.get()?;
        but_workspace::discard_workspace_changes(&repo, relevant_changes, context_lines)?
    };

    legacy_update_workspace_commit(ctx)?;

    if emit_output {
        if let Some(out) = out.for_human() {
            if !dropped.is_empty() {
                writeln!(
                    out,
                    "Warning: Some changes could not be discarded (possibly already discarded or modified):"
                )?;
                for spec in &dropped {
                    writeln!(out, "  {}", spec.path.as_bstr())?;
                }
            }
            writeln!(out, "Discarded committed changes")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
    }

    Ok(())
}

fn changes_for_path_in_commit(
    ctx: &Context,
    path: &BStr,
    source_id: gix::ObjectId,
) -> Result<Vec<DiffSpec>> {
    let repo = ctx.repo.get()?;

    let source_commit = repo.find_commit(source_id)?;
    let source_commit_parent_id = source_commit.parent_ids().next().context("First parent")?;

    let tree_changes = tree_changes(&repo, Some(source_commit_parent_id.detach()), source_id)?;
    Ok(tree_changes
        .into_iter()
        .filter(|tc| tc.path == path)
        .map(Into::into)
        .collect())
}

fn find_stack_id_for_branch_with_perm(
    ctx: &Context,
    target_branch: Option<&str>,
    perm: &mut RepoExclusive,
) -> Result<Option<but_core::Id<'S'>>, anyhow::Error> {
    let (_, ws, _) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
    let target_branch_full_name = target_branch
        .map(|branch| gix::refs::FullName::try_from(format!("refs/heads/{branch}")))
        .transpose()?;
    let assign_to = target_branch_full_name
        .and_then(|full_name| ws.find_segment_and_stack_by_refname(full_name.as_ref()))
        .and_then(|(stack, _)| stack.id);
    Ok(assign_to)
}

/// Refresh the workspace commit when legacy workspace state is available.
/// TODO: remove this as it shouldn't be needed - all the functions it calls use the rebase engine which takes care of that.
fn legacy_update_workspace_commit(_ctx: &mut Context) -> Result<()> {
    #[cfg(feature = "legacy")]
    gitbutler_branch_actions::update_workspace_commit(_ctx, false)?;
    Ok(())
}
