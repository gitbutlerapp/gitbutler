use anyhow::Context as _;
use but_core::WORKSPACE_REF_NAME;
use but_ctx::Context;
use but_workspace::legacy::StacksFilter;
use gix::refs::{Category, transaction::PreviousValue};
use serde::Serialize;

use crate::{
    BadInput, CliError, CliResult,
    theme::{self, Paint},
    utils::{OutputChannel, shorten_object_id},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TeardownResult {
    snapshot_id: String,
    checked_out_branch: String,
}

pub(crate) fn teardown(
    ctx: &mut Context,
    checkout_to: Option<String>,
    out: &mut OutputChannel,
) -> CliResult<()> {
    let t = theme::get();

    // Check that we're on gitbutler/workspace
    let head_name = {
        let repo = ctx.repo.get()?;
        let head = repo.head()?;
        head.referent_name()
            .map(|n| n.shorten().to_owned())
            .unwrap_or_default()
    };

    if !head_name.starts_with(b"gitbutler/") {
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                t.error.paint(format!(
                    "Not currently on gitbutler/workspace branch (on: {head_name})."
                ))
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                t.hint
                    .paint("Teardown can only be run while on the gitbutler/workspace branch.")
            )?;
        }
        return Err(BadInput::new("Not on gitbutler/workspace branch").into());
    }

    // Note: Validate checkout_to before snapshot creation to prevent unnecessary snapshot
    let checkout_to = if let Some(checkout_to) = &checkout_to {
        let repo = ctx.repo.get()?;
        let ref_name: gix::refs::PartialName = checkout_to.clone().try_into().map_err(|_| {
            CliError::from(
                BadInput::new(format!("Invalid ref name: {checkout_to}")).arg("--checkout-to"),
            )
        })?;
        let resolved_ref = match repo.try_find_reference(ref_name.as_ref())? {
            Some(resolved_ref) => resolved_ref,
            None => {
                return BadInput::new(format!("The reference '{checkout_to}' did not exist"))
                    .arg("--checkout-to")
                    .into_cli_result();
            }
        };
        if !matches!(resolved_ref.name().category(), Some(Category::LocalBranch)) {
            return BadInput::new(format!(
                "Invalid ref for checkout: '{checkout_to}' is not a local branch"
            ))
            .arg("--checkout-to")
            .into_cli_result();
        }
        Some(resolved_ref.name().shorten().to_string())
    } else {
        None
    };

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", t.progress.paint("Exiting GitButler mode..."))?;
        writeln!(out)?;
    }

    // Create an oplog snapshot
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", t.hint.paint("→ Creating snapshot..."))?;
    }

    let snapshot = but_api::legacy::oplog::create_snapshot(
        ctx,
        Some("Teardown: exiting GitButler mode".to_string()),
    )?;

    let snapshot_id = snapshot.to_string();

    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let snapshot_short = shorten_object_id(&repo, snapshot);
        writeln!(
            out,
            "  {}",
            t.success
                .paint(format!("✓ Snapshot created: {snapshot_short}"))
        )?;
        writeln!(out)?;
    }

    // Find the first active branch (leftmost/lowest order)
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.hint.paint("→ Finding active branch to check out...")
        )?;
    }

    // Get stacks filtered to only those in workspace, sorted by order to find the leftmost
    let mut stacks = match but_api::legacy::workspace::stacks(ctx, Some(StacksFilter::InWorkspace))
    {
        Ok(stacks) => stacks,
        Err(_e) => {
            try_stack_fixes(ctx, out)?;
            but_api::legacy::workspace::stacks(ctx, Some(StacksFilter::InWorkspace))
                .context("Failed to retrieve stacks after attempting fixes")?
        }
    };

    let target_branch_name = if let Some(checkout_to) = checkout_to {
        checkout_to
    } else {
        // Sort by order to ensure we get the leftmost (lowest order) stack first
        stacks.sort_by_key(|s| s.order.unwrap_or(usize::MAX));

        if let Some(target_stack) = stacks.first() {
            target_stack
                .heads
                .first()
                .map(|h| h.name.to_string())
                .ok_or_else(|| anyhow::anyhow!("Stack has no branches"))?
        } else {
            return BadInput::new(
                "Failed to determine checkout target branch. Specify a target branch with `--checkout-to <branch>`.",
            ).into_cli_result();
        }
    };

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            t.success
                .paint(format!("✓ Will check out: {target_branch_name}"))
        )?;
        writeln!(out)?;
    }

    // Uninstall managed hooks before checking out
    if let Ok(repo) = ctx.repo.get()
        && let Err(e) = gitbutler_repo::managed_hooks::uninstall_managed_hooks(&repo)
        && let Some(out) = out.for_human()
    {
        writeln!(
            out,
            "  {}",
            t.attention
                .paint(format!("Warning: Failed to uninstall Git hooks: {e}"))
        )?;
    }

    // Check out the target branch using Git directly
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.hint
                .paint(format!("→ Checking out {target_branch_name}..."))
        )?;
    }

    // Use git checkout via command
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("Repository has no workdir"))?;

    let output = std::process::Command::new(gix::path::env::exe_invocation())
        .arg("-C")
        .arg(workdir)
        .arg("checkout")
        .arg(&target_branch_name)
        .output()
        .context("Failed to execute git checkout")?;

    if !output.status.success() {
        // Checkout failed (likely due to local changes), try soft reset instead
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                t.attention.paint("⚠ Checkout failed, trying soft reset...\n  ⚠ This will leave changes from multiple branches in your working directory.\n  ⚠ You will have to manually remove, stash or re-commit the changes.")
            )?;
        }

        // Also update HEAD to be a symbolic ref to the branch
        std::process::Command::new(gix::path::env::exe_invocation())
            .arg("-C")
            .arg(workdir)
            .args([
                "symbolic-ref",
                "HEAD",
                &format!("refs/heads/{target_branch_name}"),
            ])
            .output()
            .context("Failed to set symbolic ref")?;
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            t.success
                .paint(format!("✓ Checked out: {target_branch_name}"))
        )?;
        writeln!(out)?;
    }

    // Final success message
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.success.paint("✓ Successfully exited GitButler mode!")
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            t.hint
                .paint(format!("You are now on branch: {target_branch_name}"))
        )?;
        writeln!(out)?;
        writeln!(out, "{}", t.info.paint("To return to GitButler mode, run:"))?;
        writeln!(out, "  {}", t.command_suggestion.paint("but setup"))?;
        writeln!(out)?;
    }

    // Output JSON if requested
    if let Some(out) = out.for_json() {
        out.write_value(&TeardownResult {
            snapshot_id,
            checked_out_branch: target_branch_name,
        })?;
    }

    Ok(())
}

