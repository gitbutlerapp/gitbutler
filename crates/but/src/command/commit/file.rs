use crate::utils::{OutputChannel, diff_specs::DiffSpecBuilder};
use anyhow::Result;
use bstr::BStr;
use bstr::ByteSlice;
use but_core::{DryRun, sync::RepoExclusive};
use but_ctx::Context;

pub fn commited_file_to_another_commit_with_perm(
    ctx: &mut Context,
    path: &BStr,
    source_id: gix::ObjectId,
    target_id: gix::ObjectId,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let relevant_changes = {
        let context_lines = ctx.settings.context_lines;
        let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
        builder.push_changes_from_path_in_commit(path, source_id, "First parent")?;
        builder.into_diff_specs()
    };

    but_api::commit::move_changes::commit_move_changes_between_only_with_perm(
        ctx,
        source_id,
        target_id,
        relevant_changes,
        DryRun::No,
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
    let context_lines = ctx.settings.context_lines;
    let relevant_changes = {
        let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
        builder.push_changes_from_path_in_commit(path, source_id, "First parent")?;
        builder.into_diff_specs()
    };

    but_api::commit::uncommit::commit_uncommit_changes_with_perm(
        ctx,
        source_id,
        relevant_changes.clone(),
        None,
        DryRun::No,
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

/// Refresh the workspace commit when legacy workspace state is available.
/// TODO: remove this as it shouldn't be needed - all the functions it calls use the rebase engine which takes care of that.
fn legacy_update_workspace_commit(_ctx: &mut Context) -> Result<()> {
    #[cfg(feature = "legacy")]
    gitbutler_branch_actions::update_workspace_commit(_ctx, false)?;
    Ok(())
}
