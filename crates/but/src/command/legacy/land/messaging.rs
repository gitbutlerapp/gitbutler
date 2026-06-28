//! Confirmation prompt and end-of-command reporting, kept honest about what actually happened.

use anyhow::bail;
use but_ctx::Context;

use crate::{
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

use super::Landed;

/// The headline reported at the end of a land, honest about whether the branch was actually moved
/// onto the target this run or was already there.
pub(super) fn land_headline(landed: &Landed, branch_name: &str, target_display: &str) -> String {
    match landed {
        Landed::AlreadyIntegrated => format!("{branch_name} was already on {target_display}."),
        Landed::Updated { .. } => format!("Landed {branch_name} onto {target_display}."),
    }
}

/// Print an honest note about reversibility. `but undo` cannot un-push a real remote, so on that
/// path we point at the manual revert recipe instead of promising a clean undo.
pub(super) fn print_undo_caveat(
    out: &mut OutputChannel,
    update_target_locally: bool,
    landed: &Landed,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    if update_target_locally {
        // The local-ref move is not captured by the oplog snapshot yet, so `but undo` rolls back
        // the branch reconcile but not the target move. Say so rather than over-promising.
        if let (
            Some(out),
            Landed::Updated {
                prev_target_oid, ..
            },
        ) = (out.for_human(), landed)
        {
            let t = theme::get();
            writeln!(
                out,
                "{}",
                t.hint.paint(format!(
                    "`but undo` reverts the branch reconcile; to also move the local target back: \
                     git update-ref refs/heads/{target_branch_name} {prev_target_oid} && \
                     git update-ref refs/remotes/{push_remote_name}/{target_branch_name} {prev_target_oid}"
                ))
            )?;
        }
        Ok(())
    } else {
        print_push_undo_caveat(out, landed, push_remote_name, target_branch_name)
    }
}

/// The real-remote variant of the undo note: undo cannot un-push, so show the force-with-lease recipe.
pub(super) fn print_push_undo_caveat(
    out: &mut OutputChannel,
    landed: &Landed,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    if let (
        Some(out),
        Landed::Updated {
            prev_target_oid, ..
        },
    ) = (out.for_human(), landed)
    {
        let t = theme::get();
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
    update_target_locally: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let action = if update_target_locally {
        format!("This will move your local target {target_display} directly.")
    } else {
        format!(
            "This lands {branch_name} directly onto {target_display} without a pull request — \
             skipping any code review, CI checks, or branch protections your team may rely on."
        )
    };
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

/// Pre-flight facts about the branch being landed, read from its stack.
pub(super) struct BranchLandingInfo {
    /// An open pull-request number attached to the branch, if any.
    pub pr_number: Option<usize>,
    /// Stack segments below the branch (oldest last). Landing the branch's tip would also publish
    /// these, so a non-empty list means the user named a non-bottom segment.
    pub lower_segments: Vec<String>,
}

/// Read the branch's stack to learn its attached PR and any segments stacked below it.
pub(super) fn branch_landing_info(
    ctx: &Context,
    branch_name: &str,
) -> anyhow::Result<BranchLandingInfo> {
    let stack = but_api::legacy::virtual_branches::list_branches(ctx, None)?
        .into_iter()
        .find(|branch| branch.name.to_string() == branch_name)
        .and_then(|branch| branch.stack);
    let Some(stack) = stack else {
        return Ok(BranchLandingInfo {
            pr_number: None,
            lower_segments: Vec::new(),
        });
    };
    // `stack.branches` is ordered newest -> oldest, so the segments AFTER `branch_name` are the
    // ones stacked below it whose commits the branch's tip carries along.
    let lower_segments = stack
        .branches
        .iter()
        .skip_while(|name| name.as_str() != branch_name)
        .skip(1)
        .cloned()
        .collect();
    Ok(BranchLandingInfo {
        pr_number: stack.pull_requests.get(branch_name).copied(),
        lower_segments,
    })
}
