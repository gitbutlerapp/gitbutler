//! Updating the rest of the workspace after the target moved, reusing the upstream-integration
//! flow that `but pull` drives.

use but_ctx::Context;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::Integrated,
    Resolution, ResolutionApproach,
    StackStatuses::{UpToDate, UpdatesRequired},
    UpstreamTreeStatus,
};

use crate::{
    theme::{self, Paint},
    utils::OutputChannel,
};

use super::{
    Landed,
    messaging::{land_headline, print_undo_caveat},
};

/// Reconcile the rest of the workspace after the target moved, and report honestly about what
/// happened — only claiming the branches were updated when the integration actually ran.
#[expect(clippy::too_many_arguments)]
pub(super) async fn reconcile_after_land(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_name: &str,
    target_display: &str,
    update_target_locally: bool,
    landed: &Landed,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let t = theme::get();

    let status =
        but_api::legacy::virtual_branches::upstream_integration_statuses(ctx.to_sync(), None)?;

    match status {
        UpToDate => {
            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "\n{}",
                    t.success
                        .paint(land_headline(landed, branch_name, target_display))
                )?;
            }
            print_undo_caveat(
                out,
                update_target_locally,
                landed,
                push_remote_name,
                target_branch_name,
            )?;
            return Ok(());
        }
        UpdatesRequired {
            worktree_conflicts,
            statuses,
        } => {
            if !worktree_conflicts.is_empty() {
                // The target already moved, but uncommitted changes block rebasing the remaining
                // branches. Be honest: this is a partial success, not "complete".
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "\n{} {}",
                        t.attention
                            .paint(land_headline(landed, branch_name, target_display)),
                        t.attention.paint(
                            "Uncommitted changes prevented updating the remaining branches — \
                             commit or stash them and run `but pull`."
                        )
                    )?;
                }
                print_undo_caveat(
                    out,
                    update_target_locally,
                    landed,
                    push_remote_name,
                    target_branch_name,
                )?;
                return Ok(());
            }

            let mut resolutions = Vec::new();
            let mut rebased_branches: Vec<String> = Vec::new();
            for (maybe_stack_id, status) in &statuses {
                let Some(stack_id) = maybe_stack_id else {
                    continue;
                };
                let approach = if status
                    .branch_statuses
                    .iter()
                    .all(|s| s.status == Integrated)
                    && status.tree_status != UpstreamTreeStatus::Conflicted
                {
                    ResolutionApproach::Delete
                } else {
                    // A Rebase rewrites this stack's commit OIDs onto the new target, so any of its
                    // branches that has a pushed PR is left stale; collect them to warn about below.
                    rebased_branches.extend(status.branch_statuses.iter().map(|s| s.name.clone()));
                    ResolutionApproach::Rebase
                };
                resolutions.push(Resolution {
                    stack_id: *stack_id,
                    approach,
                    delete_integrated_branches: true,
                });
            }

            let reconciled = !resolutions.is_empty();
            if reconciled {
                but_api::legacy::virtual_branches::integrate_upstream(
                    ctx.to_sync(),
                    resolutions,
                    None,
                )
                .await?;
            }

            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "\n{}",
                    t.success
                        .paint(land_headline(landed, branch_name, target_display))
                )?;
                // Only claim the other branches moved when the integration actually ran.
                if reconciled {
                    writeln!(
                        out,
                        "{}",
                        t.hint
                            .paint("Remaining branches were updated onto the target.")
                    )?;
                }
            }
            warn_stale_sibling_prs(ctx, out, &rebased_branches)?;
            print_undo_caveat(
                out,
                update_target_locally,
                landed,
                push_remote_name,
                target_branch_name,
            )?;
        }
    }

    Ok(())
}

/// Warn for each rebased branch that still has an open pull request: its commits were rewritten
/// onto the moved target, so `origin/<branch>` and the PR are now stale. We never auto-force-push
/// a sibling's PR branch — we only tell the user it needs re-pushing.
fn warn_stale_sibling_prs(
    ctx: &Context,
    out: &mut OutputChannel,
    rebased_branches: &[String],
) -> anyhow::Result<()> {
    if rebased_branches.is_empty() {
        return Ok(());
    }
    let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
    let pr_for = |name: &str| -> Option<usize> {
        branches
            .iter()
            .find(|b| b.name.to_string() == name)
            .and_then(|b| b.stack.as_ref())
            .and_then(|stack| stack.pull_requests.get(name).copied())
    };

    let Some(out) = out.for_human() else {
        return Ok(());
    };
    let t = theme::get();
    for name in rebased_branches {
        if let Some(pr) = pr_for(name) {
            writeln!(
                out,
                "{}",
                t.attention.paint(format!(
                    "Branch {name} (PR #{pr}) was rebased onto the new target — its pushed branch \
                     and pull request are now stale. Re-push it to update the PR."
                ))
            )?;
        }
    }
    Ok(())
}
