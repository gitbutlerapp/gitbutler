use but_ctx::Context;
use colored::Colorize;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::{self, Conflicted, Integrated, SaflyUpdatable},
    Resolution, ResolutionApproach,
    StackStatuses::{UpToDate, UpdatesRequired},
    TreeStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::utils::OutputChannel;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullResult {
    status: String,
    upstream_url: Option<String>,
    upstream_commits_found: usize,
    recent_commits: Vec<CommitInfo>,
    branches_to_update: Vec<BranchUpdateInfo>,
    integrated_branches: Vec<String>,
    conflicts: Vec<ConflictInfo>,
    summary: PullSummary,
    undo_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitInfo {
    id: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchUpdateInfo {
    name: String,
    status: String,
    commit_count: usize,
    conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConflictInfo {
    branch: String,
    files: Vec<String>,
    upstream_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullSummary {
    branches_updated: usize,
    branches_conflicted: usize,
    branches_integrated: usize,
    branches_unchanged: usize,
}

pub async fn handle(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    let mut pull_result = PullResult {
        status: String::new(),
        upstream_url: None,
        upstream_commits_found: 0,
        recent_commits: vec![],
        branches_to_update: vec![],
        integrated_branches: vec![],
        conflicts: vec![],
        summary: PullSummary {
            branches_updated: 0,
            branches_conflicted: 0,
            branches_integrated: 0,
            branches_unchanged: 0,
        },
        undo_command: None,
    };

    // Step 1: Check upstream data
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Checking upstream data...".bright_cyan())?;
    }

    // Fetch from remotes to get latest upstream info
    let base_branch = but_api::legacy::virtual_branches::fetch_from_remotes(
        ctx.legacy_project.id,
        Some("pull".to_string()),
    )?;

    let upstream_url = format!(
        "{}/{}",
        base_branch.remote_url.trim_end_matches(".git"),
        base_branch.branch_name
    );
    pull_result.upstream_url = Some(upstream_url.clone());
    pull_result.upstream_commits_found = base_branch.behind;

    if let Some(out) = out.for_human() {
        writeln!(out, "   Fetching from: {}", upstream_url.bright_cyan())?;

        if base_branch.behind > 0 {
            writeln!(
                out,
                "\n{} {} upstream commits on {}/{}",
                "Found".bright_white(),
                base_branch.behind.to_string().bright_yellow(),
                base_branch.remote_name.bright_cyan(),
                base_branch.branch_name.bright_cyan()
            )?;

            // Show recent commits
            let commits_to_show = 5.min(base_branch.recent_commits.len());
            for commit in base_branch.recent_commits.iter().take(commits_to_show) {
                let msg = commit
                    .description
                    .to_string()
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(65)
                    .collect::<String>();

                writeln!(out, "   {} {}", &commit.id[..7].bright_black(), msg)?;

                pull_result.recent_commits.push(CommitInfo {
                    id: commit.id.clone(),
                    message: commit.description.to_string(),
                });
            }

            let hidden = base_branch.behind.saturating_sub(commits_to_show);
            if hidden > 0 {
                writeln!(out, "   ... and {} more", hidden.to_string().bright_black())?;
            }
        } else {
            writeln!(out, "\n{}", "No new upstream commits found".green())?;
        }
    }

    // Step 2: Check integration status
    let status = but_api::legacy::virtual_branches::upstream_integration_statuses(
        ctx.legacy_project.id,
        None,
    )
    .await?;

    let resolutions = match status {
        UpToDate => {
            pull_result.status = "up_to_date".to_string();
            if let Some(out) = out.for_human() {
                writeln!(out, "\n{}", "Everything is up to date".green())?;
            }
            if let Some(out) = out.for_json() {
                out.write_value(&pull_result)?;
            }
            None
        }
        UpdatesRequired {
            worktree_conflicts,
            statuses,
        } => {
            if !worktree_conflicts.is_empty() {
                pull_result.status = "worktree_conflicts".to_string();
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "\n{}",
                        "There are uncommitted changes in the worktree that may conflict with the updates.".red()
                    )?;
                    writeln!(
                        out,
                        "   {}",
                        "Please commit or stash them and try again.".yellow()
                    )?;
                }
                if let Some(out) = out.for_json() {
                    out.write_value(&pull_result)?;
                }
                None
            } else {
                pull_result.status = "updating".to_string();

                // Analyze branches to update
                let mut branches_to_update = 0;
                let mut integrated_branches = vec![];
                let mut conflicted_branches = vec![];
                let mut resolutions = vec![];

                for (maybe_stack_id, status) in &statuses {
                    let Some(stack_id) = maybe_stack_id else {
                        if let Some(out) = out.for_human() {
                            writeln!(
                                out,
                                "\n{}",
                                "No stack ID, assuming we're on single-branch mode...".yellow()
                            )?;
                        }
                        continue;
                    };

                    for branch_status in &status.branch_statuses {
                        branches_to_update += 1;

                        let branch_info = BranchUpdateInfo {
                            name: branch_status.name.clone(),
                            status: format_branch_status(&branch_status.status),
                            commit_count: 0, // TODO: Get actual commit count
                            conflicts: vec![],
                        };

                        match &branch_status.status {
                            Integrated => {
                                integrated_branches.push(branch_status.name.clone());
                                pull_result.summary.branches_integrated += 1;
                            }
                            Conflicted { rebasable } => {
                                conflicted_branches.push((branch_status.name.clone(), *rebasable));
                                pull_result.summary.branches_conflicted += 1;
                                // TODO: Get actual conflict files
                            }
                            SaflyUpdatable => {
                                pull_result.summary.branches_updated += 1;
                            }
                            _ => {}
                        }

                        pull_result.branches_to_update.push(branch_info);
                    }

                    let approach = if status
                        .branch_statuses
                        .iter()
                        .all(|s| s.status == Integrated)
                        && status.tree_status != TreeStatus::Conflicted
                    {
                        ResolutionApproach::Delete
                    } else {
                        ResolutionApproach::Rebase
                    };

                    let resolution = Resolution {
                        stack_id: *stack_id,
                        approach: approach.clone(),
                        delete_integrated_branches: true,
                    };
                    resolutions.push(resolution);
                }

                if let Some(out) = out.for_human() {
                    if branches_to_update > 0 {
                        writeln!(
                            out,
                            "\n{} {} active branches...",
                            "Updating".bright_cyan(),
                            branches_to_update.to_string().bright_yellow()
                        )?;
                    }
                }

                pull_result.integrated_branches = integrated_branches.clone();

                Some((resolutions, statuses))
            }
        }
    };

    // Step 3: Actually perform the integration
    if let Some((resolutions, statuses)) = resolutions {
        // Store branch information before integration, along with resolution approaches
        let mut branch_info_map: HashMap<gitbutler_stack::StackId, (String, String)> =
            HashMap::new();
        let mut resolution_map: HashMap<gitbutler_stack::StackId, ResolutionApproach> =
            HashMap::new();

        for (maybe_stack_id, status) in &statuses {
            if let Some(stack_id) = maybe_stack_id {
                for branch_status in &status.branch_statuses {
                    let status_str = format_branch_status(&branch_status.status);
                    branch_info_map
                        .insert(stack_id.clone(), (branch_status.name.clone(), status_str));
                }
            }
        }

        // Store resolution approaches before moving resolutions
        for resolution in &resolutions {
            resolution_map.insert(resolution.stack_id.clone(), resolution.approach.clone());
        }

        let integration_result = but_api::legacy::virtual_branches::integrate_upstream(
            ctx.legacy_project.id,
            resolutions,
            None,
        )
        .await;

        match integration_result {
            Ok(_outcome) => {
                // IntegrationOutcome only tells us about deleted branches, but we already tracked that

                // Show results for each branch
                if let Some(out) = out.for_human() {
                    // Re-fetch status to check for any remaining conflicts
                    let post_status =
                        but_api::legacy::virtual_branches::upstream_integration_statuses(
                            ctx.legacy_project.id,
                            None,
                        )
                        .await?;

                    writeln!(out)?;

                    // Report detailed results for each resolution
                    let mut successful_rebases: Vec<String> = Vec::new();
                    let mut conflicted_rebases: Vec<String> = Vec::new();

                    for (stack_id, approach) in &resolution_map {
                        if let Some((branch_name, _original_status)) = branch_info_map.get(stack_id)
                        {
                            match approach {
                                ResolutionApproach::Rebase => {
                                    // Check if this branch still has conflicts in post_status
                                    let still_conflicted = if let UpdatesRequired {
                                        statuses: post_statuses,
                                        ..
                                    } = &post_status
                                    {
                                        post_statuses.iter().any(|(id, status)| {
                                            id.as_ref() == Some(stack_id)
                                                && status.branch_statuses.iter().any(|bs| {
                                                    matches!(bs.status, Conflicted { .. })
                                                })
                                        })
                                    } else {
                                        false
                                    };

                                    if still_conflicted {
                                        conflicted_rebases.push(branch_name.to_string());
                                    } else {
                                        successful_rebases.push(branch_name.to_string());
                                    }
                                }
                                ResolutionApproach::Delete => {
                                    // Already handled in integrated_branches
                                }
                                _ => {}
                            }
                        }
                    }

                    // Show successful rebases
                    for branch_name in &successful_rebases {
                        writeln!(
                            out,
                            "{} of {} {}",
                            "Rebase".bright_white(),
                            branch_name.as_str().bright_cyan(),
                            "successful".green()
                        )?;
                    }

                    // Show conflicted rebases
                    for branch_name in &conflicted_rebases {
                        writeln!(
                            out,
                            "{} of {} {}",
                            "Rebase".bright_white(),
                            branch_name.as_str().bright_cyan(),
                            "resulted in conflicts".yellow()
                        )?;
                    }

                    // Report on integrated branches
                    if !pull_result.integrated_branches.is_empty() {
                        writeln!(out)?;
                        for branch in &pull_result.integrated_branches {
                            writeln!(
                                out,
                                "{} {} has been integrated upstream and removed locally",
                                "Branch".bright_white(),
                                branch.bright_green()
                            )?;
                        }
                    }

                    // Check if there are any conflicted files
                    let has_conflicts = !conflicted_rebases.is_empty()
                        || (if let UpdatesRequired {
                            statuses: post_statuses,
                            ..
                        } = &post_status
                        {
                            post_statuses.iter().any(|(_, status)| {
                                status.tree_status == TreeStatus::Conflicted
                                    || status
                                        .branch_statuses
                                        .iter()
                                        .any(|bs| matches!(bs.status, Conflicted { .. }))
                            })
                        } else {
                            false
                        });

                    // Final summary
                    writeln!(out, "\n{}", "Summary".bold())?;
                    writeln!(out, "────────")?;

                    // List each branch with color-coded status
                    for branch in &successful_rebases {
                        writeln!(out, "  {} - {}", branch.bright_cyan(), "rebased".green())?;
                    }

                    for branch in &pull_result.integrated_branches {
                        writeln!(
                            out,
                            "  {} - {}",
                            branch.bright_cyan(),
                            "integrated".bright_purple()
                        )?;
                    }

                    for branch in &conflicted_rebases {
                        writeln!(out, "  {} - {}", branch.bright_cyan(), "conflicted".red())?;
                    }

                    // Conflict resolution instructions
                    if has_conflicts {
                        writeln!(out)?;
                        writeln!(out, "{}", "To resolve conflicts:".bold())?;
                        writeln!(
                            out,
                            "  1. Run {} to see conflicted commits",
                            "`but status`".bright_cyan()
                        )?;
                        writeln!(
                            out,
                            "  2. Switch to resolve mode to check out conflicts with `but resolve <commit-id>`"
                        )?;
                        writeln!(out, "  3. Fix all conflicts")?;
                        writeln!(
                            out,
                            "  4. Run {} to finalize the resolution",
                            "`but resolve`".bright_cyan()
                        )?;
                    }

                    // Undo instructions
                    writeln!(out)?;
                    writeln!(out, "{}", "To undo this operation:".bright_white())?;
                    writeln!(out, "  Run `but undo`")?;
                }
            }
            Err(e) => {
                pull_result.status = "error".to_string();
                if let Some(out) = out.for_human() {
                    writeln!(out, "\n{} {}", "Error during integration:".red(), e)?;
                }
                if let Some(out) = out.for_json() {
                    out.write_value(&pull_result)?;
                }
                return Err(e);
            }
        }
    }

    Ok(())
}

fn format_branch_status(status: &BranchStatus) -> String {
    match status {
        SaflyUpdatable => "updatable".to_string(),
        Integrated => "integrated".to_string(),
        Conflicted { rebasable } => {
            if *rebasable {
                "conflicted_rebasable".to_string()
            } else {
                "conflicted_not_rebasable".to_string()
            }
        }
        BranchStatus::Empty => "empty".to_string(),
    }
}
