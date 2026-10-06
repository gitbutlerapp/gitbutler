//! `but merge <branch>...`: land one or more branches directly onto the target ref (the "avoid
//! pull requests" workflow).
//!
//! The landing itself — fetch, fast-forward or signed merge, push or local ref move, retry on a
//! moved target, and reconcile of the remaining branches — lives in `but_api::land::branch_land` so
//! every client shares it. This module only resolves the branch identifiers, derives the display
//! strings for the confirmation prompt, calls the API, and renders the result.

pub(crate) mod messaging;

use std::fmt::Write;

use anyhow::bail;
use but_ctx::Context;

use crate::{
    CliId, IdMap,
    theme::{self, Paint},
    utils::OutputChannel,
};

pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_ids: &[String],
    yes: bool,
    no_ff: bool,
    whole_stack: bool,
) -> anyhow::Result<()> {
    // Resolve the branch identifiers and read the target configuration. The checkout guard runs here
    // so the user is never asked to confirm a land that can't happen; the API enforces it again,
    // along with the bottom-segment, conflicted-commit, and triangular-remote guards, before
    // mutating anything.
    let (branch_names, base_branch) = {
        let mut guard = ctx.exclusive_worktree_access();

        {
            let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
            but_api::land::ensure_landable_checkout(&ws, ctx.settings.feature_flags.single_branch)?;
        }

        let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
        let mut branch_names: Vec<String> = Vec::new();
        for branch_id in branch_ids {
            let resolved_ids = id_map.parse_using_context(branch_id, ctx)?;
            if resolved_ids.is_empty() {
                bail!("Could not find branch: {branch_id}");
            }
            if resolved_ids.len() > 1 {
                bail!("Ambiguous branch '{branch_id}', matches multiple items");
            }
            let branch_name = match &resolved_ids[0] {
                CliId::Branch(branch) => branch.name.clone(),
                other => bail!("Expected a branch ID, got {}", other.kind_for_humans()),
            };
            // The same branch may be named twice, e.g. by its CLI ID and its name.
            if !branch_names.contains(&branch_name) {
                branch_names.push(branch_name);
            }
        }

        let base_branch =
            but_api::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())?
                .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?;
        (branch_names, base_branch)
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

    let landings = messaging::landings(ctx, &branch_names, whole_stack)?;
    messaging::confirm_direct_target_update(out, ctx, &landings, &target_display, yes)?;

    {
        let mut progress = out.progress_channel();
        writeln!(
            progress,
            "Landing {} onto {target_display}...",
            branch_names.join(", ")
        )?;
    }

    let head_before = ctx.repo.get()?.head_name()?;
    let result = but_api::land::branch_land(ctx, branch_names.clone(), no_ff, whole_stack)?;

    messaging::report_land_result(
        out,
        ctx,
        &result,
        &branch_names,
        &target_display,
        &push_remote_name,
        &target_branch_name,
    )?;

    // In single-branch mode the reconcile checks out the target (or a generated branch) once the
    // checked-out branch has landed.
    let head_after = ctx.repo.get()?.head_name()?;
    if let Some(out) = out.for_human()
        && let Some(head) = head_after.filter(|head| Some(head) != head_before.as_ref())
    {
        writeln!(
            out,
            "Checked out {}.",
            theme::get().local_branch.paint(head.shorten().to_string())
        )?;
    }
    Ok(())
}
