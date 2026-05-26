use crate::utils::OutputChannel;
use anyhow::{Context as _, Result};
use bstr::BStr;
use but_core::{DiffSpec, DryRun, diff::tree_changes, sync::RepoExclusive};
use but_ctx::Context;

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

/// Refresh the workspace commit when legacy workspace state is available.
/// TODO: remove this as it shouldn't be needed - all the functions it calls use the rebase engine which takes care of that.
fn legacy_update_workspace_commit(_ctx: &mut Context) -> Result<()> {
    #[cfg(feature = "legacy")]
    gitbutler_branch_actions::update_workspace_commit(_ctx, false)?;
    Ok(())
}
