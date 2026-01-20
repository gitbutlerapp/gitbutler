use anyhow::Context as _;
use but_ctx::Context;
use colored::Colorize;
use gitbutler_branch_actions::BranchListingFilter;
use serde::Serialize;

use crate::utils::OutputChannel;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TeardownResult {
    snapshot_id: String,
    checked_out_branch: String,
    dangling_commits: Vec<DanglingCommit>,
    all_cherry_picks_successful: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DanglingCommit {
    commit_id: String,
    cherry_picked: bool,
}

pub(crate) fn teardown(ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    // Check that we're on gitbutler/workspace
    let repo = ctx.repo.get()?;
    let head = repo.head()?;
    let head_name = head
        .referent_name()
        .map(|n| n.shorten().to_string())
        .unwrap_or_default();

    if head_name != "gitbutler/workspace" {
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                format!(
                    "Not currently on gitbutler/workspace branch (on: {}).",
                    head_name
                )
                .red()
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

    let snapshot = but_api::legacy::oplog::create_snapshot(
        ctx.legacy_project.id,
        Some("Teardown: exiting GitButler mode".to_string()),
    )?;

    let snapshot_id = snapshot.to_string();

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            format!("✓ Snapshot created: {}", &snapshot_id[..7]).green()
        )?;
        writeln!(out)?;
    }

    // Find the first active branch
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            "→ Finding active branch to check out...".dimmed()
        )?;
    }

    let filter = Some(BranchListingFilter {
        local: Some(true),
        applied: Some(true),
    });

    let branches = but_api::legacy::virtual_branches::list_branches(ctx.legacy_project.id, filter)?;

    let target_branch = branches
        .first()
        .ok_or_else(|| anyhow::anyhow!("No active branches found"))?;

    let target_branch_name = target_branch.name.to_string();

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            format!("✓ Will check out: {}", target_branch_name).green()
        )?;
        writeln!(out)?;
    }

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
        }

        // This is a dangling commit
        dangling_commits.push(commit.id().detach());

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
        // Reverse to get chronological order (oldest first)
        dangling_commits.reverse();

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                format!("⚠ Found {} dangling commit(s):", dangling_commits.len()).yellow()
            )?;
            for commit_id in &dangling_commits {
                writeln!(out, "    {}", &commit_id.to_string()[..7])?;
            }
            writeln!(out)?;
        }
    }

    // Check out the target branch using Git directly
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            format!("→ Checking out {}...", target_branch_name).dimmed()
        )?;
    }

    // Use git checkout via command
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
        anyhow::bail!(
            "Failed to checkout branch: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            format!("✓ Checked out: {}", target_branch_name).green()
        )?;
        writeln!(out)?;
    }

    // Cherry-pick dangling commits if any
    let mut dangling_commit_results = Vec::new();
    let mut all_successful = true;

    if !dangling_commits.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "{}", "→ Cherry-picking dangling commits...".dimmed())?;
        }

        for commit_id in &dangling_commits {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(workdir)
                .arg("cherry-pick")
                .arg(commit_id.to_string())
                .output()
                .context("Failed to execute git cherry-pick")?;

            let cherry_picked = output.status.success();

            if !cherry_picked {
                all_successful = false;
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "  {}",
                        format!("✗ Failed to cherry-pick: {}", &commit_id.to_string()[..7]).red()
                    )?;
                    writeln!(
                        out,
                        "    {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    )?;
                }
            } else if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "  {}",
                    format!("✓ Cherry-picked: {}", &commit_id.to_string()[..7]).green()
                )?;
            }

            dangling_commit_results.push(DanglingCommit {
                commit_id: commit_id.to_string(),
                cherry_picked,
            });
        }

        if let Some(out) = out.for_human() {
            writeln!(out)?;
        }

        if !all_successful && let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                "⚠ Some commits could not be cherry-picked automatically.".yellow()
            )?;
            writeln!(
                out,
                "{}",
                "  Resolve conflicts and run 'git cherry-pick --continue'".dimmed()
            )?;
            writeln!(out)?;
        }
    }

    // Final success message
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            "✓ Successfully exited GitButler mode!".green().bold()
        )?;
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
        let result = TeardownResult {
            snapshot_id,
            checked_out_branch: target_branch_name,
            dangling_commits: dangling_commit_results,
            all_cherry_picks_successful: all_successful,
        };
        let json = serde_json::to_string_pretty(&result)?;
        writeln!(out, "{}", json)?;
    }

    Ok(())
}