// a call to get stacks failed, which could be because someone committed on top
// of gitbutler/workspace. Try to fix that.
fn try_stack_fixes(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    let t = theme::get();
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "\n{}",
            t.attention.paint("Attempting to fix workspace stacks...")
        )?;
    }

    // check if gitbutler/workspace is pointing at a commit that does not start with "GitButler Workspace Commit"
    let repo = ctx.repo.get()?;

    // Look for dangling commits on gitbutler/workspace
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.hint.paint("→ Checking for dangling commits...")
        )?;
    }

    let mut workspace_ref = repo.find_reference(WORKSPACE_REF_NAME)?;
    let workspace_commit = workspace_ref.peel_to_commit()?;

    // Get all commits on workspace that are not workspace commits
    let mut dangling_commits = Vec::new();
    let mut commit = workspace_commit;

    loop {
        let message = commit.message_raw()?;
        let message_str = String::from_utf8_lossy(message);
        let first_line = message_str.lines().next().unwrap_or("");

        // Stop when we hit a workspace commit or reach the base
        if first_line.starts_with("GitButler Workspace Commit") {
            break;
        } // This is a dangling commit
        dangling_commits.push(commit.clone());

        // Move to parent
        let parent_ids: Vec<_> = commit.parent_ids().collect();
        if parent_ids.is_empty() {
            break;
        }

        commit = repo.find_commit(parent_ids[0])?;
    }

    if dangling_commits.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "  {}", t.success.paint("✓ No dangling commits found"))?;
            writeln!(out)?;
        }
    } else {
        // soft reset to the first workspace commit
        let target_commit = commit.id();
        let target_short = shorten_object_id(&repo, target_commit);
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                t.hint
                    .paint(format!("→ Resetting gitbutler/workspace to {target_short}"))
            )?;
        }
        repo.reference(
            WORKSPACE_REF_NAME,
            target_commit,
            PreviousValue::Any,
            "soft resetting to GitButler workspace",
        )?;
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                t.success
                    .paint(format!("✓ gitbutler/workspace reset to {target_short}"))
            )?;
        }

        // Reverse to get chronological order (oldest first)
        dangling_commits.reverse();

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "\n  {}",
                t.attention.paint(format!(
                    "⚠ Non-GitButler created commits found.
  ⚠ Undoing these commits but keeping the changes in your working directory.
  ⚠ Uncommitted {} dangling commit(s):",
                    dangling_commits.len()
                ))
            )?;
            for commit in &dangling_commits {
                let message = String::from_utf8_lossy(commit.message_raw().unwrap_or((&[]).into()));
                let first_line = message.lines().next().unwrap_or("");
                writeln!(
                    out,
                    "    {}: {}",
                    shorten_object_id(&repo, commit.id()),
                    first_line
                )?;
            }
            writeln!(out)?;
        }
    }

    Ok(())
}
