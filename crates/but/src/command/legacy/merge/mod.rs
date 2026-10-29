//! `but merge <branch>`: land a branch directly onto the target ref (the "avoid pull requests"
//! workflow).
//!
//! The landing itself — fetch, fast-forward or signed merge, push or local ref move, retry on a
//! moved target, and reconcile of the remaining branches — lives in `but_api::land::branch_land` so
//! every client shares it. This module only resolves the branch identifier, derives the display
//! strings for the confirmation prompt, calls the API, and renders the result.
//!
//! A target without an upstream is landed locally: nothing is published and `but undo` reverts
//! it, so there is no confirmation.

mod messaging;

use std::fmt::Write;

use anyhow::{Context as _, bail};
use but_core::ChangeId;
use but_ctx::Context;

use crate::{
    CliId, IdMap,
    theme::{self, Paint},
    utils::OutputChannel,
};

pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_id: &str,
    yes: bool,
    no_ff: bool,
    ff_only: bool,
    whole_stack: bool,
) -> anyhow::Result<Vec<(gix::ObjectId, ChangeId)>> {
    // Resolve the branch identifier and read the target configuration. The checkout guard runs here
    // so the user is never asked to confirm a land that can't happen; the API enforces it again,
    // along with the bottom-segment, conflicted-commit, and triangular-remote guards, before
    // mutating anything.
    let (branch_name, target_display, push_remote_name, target_branch_name, tip) = {
        let mut guard = ctx.exclusive_worktree_access();

        let local_target = {
            let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
            but_api::land::ensure_landable_checkout(&ws)?;
            but_api::land::ensure_supported_target_upstream(&ws.graph.project_meta, &repo)?;
            let project_meta = &ws.graph.project_meta;
            match project_meta.remote_target_ref(&repo)? {
                Some(_) => None,
                None => Some(
                    project_meta
                        .target_ref_or_err()?
                        .as_local_full_name_ref()
                        .shorten()
                        .to_string(),
                ),
            }
        };

        let id_map = IdMap::new_from_context(ctx, guard.read_permission(), None)?;
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
        // The tip the user is asked about: the land refuses if the branch moves before it's
        // decided, as when an agent commits during the prompt or the fetch.
        let tip = (ctx.repo.get()?)
            .try_find_reference(&format!("refs/heads/{branch_name}"))?
            .map(|mut reference| reference.peel_to_id().map(|id| id.detach()))
            .transpose()?;

        // Display strings for the prompt and the final report. The API recomputes the
        // target/remote configuration internally; the CLI only needs these names to describe
        // what's about to happen.
        match local_target {
            Some(local_target) => (branch_name, local_target.clone(), None, local_target, tip),
            None => {
                let base_branch = but_api::legacy::virtual_branches::get_base_branch_data(
                    ctx,
                    guard.write_permission(),
                )?
                .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?;
                let target_branch_name = base_branch.short_name.clone();
                let push_remote_name = if base_branch
                    .push_remote_name
                    .as_ref()
                    .context("Merging without a remote isn't supported yet")?
                    .is_empty()
                {
                    base_branch
                        .remote_name
                        .as_ref()
                        .context("Merging without a remote isn't supported yet")?
                        .clone()
                } else {
                    base_branch
                        .push_remote_name
                        .as_ref()
                        .context("Merging without a remote isn't supported yet")?
                        .clone()
                };
                let target_display = format!("{push_remote_name}/{target_branch_name}");
                (
                    branch_name,
                    target_display,
                    Some(push_remote_name),
                    target_branch_name,
                    tip,
                )
            }
        }
    };

    // Publishing to a remote is hard to reverse, so it is confirmed. A local land is undoable.
    if push_remote_name.is_some() {
        // With --whole-stack the confirmation must disclose everything that will be published,
        // not just the branch the user typed — including commits on segments that no longer have
        // a name. The API re-derives and enforces this; what's gathered here is display.
        let lower = if whole_stack {
            but_api::land::lower_stack(ctx, &branch_name)?
        } else {
            but_api::land::LowerStack::default()
        };

        let attached_prs = messaging::attached_pr_numbers(ctx, &branch_name, &lower.segments)?;
        messaging::confirm_direct_target_update(
            out,
            &branch_name,
            &lower,
            &attached_prs,
            &target_display,
            yes,
        )?;
    }

    // A local land is instant, so only the headline reports it.
    if push_remote_name.is_some() {
        let mut progress = out.progress_channel();
        writeln!(progress, "Landing {branch_name} onto {target_display}...")?;
    }

    let head_before = ctx.repo.get()?.head_name()?;
    let result =
        but_api::land::branch_land(ctx, branch_name.clone(), no_ff, ff_only, whole_stack, tip)?;

    messaging::report_land_result(
        out,
        ctx,
        &result,
        &branch_name,
        &target_display,
        push_remote_name.as_deref(),
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
    Ok(result.workspace.conflicted_commits)
}
