mod json;

use but_ctx::Context;
use colored::Colorize;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::{self, Conflicted, Empty, Integrated, SaflyUpdatable},
    Resolution, ResolutionApproach,
    StackStatuses::{UpToDate, UpdatesRequired},
    TreeStatus,
};
use json::{BaseBranchInfo, BranchStatusInfo, PullCheckOutput, UpstreamCommit, UpstreamInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write;

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

pub async fn handle(
    ctx: &Context,
    out: &mut OutputChannel,
    check_only: bool,
) -> anyhow::Result<()> {
    if check_only {
        handle_check(ctx, out).await
    } else {
        handle_pull(ctx, out).await
    }
}

async fn handle_check(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    let mut progress = out.progress_channel();

    if out.for_human().is_some() {
        writeln!(progress, "Fetching from upstream remotes...")?;
    }

    let base_branch = but_api::legacy::virtual_branches::fetch_from_remotes(
        ctx.legacy_project.id,
        Some("auto".to_string()),
    )?;

    if out.for_human().is_some() {
        writeln!(progress, "Checking integration statuses...")?;
    }

    let status = but_api::legacy::virtual_branches::upstream_integration_statuses(
        ctx.legacy_project.id,
        None,
    )
    .await?;

    if let Some(out) = out.for_json() {
        let (up_to_date, has_worktree_conflicts, branch_statuses) = match &status {
            UpToDate => (true, false, vec![]),
            UpdatesRequired {
                worktree_conflicts,
                statuses,
            } => {
                let branch_statuses: Vec<BranchStatusInfo> = statuses
                    .iter()
                    .flat_map(|(_id, stack_status)| {
                        stack_status.branch_statuses.iter().map(|bs| {
                            let (status_str, rebasable) = match bs.status {
                                SaflyUpdatable => ("updatable", None),
                                Integrated => ("integrated", None),
                                Conflicted { rebasable } => ("conflicted", Some(rebasable)),
                                Empty => ("empty", None),
                            };
                            BranchStatusInfo {
                                name: bs.name.clone(),
                                status: status_str.to_string(),
                                rebasable,
                            }
                        })
                    })
                    .collect();
                (false, !worktree_conflicts.is_empty(), branch_statuses)
            }
        };

        let output = PullCheckOutput {
            base_branch: BaseBranchInfo {
                name: base_branch.branch_name.clone(),
                remote_name: base_branch.remote_name.clone(),
                base_sha: base_branch.base_sha.to_string(),
                current_sha: base_branch.current_sha.to_string(),
            },
            upstream_commits: UpstreamInfo {
                count: base_branch.behind,
                commits: base_branch
                    .upstream_commits
                    .iter()
                    .map(|c| UpstreamCommit {
                        id: c.id.clone(),
                        description: c.description.to_string(),
                        author_name: c.author.name.clone(),
                    })
                    .collect(),
            },
            branch_statuses,
            up_to_date,
            has_worktree_conflicts,
        };
        out.write_value(output)?;
    } else if let Some(out) = out.for_human() {
        writeln!(progress, "{}", "Checking base branch status...".bold())?;
        writeln!(
            out,
            "\n{}\t{}",
            "Base branch:".dimmed(),
            base_branch.branch_name.cyan()
        )?;
        let upstream_label = format!(
            "{} new commits on {}",
            base_branch.behind, base_branch.branch_name
        );
        writeln!(
            out,
            "{}\t{}",
            "Upstream:".dimmed(),
            if base_branch.behind > 0 {
                upstream_label.yellow()
            } else {
                upstream_label.green()
            }
        )?;

        if !base_branch.upstream_commits.is_empty() {
            writeln!(out)?;
            let commits = base_branch.upstream_commits.iter().take(3);
            for commit in commits {
                writeln!(
                    out,
                    "  {} {}",
                    commit.id[..7].yellow(),
                    commit
                        .description
                        .to_string()
                        .replace('\n', " ")
                        .chars()
                        .take(72)
                        .collect::<String>()
                        .dimmed()
                )?;
            }
            let hidden_commits = base_branch.behind.saturating_sub(3);
            if hidden_commits > 0 {
                writeln!(out, "  {}", format!("... ({hidden_commits} more)").dimmed())?;
            }
        }

        match status {
            UpToDate => {
                writeln!(out, "\n{}", "Up to date".green().bold())?;
            }
            UpdatesRequired {
                worktree_conflicts,
                statuses,
            } => {
                if !worktree_conflicts.is_empty() {
                    writeln!(
                        out,
                        "\n{}",
                        "Warning: uncommitted changes may conflict with updates."
                            .yellow()
                            .bold()
                    )?;
                }
                if !statuses.is_empty() {
                    writeln!(out, "\n{}", "Branch Status".bold())?;
                    for (_id, status) in statuses {
                        for bs in status.branch_statuses {
                            let status_text = match bs.status {
                                SaflyUpdatable => "[ok]".green(),
                                Integrated => "[integrated]".blue(),
                                Conflicted { rebasable } => {
                                    if rebasable {
                                        "[conflict - rebasable]".yellow()
                                    } else {
                                        "[conflict]".red()
                                    }
                                }
                                Empty => "[empty]".dimmed(),
                            };
                            writeln!(out, "  {} {}", status_text, bs.name)?;
                        }
                    }
                }
                writeln!(
                    out,
                    "\n{}",
                    "Run `but pull` to update your branches".dimmed()
                )?;
            }
        }
    }
    Ok(())
}

async fn handle_pull(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
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

    let mut progress = out.progress_channel();

    // Step 1: Check upstream data
    if let Some(_out) = out.for_human() {
        writeln!(
            progress,
            "{}",
            "Fetching newest data from remotes...".bright_cyan()
        )?;
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

    // Populate recent commits from upstream_commits (actual new commits to integrate)
    let commits_to_show = 5.min(base_branch.upstream_commits.len());
    for commit in base_branch.upstream_commits.iter().take(commits_to_show) {
        pull_result.recent_commits.push(CommitInfo {
            id: commit.id.clone(),
            message: commit.description.to_string(),
        });
    }

    if let Some(out) = out.for_human() {
        writeln!(progress, "   Checking: {}", upstream_url.bright_cyan())?;

        if base_branch.behind > 0 {
            writeln!(
                out,
                "\n{} {} upstream commits on {}",
                "Found".bright_white(),
                base_branch.behind.to_string().bright_yellow(),
                base_branch.branch_name.bright_cyan()
            )?;

            // Show upstream commits (actual new commits to integrate)
            for commit_info in &pull_result.recent_commits {
                let msg = commit_info
                    .message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(65)
                    .collect::<String>();

                writeln!(out, "   {} {}", &commit_info.id[..7].bright_black(), msg)?;
            }

            let hidden = base_branch.behind.saturating_sub(commits_to_show);
            if hidden > 0 {
                writeln!(out, "   ... and {} more", hidden.to_string().bright_black())?;
            }
        } else {
            writeln!(out, "\n{}", "No new upstream commits found".green())?;
        }

        writeln!(progress, "   Checking integration statuses...")?;
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
                            Conflicted { .. } => {
                                pull_result.summary.branches_conflicted += 1;
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
                        approach,
                        delete_integrated_branches: true,
                    };
                    resolutions.push(resolution);
                }

                if let Some(out) = out.for_human()
                    && branches_to_update > 0
                {
                    writeln!(
                        out,
                        "\n{} {} active branches...",
                        "Updating".bright_cyan(),
                        branches_to_update.to_string().bright_yellow()
                    )?;
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
                    branch_info_map.insert(*stack_id, (branch_status.name.clone(), status_str));
                }
            }
        }

        // Store resolution approaches before moving resolutions
        for resolution in &resolutions {
            resolution_map.insert(resolution.stack_id, resolution.approach);
        }

        let integration_result = but_api::legacy::virtual_branches::integrate_upstream(
            ctx.legacy_project.id,
            resolutions,
            None,
        )
        .await;

        match integration_result {
            Ok(_outcome) => {
                // Re-fetch status to check for any remaining conflicts
                let post_status = but_api::legacy::virtual_branches::upstream_integration_statuses(
                    ctx.legacy_project.id,
                    None,
                )
                .await?;

                // Report detailed results for each resolution
                let mut successful_rebases: Vec<String> = Vec::new();
                let mut conflicted_rebases: Vec<String> = Vec::new();

                for (stack_id, approach) in &resolution_map {
                    if let Some((branch_name, _original_status)) = branch_info_map.get(stack_id) {
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
                                            && status
                                                .branch_statuses
                                                .iter()
                                                .any(|bs| matches!(bs.status, Conflicted { .. }))
                                    })
                                } else {
                                    false
                                };

                                // Also check if any commits in the branch have conflicts
                                let has_conflicted_commits =
                                    but_api::legacy::workspace::stack_details(
                                        ctx.legacy_project.id,
                                        Some(*stack_id),
                                    )
                                    .ok()
                                    .map(|details| {
                                        details
                                            .branch_details
                                            .iter()
                                            .any(|bd| bd.commits.iter().any(|c| c.has_conflicts))
                                    })
                                    .unwrap_or(false);

                                if still_conflicted || has_conflicted_commits {
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

                // Update final status
                pull_result.status = if has_conflicts {
                    "completed_with_conflicts".to_string()
                } else {
                    "completed".to_string()
                };

                // Update summary counts
                pull_result.summary.branches_updated = successful_rebases.len();
                pull_result.summary.branches_conflicted = conflicted_rebases.len();
                pull_result.summary.branches_integrated = pull_result.integrated_branches.len();

                // Set undo command
                pull_result.undo_command = Some("but undo".to_string());

                // Populate conflicts info
                for branch_name in &conflicted_rebases {
                    pull_result.conflicts.push(ConflictInfo {
                        branch: branch_name.clone(),
                        files: vec![], // TODO: Get actual conflicted files
                        upstream_commit: None,
                    });
                }

                // Show results for each branch
                if let Some(out) = out.for_human() {
                    writeln!(out)?;

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
                            "  2. Run {} to enter resolution mode on any conflicted commit",
                            "`but resolve <commit>`".bright_cyan()
                        )?;
                        writeln!(out, "  3. Edit files to resolve the conflicts")?;
                        writeln!(
                            out,
                            "  4. Run {} to finalize the resolution",
                            "`but resolve finish`".bright_cyan()
                        )?;
                    }

                    // Undo instructions
                    writeln!(out)?;
                    writeln!(out, "{}", "To undo this operation:".bright_white())?;
                    writeln!(out, "  Run `but undo`")?;
                }

                // Output JSON result
                if let Some(out) = out.for_json() {
                    out.write_value(&pull_result)?;
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
