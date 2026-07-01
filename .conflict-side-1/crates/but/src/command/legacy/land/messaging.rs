//! Confirmation prompt and end-of-command reporting, kept honest about what actually happened.

use anyhow::bail;
use but_api::land::{BranchLandKind, BranchLandResult};
use but_ctx::Context;

use crate::{
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

/// Render the outcome of a land: the headline, a note when the remaining branches were left
/// un-reconciled, stale pull-request warnings for rebased siblings, and the undo caveat.
pub(super) fn report_land_result(
    out: &mut OutputChannel,
    ctx: &Context,
    result: &BranchLandResult,
    branch_name: &str,
    target_display: &str,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let t = theme::get();

    if let Some(out) = out.for_human() {
        let headline = match result.landed {
            BranchLandKind::AlreadyIntegrated => {
                format!("{branch_name} was already on {target_display}.")
            }
            BranchLandKind::Updated { .. } => {
                format!("Landed {branch_name} onto {target_display}.")
            }
        };
        let style = if result.reconcile_skipped {
            t.attention
        } else {
            t.success
        };
        writeln!(out, "\n{}", style.paint(headline))?;
    }

    if result.reconcile_skipped {
        if let Some(out) = out.for_human() {
            // One message for both skip causes (the fetch tracking ref hadn't caught up, or
            // uncommitted changes blocked the rebase). Both are resolved by a later `but pull`, so
            // we don't assert a cause the user may not have.
            writeln!(
                out,
                "{}",
                t.attention.paint(
                    "The remaining branches were not updated onto the new target. Run `but pull` \
                     to finish."
                )
            )?;
        }
    } else {
        warn_stale_sibling_prs(ctx, out, result, branch_name)?;
    }

    if let BranchLandKind::Updated {
        prev_target_oid, ..
    } = &result.landed
    {
        print_undo_caveat(
            out,
            result.local_delivery,
            *prev_target_oid,
            push_remote_name,
            target_branch_name,
        )?;
    }

    Ok(())
}

/// Warn for each sibling branch that the reconcile actually rewrote and that still has an open pull
/// request: its pushed branch and PR are now stale. We never auto-force-push a sibling's PR branch —
/// we only tell the user it needs re-pushing.
///
/// "Actually rewrote" is the key: only branches whose head is a replacement commit are warned, so a
/// branch that wasn't rebased (or isn't applied in the workspace) is not falsely flagged.
fn warn_stale_sibling_prs(
    ctx: &Context,
    out: &mut OutputChannel,
    result: &BranchLandResult,
    landed_branch: &str,
) -> anyhow::Result<()> {
    let rewritten: std::collections::HashSet<gix::ObjectId> = result
        .workspace
        .replaced_commits
        .values()
        .copied()
        .collect();
    if rewritten.is_empty() {
        return Ok(());
    }
    let Some(out) = out.for_human() else {
        return Ok(());
    };
    let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
    let t = theme::get();
    for branch in &branches {
        let name = branch.name.to_string();
        if name == landed_branch || !rewritten.contains(&branch.head) {
            continue;
        }
        let pr = branch
            .stack
            .as_ref()
            .and_then(|stack| stack.pull_requests.get(&name).copied());
        if let Some(pr) = pr {
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

/// Print an honest note about reversibility. `but undo` cannot un-push a real remote, so on that
/// path we point at the manual revert recipe instead of promising a clean undo.
fn print_undo_caveat(
    out: &mut OutputChannel,
    local_delivery: bool,
    prev_target_oid: gix::ObjectId,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let Some(out) = out.for_human() else {
        return Ok(());
    };
    let t = theme::get();
    if local_delivery {
        // The local-ref move is not captured by the oplog snapshot yet, so `but undo` rolls back
        // the branch reconcile but not the target move. Say so rather than over-promising.
        writeln!(
            out,
            "{}",
            t.hint.paint(format!(
                "`but undo` reverts the branch reconcile; to also move the local target back: \
                 git update-ref refs/heads/{target_branch_name} {prev_target_oid} && \
                 git update-ref refs/remotes/{push_remote_name}/{target_branch_name} {prev_target_oid}"
            ))
        )?;
    } else {
        writeln!(
            out,
            "{}",
            t.hint.paint(format!(
                "Pushed to {push_remote_name}/{target_branch_name}; `but undo` cannot un-push it. \
                 To revert the remote: git push --force-with-lease {push_remote_name} \
                 {prev_target_oid}:refs/heads/{target_branch_name} — or, if the branch is protected \
                 against force-pushes, revert with a new commit or via your forge instead."
            ))
        )?;
    }
    Ok(())
}

/// Confirm a direct target update. The PR-attached warning is always printed; `--yes` only skips
/// the interactive prompt, never the warning.
pub(super) fn confirm_direct_target_update(
    out: &mut OutputChannel,
    branch_name: &str,
    pr_number: Option<usize>,
    target_display: &str,
    yes: bool,
) -> anyhow::Result<()> {
    let action = format!(
        "This lands {branch_name} directly onto {target_display} without a pull request — \
         skipping any code review, CI checks, or branch protections your team may rely on."
    );
    let warning = if let Some(pr_number) = pr_number {
        format!(
            "Branch {branch_name} has PR #{pr_number} attached. {action} The pull request will be \
             left open and its remote branch orphaned."
        )
    } else {
        action
    };

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", theme::get().attention.paint(&warning))?;
    }

    if yes {
        return Ok(());
    }

    let Some(mut inout) = out.prepare_for_terminal_input() else {
        bail!(
            "Refusing to directly update {target_display} without confirmation. Re-run with --yes to confirm."
        );
    };

    if inout.confirm(
        format!("Land {branch_name} directly onto {target_display}?"),
        ConfirmDefault::No,
    )? == Confirm::No
    {
        bail!("Land cancelled");
    }

    Ok(())
}

/// The open pull-request number attached to `branch_name`, if any, read from its stack.
pub(super) fn branch_pr_number(ctx: &Context, branch_name: &str) -> anyhow::Result<Option<usize>> {
    let pr_number = but_api::legacy::virtual_branches::list_branches(ctx, None)?
        .into_iter()
        .find(|branch| branch.name.to_string() == branch_name)
        .and_then(|branch| branch.stack)
        .and_then(|stack| stack.pull_requests.get(branch_name).copied());
    Ok(pr_number)
}
