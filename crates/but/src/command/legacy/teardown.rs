use anyhow::Context as _;
use but_ctx::Context;
use but_workspace::legacy::StacksFilter;
use colored::Colorize;
use gix::refs::transaction::PreviousValue;
use serde::Serialize;

use crate::utils::OutputChannel;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TeardownResult {
    snapshot_id: String,
    checked_out_branch: String,
}

pub(crate) fn teardown(ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    // Check that we're on gitbutler/workspace
    let head_name = {
        let repo = ctx.repo.get()?;
        let head = repo.head()?;
        head.referent_name()
            .map(|n| n.shorten().to_string())
            .unwrap_or_default()
    };

    if !head_name.starts_with("gitbutler/") {
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                format!("Not currently on gitbutler/workspace branch (on: {}).", head_name).red()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "Teardown can only be run while on the gitbutler/workspace branch.".dimmed()
            )?;
        }
        anyhow::bail!("Not on gitbutler/workspace branch");
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Exiting GitButler mode...".cyan().bold())?;
        writeln!(out)?;
    }

    // Create an oplog snapshot
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "→ Creating snapshot...".dimmed())?;
    }

    let snapshot = but_api::legacy::oplog::create_snapshot(ctx, Some("Teardown: exiting GitButler mode".to_string()))?;

    let snapshot_id = snapshot.to_string();

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            format!("✓ Snapshot created: {}", &snapshot_id[..7]).green()
        )?;
        writeln!(out)?;
    }

    // Find the first active branch (leftmost/lowest order)
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "→ Finding active branch to check out...".dimmed())?;
    }

    // Get stacks filtered to only those in workspace, sorted by order to find the leftmost
    let mut stacks = match but_api::legacy::workspace::stacks(ctx, Some(StacksFilter::InWorkspace)) {
        Ok(stacks) => stacks,
        Err(_e) => {
            try_stack_fixes(ctx, out)?;
            but_api::legacy::workspace::stacks(ctx, Some(StacksFilter::InWorkspace))
                .context("Failed to retrieve stacks after attempting fixes")?
        }
    };

    // Sort by order to ensure we get the leftmost (lowest order) stack first
    stacks.sort_by_key(|s| s.order.unwrap_or(usize::MAX));

    let target_stack = stacks
        .first()
        .ok_or_else(|| anyhow::anyhow!("No active branches found"))?;

    // Get the name of the top branch in the stack
    let target_branch_name = target_stack
        .heads
        .first()
        .map(|h| h.name.to_string())
        .ok_or_else(|| anyhow::anyhow!("Stack has no branches"))?;

    if let Some(out) = out.for_human() {
        writeln!(out, "  {}", format!("✓ Will check out: {}", target_branch_name).green())?;
        writeln!(out)?;
    }

    // Uninstall managed hooks before checking out
    if let Ok(git2_repo) = ctx.git2_repo.get()
        && let Err(e) = gitbutler_repo::managed_hooks::uninstall_managed_hooks(&git2_repo)
        && let Some(out) = out.for_human()
    {
        writeln!(
            out,
            "  {}",
            format!("Warning: Failed to uninstall Git hooks: {}", e).yellow()
        )?;
    }

    // Check out the target branch using Git directly
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", format!("→ Checking out {}...", target_branch_name).dimmed())?;
    }

    // Use git checkout via command
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("Repository has no workdir"))?;

    let output = std::process::Command::new("git")
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
                "⚠ Checkout failed, trying soft reset...\n  ⚠ This will leave changes from multiple branches in your working directory.\n  ⚠ You will have to manually remove, stash or re-commit the changes.".yellow()
            )?;
        }

        // Also update HEAD to be a symbolic ref to the branch
        std::process::Command::new("git")
            .arg("-C")
            .arg(workdir)
            .args(["symbolic-ref", "HEAD", &format!("refs/heads/{}", target_branch_name)])
            .output()
            .context("Failed to set symbolic ref")?;
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "  {}", format!("✓ Checked out: {}", target_branch_name).green())?;
        writeln!(out)?;
    }

    // Final success message
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "✓ Successfully exited GitButler mode!".green().bold())?;
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            format!("You are now on branch: {}", target_branch_name).dimmed()
        )?;
        writeln!(out)?;
        writeln!(out, "{}", "To return to GitButler mode, run:".blue())?;
        writeln!(out, "  {}", "but setup".cyan())?;
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
fn try_stack_fixes(ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    if let Some(out) = out.for_human() {
        writeln!(out, "\n{}", "Attempting to fix workspace stacks...".yellow())?;
    }

    // check if gitbutler/workspace is pointing at a commit that does not start with "GitButler Workspace Commit"
    let repo = ctx.repo.get()?;

    // Look for dangling commits on gitbutler/workspace
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "→ Checking for dangling commits...".dimmed())?;
    }

    let mut workspace_ref = repo.find_reference("refs/heads/gitbutler/workspace")?;
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
            writeln!(out, "  {}", "✓ No dangling commits found".green())?;
            writeln!(out)?;
        }
    } else {
        // soft reset to the first workspace commit
        let target_commit = commit.id();
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                format!("→ Resetting gitbutler/workspace to {}", &target_commit.to_string()[..7]).dimmed()
            )?;
        }
        repo.reference(
            "refs/heads/gitbutler/workspace",
            target_commit,
            PreviousValue::Any,
            "soft resetting to GitButler workspace",
        )?;
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                format!("✓ gitbutler/workspace reset to {}", &target_commit.to_string()[..7]).green()
            )?;
        }

        // Reverse to get chronological order (oldest first)
        dangling_commits.reverse();

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "\n  {}",
                format!(
                    "⚠ Non-GitButler created commits found.\n  ⚠ Undoing these commits but keeping the changes in your working directory.\n  ⚠ Uncommitted {} dangling commit(s):",
                    dangling_commits.len()
                )
                .yellow()
            )?;
            for commit in &dangling_commits {
                let message = String::from_utf8_lossy(commit.message_raw().unwrap_or((&[]).into()));
                let first_line = message.lines().next().unwrap_or("");
                writeln!(out, "    {}: {}", &commit.id().to_string()[..7], first_line)?;
            }
            writeln!(out)?;
        }
    }

    Ok(())
}
