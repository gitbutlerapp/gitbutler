//! `but land <branch>`: land a branch directly onto the target ref (the "avoid pull requests"
//! workflow).
//!
//! The landing itself — fetch, fast-forward or signed merge, push or local ref move, retry on a
//! moved target, and reconcile of the remaining branches — lives in `but_api::land::branch_land` so
//! every client shares it. This module only resolves the branch identifier, derives the display
//! strings for the confirmation prompt, calls the API, and renders the result.

mod messaging;

use std::fmt::Write;

use anyhow::bail;
use but_ctx::Context;

use crate::{CliId, IdMap, utils::OutputChannel};

pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_id: &str,
    yes: bool,
    no_ff: bool,
) -> anyhow::Result<()> {
    // Resolve the branch identifier and read the target configuration. The managed-workspace guard
    // runs here for a friendly message before the prompt; the API enforces it again, along with the
    // bottom-segment, conflicted-commit, and triangular-remote guards, before mutating anything.
    let (branch_name, base_branch) = {
        let mut guard = ctx.exclusive_worktree_access();

        {
            let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
            if !ws.kind.has_managed_ref() {
                bail!(
                    "`but land` requires an active GitButler workspace (`gitbutler/workspace`). \
                     Switch into the workspace and try again."
                );
            }
        }

        let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
        let resolved_ids = id_map.parse_using_context(branch_id, ctx)?;
        if resolved_ids.is_empty() {
            bail!("Could not find branch: {branch_id}");
        }
        if resolved_ids.len() > 1 {
            bail!("Ambiguous branch '{branch_id}', matches multiple items");
        }

        let branch_name = match &resolved_ids[0] {
            CliId::Branch { name, .. } => name.clone(),
            other => bail!("Expected a branch ID, got {}", other.kind_for_humans()),
        };

        let base_branch =
            but_api::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())?
                .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?;
        (branch_name, base_branch)
    };

    // Display strings for the prompt and the final report. The API recomputes the target/remote
    // configuration internally; the CLI only needs these names to describe what's about to happen.
    let target_branch_name = base_branch.short_name.clone();
    let push_remote_name = if base_branch.push_remote_name.is_empty() {
        base_branch.remote_name.clone()
    } else {
        base_branch.push_remote_name.clone()
    };
    let target_display = format!("{push_remote_name}/{target_branch_name}");

    let pr_number = messaging::branch_pr_number(ctx, &branch_name)?;
    messaging::confirm_direct_target_update(out, &branch_name, pr_number, &target_display, yes)?;

    {
        let mut progress = out.progress_channel();
        writeln!(progress, "Landing {branch_name} onto {target_display}...")?;
    }

    let result = but_api::land::branch_land(ctx, branch_name.clone(), no_ff)?;

    messaging::report_land_result(
        out,
        ctx,
        &result,
        &branch_name,
        &target_display,
        &push_remote_name,
        &target_branch_name,
    )
}
