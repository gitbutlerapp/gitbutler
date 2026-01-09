use std::io::IsTerminal;

use but_core::{RepositoryExt, ref_metadata::StackId};
use but_ctx::Context;
use cli_prompts::DisplayPrompt;
use colored::Colorize;
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_project::Project;
use serde::Serialize;
use std::fmt::Write;

use crate::{
    CliId, IdMap,
    args::{push, push::Command},
    utils::OutputChannel,
};

/// Represents the result of branch selection when no branch is specified
enum BranchSelection {
    /// Push a single branch
    Single(String),
    /// Push all branches with unpushed commits
    All,
}

/// Batch push result for JSON output
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPushResult {
    /// Successfully pushed branches
    pushed: Vec<PushResult>,
    /// Failed branches with error messages
    failed: Vec<FailedBranch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FailedBranch {
    branch_name: String,
    error: String,
}

pub fn handle(
    args: push::Command,
    ctx: &mut Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let mut id_map = IdMap::new_from_context(ctx, None)?;
    id_map.add_committed_file_info_from_context(ctx)?;

    // Check gerrit mode early
    let gerrit_mode = {
        let repo = ctx.repo.get()?;
        repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false)
    };

    // If dry-run, show what would be pushed
    if args.dry_run {
        let project_id = ctx.legacy_project.id;
        let project_gb_dir = ctx.legacy_project.gb_dir().to_path_buf();
        return handle_dry_run(ctx, project_id, &project_gb_dir, &args.branch_id, out);
    }

    // If no branch_id is provided, show all branches and prompt or push all
    let branch_selection = if let Some(ref branch_id) = args.branch_id {
        // Resolve branch_id to actual branch name
        let branch_name = resolve_branch_name(ctx, &id_map, branch_id)?;
        BranchSelection::Single(branch_name)
    } else {
        handle_no_branch_specified(ctx, &ctx.legacy_project, out)?
    };

    // Handle branch selection
    match branch_selection {
        BranchSelection::All => {
            push_all_branches(ctx, &ctx.legacy_project, &args, gerrit_mode, out)
        }
        BranchSelection::Single(branch_name) => push_single_branch(
            ctx,
            &ctx.legacy_project,
            &branch_name,
            &args,
            gerrit_mode,
            out,
        ),
    }
}

/// Information about what would be pushed for a branch
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DryRunBranchInfo {
    /// The branch name
    branch_name: String,
    /// The stack this branch belongs to
    stack_name: String,
    /// Number of unpushed commits
    unpushed_commits: usize,
    /// The remote where it will be pushed
    remote: String,
    /// The remote ref name where it will be pushed
    remote_ref: String,
    /// Commit details
    commits: Vec<DryRunCommit>,
    /// Upstream commits that would be overwritten (requires force push)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    upstream_commits: Vec<DryRunUpstreamCommit>,
    /// Whether this push requires force
    requires_force: bool,
    /// Warning message if push cannot proceed safely
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    /// Name of the branch this is stacked on top of (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    stacked_on: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DryRunCommit {
    /// Short SHA
    sha_short: String,
    /// Full SHA
    sha: String,
    /// Commit message (first line)
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DryRunUpstreamCommit {
    /// Short SHA
    sha_short: String,
    /// Full SHA
    sha: String,
    /// Commit message (first line)
    message: String,
}

/// Batch dry-run result for JSON output
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DryRunResult {
    /// Branches that would be pushed
    branches: Vec<DryRunBranchInfo>,
}

fn handle_dry_run(
    ctx: &mut Context,
    project_id: gitbutler_project::ProjectId,
    project_gb_dir: &std::path::Path,
    branch_id: &Option<String>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let mut progress = out.progress_channel();

    // Fetch from remote first to get latest state
    if out.for_human().is_some() {
        writeln!(progress, "Fetching from remote...")?;
    }

    but_api::legacy::virtual_branches::fetch_from_remotes(project_id, Some("dry_run_push".into()))?;

    // Get all branches with info
    let branches_with_info = get_branches_with_unpushed_info(ctx, &ctx.legacy_project)?;

    // Filter based on branch_id if provided
    let branches_to_show: Vec<_> = if let Some(branch_id) = branch_id {
        // Resolve branch name
        let mut id_map = IdMap::new_from_context(ctx, None)?;
        id_map.add_committed_file_info_from_context(ctx)?;
        let branch_name = resolve_branch_name(ctx, &id_map, branch_id)?;

        branches_with_info
            .into_iter()
            .filter(|(name, count, _)| name == &branch_name && *count > 0)
            .collect()
    } else {
        // Show all branches with unpushed commits
        branches_with_info
            .into_iter()
            .filter(|(_, count, _)| *count > 0)
            .collect()
    };

    if branches_to_show.is_empty() {
        if let Some(out) = out.for_json() {
            out.write_value(&DryRunResult { branches: vec![] })?;
        }

        if out.for_human().is_some() {
            writeln!(
                progress,
                "{}",
                "No branches have unpushed commits.".dimmed()
            )?;
        }
        return Ok(());
    }

    // Get detailed information for each branch
    let mut dry_run_infos = Vec::new();

    let stacks = but_api::legacy::workspace::stacks(
        project_id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Get the default target for remote name
    let vb_state = gitbutler_stack::VirtualBranchesHandle::new(project_gb_dir);
    let default_target = vb_state.get_default_target()?;
    let remote = default_target.push_remote_name();

    for (branch_name, unpushed_count, stack_name) in &branches_to_show {
        // Find the stack containing this branch
        for stack_entry in &stacks {
            if let Some(stack_id) = stack_entry.id {
                let stack_details =
                    but_api::legacy::workspace::stack_details(project_id, Some(stack_id))?;

                // Find the branch details
                if let Some(branch_detail) = stack_details
                    .branch_details
                    .iter()
                    .find(|b| b.name == branch_name.as_str())
                {
                    // Get the actual Stack to call push_details
                    let stack = vb_state.get_stack(stack_id)?;

                    // Get push details to determine remote ref
                    let push_details = match stack.push_details(ctx, branch_name.clone()) {
                        Ok(details) => details,
                        Err(_) => continue, // Skip if we can't get push details
                    };

                    // Collect commit information
                    let commits: Vec<DryRunCommit> = branch_detail
                        .commits
                        .iter()
                        .filter(|c| matches!(c.state, but_workspace::ui::CommitState::LocalOnly))
                        .take(10) // Limit to first 10 commits for display
                        .map(|c| {
                            let sha = c.id.to_string();
                            let sha_short: String = sha.chars().take(7).collect();
                            let message = c
                                .message
                                .to_string()
                                .lines()
                                .next()
                                .unwrap_or("")
                                .to_string();
                            DryRunCommit {
                                sha_short,
                                sha,
                                message,
                            }
                        })
                        .collect();

                    // Collect upstream commits (commits on remote but not local)
                    let upstream_commits: Vec<DryRunUpstreamCommit> = branch_detail
                        .upstream_commits
                        .iter()
                        .take(10) // Limit to first 10 commits for display
                        .map(|c| {
                            let sha = c.id.to_string();
                            let sha_short: String = sha.chars().take(7).collect();
                            let message = c
                                .message
                                .to_string()
                                .lines()
                                .next()
                                .unwrap_or("")
                                .to_string();
                            DryRunUpstreamCommit {
                                sha_short,
                                sha,
                                message,
                            }
                        })
                        .collect();

                    // Determine if force push is required and generate warning
                    let requires_force = matches!(
                        branch_detail.push_status,
                        but_workspace::ui::PushStatus::UnpushedCommitsRequiringForce
                    );

                    let warning = if !upstream_commits.is_empty() && !requires_force {
                        Some(format!(
                            "Cannot push: {} upstream commit{} would be overwritten. Use force push to proceed.",
                            upstream_commits.len(),
                            if upstream_commits.len() == 1 { "" } else { "s" }
                        ))
                    } else if !upstream_commits.is_empty() && requires_force {
                        Some(format!(
                            "Force push required: {} upstream commit{} will be overwritten.",
                            upstream_commits.len(),
                            if upstream_commits.len() == 1 { "" } else { "s" }
                        ))
                    } else {
                        None
                    };

                    // Determine if this branch is stacked on another branch
                    // by finding a branch whose tip matches this branch's base_commit
                    let stacked_on = stack_details
                        .branch_details
                        .iter()
                        .find(|b| {
                            b.tip == branch_detail.base_commit && b.name != branch_detail.name
                        })
                        .map(|b| b.name.to_string());

                    dry_run_infos.push(DryRunBranchInfo {
                        branch_name: branch_name.clone(),
                        stack_name: stack_name.clone(),
                        unpushed_commits: *unpushed_count,
                        remote: remote.clone(),
                        remote_ref: push_details.remote_refname.to_string(),
                        commits,
                        upstream_commits,
                        requires_force,
                        warning,
                        stacked_on,
                    });

                    break;
                }
            }
        }
    }

    // Output JSON if requested
    if let Some(out) = out.for_json() {
        out.write_value(&DryRunResult {
            branches: dry_run_infos.clone(),
        })?;
    }

    // Output human-readable format
    if out.for_human().is_some() {
        writeln!(progress)?;
        writeln!(
            progress,
            "{} {}",
            "Dry run:".bright_blue().bold(),
            "Showing what would be pushed".dimmed()
        )?;
        writeln!(progress)?;

        // Group branches by stack
        let mut branches_by_stack: std::collections::HashMap<String, Vec<&DryRunBranchInfo>> =
            std::collections::HashMap::new();
        for info in &dry_run_infos {
            branches_by_stack
                .entry(info.stack_name.clone())
                .or_default()
                .push(info);
        }

        let mut stack_names: Vec<_> = branches_by_stack.keys().collect();
        stack_names.sort();

        for stack_name in stack_names {
            let branches = branches_by_stack.get(stack_name).unwrap();

            // Highlight stacked branches (multiple branches in same stack)
            if branches.len() > 1 {
                writeln!(
                    progress,
                    "{} {} {}",
                    "Stack:".yellow().bold(),
                    stack_name.cyan(),
                    format!("({} branches)", branches.len()).dimmed()
                )?;
            }

            // Sort branches to show stacking order (top to bottom)
            let mut sorted_branches: Vec<_> = branches.to_vec();
            sorted_branches.sort_by(|a, b| {
                // If a is stacked on b, then a should come first (reverse of before)
                if a.stacked_on.as_ref() == Some(&b.branch_name) {
                    std::cmp::Ordering::Less
                } else if b.stacked_on.as_ref() == Some(&a.branch_name) {
                    std::cmp::Ordering::Greater
                } else {
                    a.branch_name.cmp(&b.branch_name)
                }
            });

            for info in sorted_branches.iter() {
                let has_stacked_on = info.stacked_on.is_some();
                let is_stacked_on = sorted_branches
                    .iter()
                    .any(|b| b.stacked_on.as_ref() == Some(&info.branch_name));

                let is_in_stack = has_stacked_on || is_stacked_on;
                let is_first = has_stacked_on && !is_stacked_on;
                let is_last = !has_stacked_on && is_stacked_on;
                let has_next = is_in_stack && !is_last;

                if is_in_stack && !is_first {
                    writeln!(progress, "{}", "│".dimmed())?;
                } else {
                    writeln!(progress)?;
                }

                // Determine the gutter character
                let gutter = if is_in_stack {
                    if is_first {
                        "┌─" // Top branch in stack
                    } else if is_last {
                        "└─" // Bottom branch in stack
                    } else {
                        "├─" // Middle branch
                    }
                } else {
                    "  " // Base branch (no parent)
                };

                // Display branch name with stacking indicator and visual line
                if let Some(stacked_on) = &info.stacked_on {
                    writeln!(
                        progress,
                        "{} {} {} {} {}",
                        gutter.dimmed(),
                        "Branch:".bold(),
                        info.branch_name.cyan().bold(),
                        "↑".dimmed(),
                        format!("(on top of {})", stacked_on).blue()
                    )?;
                } else {
                    writeln!(
                        progress,
                        "{} {} {}",
                        gutter.dimmed(),
                        "Branch:".bold(),
                        info.branch_name.cyan().bold()
                    )?;
                }

                // Extract branch name from remote_ref (e.g., refs/remotes/origin/branch -> branch)
                let branch_name = info
                    .remote_ref
                    .strip_prefix("refs/remotes/")
                    .and_then(|s| s.strip_prefix(&format!("{}/", info.remote)))
                    .unwrap_or(&info.remote_ref);

                // Determine the line prefix for details (vertical line or space)
                // Show line if there are more branches after this one
                let line_prefix = if has_next { "│ " } else { "  " };

                writeln!(
                    progress,
                    "{}  {} {} {}",
                    line_prefix.dimmed(),
                    "→".green(),
                    "Would push to:".dimmed(),
                    format!("{}/{}", info.remote, branch_name).yellow()
                )?;
                writeln!(
                    progress,
                    "{}  {} {}",
                    line_prefix.dimmed(),
                    "Commits:".dimmed(),
                    format!(
                        "{} unpushed commit{}",
                        info.unpushed_commits,
                        if info.unpushed_commits == 1 { "" } else { "s" }
                    )
                    .yellow()
                )?;

                if !info.commits.is_empty() {
                    if is_in_stack {
                        writeln!(progress, "{}", line_prefix.dimmed())?;
                    } else {
                        writeln!(progress)?;
                    }
                    for commit in &info.commits {
                        writeln!(
                            progress,
                            "{}    {} {}",
                            line_prefix.dimmed(),
                            commit.sha_short.green(),
                            commit.message.dimmed()
                        )?;
                    }

                    if info.unpushed_commits > info.commits.len() {
                        writeln!(
                            progress,
                            "{}    {}",
                            line_prefix.dimmed(),
                            format!(
                                "... and {} more",
                                info.unpushed_commits - info.commits.len()
                            )
                            .dimmed()
                        )?;
                    }
                }

                // Show upstream commits if any
                if !info.upstream_commits.is_empty() {
                    writeln!(progress)?;
                    writeln!(
                        progress,
                        "{}  {} {} {}",
                        line_prefix.dimmed(),
                        "⚠".yellow(),
                        "Upstream commits (on remote):".yellow(),
                        format!(
                            "{} commit{}",
                            info.upstream_commits.len(),
                            if info.upstream_commits.len() == 1 {
                                ""
                            } else {
                                "s"
                            }
                        )
                        .yellow()
                    )?;
                    writeln!(progress)?;
                    for commit in &info.upstream_commits {
                        writeln!(
                            progress,
                            "{}    {} {}",
                            line_prefix.dimmed(),
                            commit.sha_short.red(),
                            commit.message.dimmed()
                        )?;
                    }
                }

                // Show warning if present
                if let Some(warning) = &info.warning {
                    writeln!(progress)?;
                    writeln!(
                        progress,
                        "{}  {} {}",
                        line_prefix.dimmed(),
                        "⚠".red().bold(),
                        warning.red()
                    )?;
                }

                // Show force push indicator
                if info.requires_force {
                    writeln!(progress)?;
                    writeln!(
                        progress,
                        "{}  {} {}",
                        line_prefix.dimmed(),
                        "⚡".yellow(),
                        "Force push required".yellow()
                    )?;
                }
            }

            writeln!(progress)?;
        }

        let total_commits: usize = dry_run_infos.iter().map(|i| i.unpushed_commits).sum();
        let total_branches = dry_run_infos.len();

        writeln!(progress)?;
        writeln!(
            progress,
            "{} Would push {} {} across {} {}",
            "Summary:".bright_blue().bold(),
            total_commits.to_string().yellow().bold(),
            if total_commits == 1 {
                "commit"
            } else {
                "commits"
            },
            total_branches.to_string().cyan().bold(),
            if total_branches == 1 {
                "branch"
            } else {
                "branches"
            }
        )?;
        writeln!(progress)?;
        writeln!(
            progress,
            "{}",
            "Run without --dry-run to push these changes.".dimmed()
        )?;
    }

    Ok(())
}

fn push_single_branch(
    ctx: &Context,
    project: &Project,
    branch_name: &str,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let result = push_single_branch_impl(ctx, project, branch_name, args, gerrit_mode)?;
    let mut progress = out.progress_channel();

    if let Some(out) = out.for_json() {
        out.write_value(&result)?;
    }

    if out.for_human().is_some() {
        writeln!(progress)?;
        writeln!(
            progress,
            "{} Push completed successfully",
            "✓".green().bold()
        )?;
        writeln!(progress)?;
        if !result.branch_sha_updates.is_empty() {
            for (branch, before_sha, after_sha) in &result.branch_sha_updates {
                let before_str = if before_sha == "0000000000000000000000000000000000000000" {
                    "(new branch)".to_string()
                } else {
                    before_sha.chars().take(7).collect()
                };
                let after_str: String = after_sha.chars().take(7).collect();

                // Construct simple remote ref format: remote/branch
                let remote_ref = format!("{}/{}", result.remote, branch);

                writeln!(
                    progress,
                    "  {} -> {} ({} -> {})",
                    branch.cyan(),
                    remote_ref.dimmed(),
                    before_str.dimmed(),
                    after_str.green()
                )?;
            }
        }
    }

    Ok(())
}

// Shared implementation for pushing a single branch
fn push_single_branch_impl(
    _ctx: &Context,
    project: &Project,
    branch_name: &str,
    args: &Command,
    gerrit_mode: bool,
) -> anyhow::Result<PushResult> {
    // Find stack_id from branch name
    let stack_id = find_stack_id_by_branch_name(project, branch_name)?;

    // Convert CLI args to gerrit flags with validation
    let gerrit_flags = get_gerrit_flags(args, branch_name, gerrit_mode)?;

    // Call push_stack
    let result: PushResult = but_api::legacy::stack::push_stack(
        project.id,
        stack_id,
        args.with_force,
        args.skip_force_push_protection,
        branch_name.to_string(),
        args.run_hooks,
        gerrit_flags,
    )?;

    Ok(result)
}

fn push_all_branches(
    ctx: &Context,
    project: &Project,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let mut progress = out.progress_channel();
    let branches_with_info = get_branches_with_unpushed_info(ctx, project)?;

    // Filter to only branches with unpushed commits
    let branches_to_push: Vec<_> = branches_with_info
        .into_iter()
        .filter(|(_, count, _)| *count > 0)
        .collect();

    if branches_to_push.is_empty() {
        // Output empty result for JSON
        if let Some(out) = out.for_json() {
            let batch_result = BatchPushResult {
                pushed: vec![],
                failed: vec![],
            };
            out.write_value(&batch_result)?;
        }

        if out.for_human().is_some() {
            writeln!(
                progress,
                "{}",
                "No branches have unpushed commits.".dimmed()
            )?;
        }
        return Ok(());
    }

    if out.for_human().is_some() {
        writeln!(progress)?;
        writeln!(progress, "{}", "Pushing branches...".bright_blue().bold())?;
        writeln!(progress)?;
    }

    let mut total_commits_pushed = 0;
    let mut pushed_results = Vec::new();
    let mut failed_branches = Vec::new();

    for (branch_name, unpushed_count, _) in branches_to_push {
        if out.for_human().is_some() {
            write!(progress, "  {} {}... ", "→".cyan(), branch_name.bold())?;
        }

        match push_single_branch_impl(ctx, project, &branch_name, args, gerrit_mode) {
            Ok(result) => {
                total_commits_pushed += unpushed_count;
                if out.for_human().is_some() {
                    writeln!(
                        progress,
                        "{} ({} commit{})",
                        "✓".green(),
                        unpushed_count.to_string().yellow(),
                        if unpushed_count == 1 { "" } else { "s" }
                    )?;
                }
                pushed_results.push(result);
            }
            Err(e) => {
                failed_branches.push(FailedBranch {
                    branch_name: branch_name.clone(),
                    error: e.to_string(),
                });
                if out.for_human().is_some() {
                    writeln!(progress, "{} {}", "✗".red(), e.to_string().red())?;
                }
            }
        }
    }

    // Output JSON if requested
    if let Some(out) = out.for_json() {
        let batch_result = BatchPushResult {
            pushed: pushed_results.clone(),
            failed: failed_branches.clone(),
        };
        out.write_value(&batch_result)?;
    }

    if out.for_human().is_some() {
        writeln!(progress)?;

        if !pushed_results.is_empty() {
            writeln!(
                progress,
                "{} {} {} {}",
                "✓".green().bold(),
                "Successfully pushed".green().bold(),
                total_commits_pushed.to_string().yellow().bold(),
                if total_commits_pushed == 1 {
                    "commit"
                } else {
                    "commits"
                }
            )?;
            writeln!(progress)?;

            // Print combined branch, remote, and SHA information for all pushed branches
            for result in &pushed_results {
                for (branch, before_sha, after_sha) in &result.branch_sha_updates {
                    let before_str = if before_sha == "0000000000000000000000000000000000000000" {
                        "(new branch)".to_string()
                    } else {
                        before_sha.chars().take(7).collect()
                    };
                    let after_str: String = after_sha.chars().take(7).collect();

                    // Construct simple remote ref format: remote/branch
                    let remote_ref = format!("{}/{}", result.remote, branch);

                    writeln!(
                        progress,
                        "  {} -> {} ({} -> {})",
                        branch.cyan(),
                        remote_ref.dimmed(),
                        before_str.dimmed(),
                        after_str.green()
                    )?;
                }
            }
        }

        if !failed_branches.is_empty() {
            writeln!(progress)?;
            writeln!(
                progress,
                "{} Failed to push {} {}:",
                "✗".red().bold(),
                failed_branches.len().to_string().red().bold(),
                if failed_branches.len() == 1 {
                    "branch"
                } else {
                    "branches"
                }
            )?;
            for failed in &failed_branches {
                writeln!(
                    progress,
                    "    {} - {}",
                    failed.branch_name.red(),
                    failed.error.dimmed()
                )?;
            }
        }
    }

    Ok(())
}

fn handle_no_branch_specified(
    ctx: &Context,
    project: &Project,
    out: &mut OutputChannel,
) -> anyhow::Result<BranchSelection> {
    let branches_with_info = get_branches_with_unpushed_info(ctx, project)?;

    if branches_with_info.is_empty() {
        anyhow::bail!("No branches found in the workspace");
    }

    // Check if we're in an interactive terminal
    let is_interactive = std::io::stdin().is_terminal() && out.for_human().is_some();
    if !is_interactive {
        tracing::info!(
            "Non-interactive mode detected. Pushing all branches with unpushed commits..."
        );
        // Non-interactive mode: push all branches with unpushed commits
        return Ok(BranchSelection::All);
    }

    // Interactive mode: show branches and prompt for selection
    let mut progress = out.progress_channel();
    if out.for_human().is_some() {
        let mut has_unpushed = false;

        for (_branch_name, unpushed_count, _stack_name) in &branches_with_info {
            if *unpushed_count > 0 {
                has_unpushed = true;
            }
        }

        if !has_unpushed {
            writeln!(progress)?;
            writeln!(
                progress,
                "{}",
                "✓ All branches are up to date with the remote.".green()
            )?;
            return Ok(BranchSelection::All);
        }

        writeln!(progress)?;

        // Create selection options
        let mut options = vec!["all - Push all branches with unpushed commits".to_string()];
        for (branch_name, unpushed_count, _) in &branches_with_info {
            if *unpushed_count > 0 {
                options.push(format!(
                    "{} - {} unpushed commit{}",
                    branch_name,
                    unpushed_count,
                    if *unpushed_count == 1 { "" } else { "s" }
                ));
            }
        }

        let prompt = cli_prompts::prompts::Selection::new(
            "Which branch(es) would you like to push?",
            options.into_iter(),
        );

        let selection = prompt
            .display()
            .map_err(|e| anyhow::anyhow!("Selection aborted: {:?}", e))?;

        // Parse the selection
        if selection.starts_with("all ") {
            Ok(BranchSelection::All)
        } else {
            // Extract branch name from the selection
            let branch_name = selection
                .split(" - ")
                .next()
                .ok_or_else(|| anyhow::anyhow!("Invalid selection"))?;
            Ok(BranchSelection::Single(branch_name.to_string()))
        }
    } else {
        Err(anyhow::anyhow!(
            "Human output required for interactive prompt"
        ))
    }
}

fn get_branches_with_unpushed_info(
    _ctx: &Context,
    project: &Project,
) -> anyhow::Result<Vec<(String, usize, String)>> {
    let stacks = but_api::legacy::workspace::stacks(
        project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    let mut branches_info = Vec::new();

    for stack in stacks {
        if let Some(stack_id) = stack.id {
            let stack_details =
                but_api::legacy::workspace::stack_details(project.id, Some(stack_id))?;
            let stack_name = stack
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unnamed".to_string());

            // Get branch names from the heads
            for head in &stack.heads {
                let branch_name = head.name.to_string();

                // Find the corresponding branch details to count unpushed commits
                let unpushed_count = if let Some(branch_detail) = stack_details
                    .branch_details
                    .iter()
                    .find(|b| b.name == head.name)
                {
                    // Count only commits that are LocalOnly (not pushed to remote)
                    // LocalAndRemote means it exists on both, Integrated means it's already in base
                    let local_only_count = branch_detail
                        .commits
                        .iter()
                        .filter(|c| matches!(c.state, but_workspace::ui::CommitState::LocalOnly))
                        .count();

                    // Additionally check if push_status indicates there are unpushed commits
                    // even if we don't find any LocalOnly commits (e.g., for new branches)
                    match branch_detail.push_status {
                        but_workspace::ui::PushStatus::CompletelyUnpushed => {
                            // All commits on the branch need to be pushed
                            branch_detail.commits.len().max(local_only_count)
                        }
                        but_workspace::ui::PushStatus::UnpushedCommits
                        | but_workspace::ui::PushStatus::UnpushedCommitsRequiringForce => {
                            // There are commits to push
                            local_only_count.max(1) // At least 1 if push_status says so
                        }
                        _ => local_only_count,
                    }
                } else {
                    // If no detailed branch info found, assume no unpushed commits
                    0
                };

                branches_info.push((branch_name, unpushed_count, stack_name.clone()));
            }
        }
    }

    // Sort by stack name and then by branch name for consistent ordering
    branches_info.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));

    Ok(branches_info)
}

pub fn get_gerrit_flags(
    args: &Command,
    branch_name: &str,
    gerrit_mode: bool,
) -> anyhow::Result<Vec<but_gerrit::PushFlag>> {
    let has_gerrit_flag = args.wip
        || args.ready
        || !args.hashtag.is_empty()
        || args.topic.is_some()
        || args.topic_from_branch
        || args.private;

    if has_gerrit_flag && !gerrit_mode {
        return Err(anyhow::anyhow!(
            "Gerrit push flags (--wip, --ready, --hashtag/--tag, --topic, --topic-from-branch, --private) can only be used when gerrit_mode is enabled for this repository"
        ));
    }

    if !gerrit_mode {
        return Ok(vec![]);
    }

    let mut flags = Vec::new();

    // Handle Wip/Ready - Ready is default if neither is specified
    if args.wip {
        flags.push(but_gerrit::PushFlag::Wip);
    } else {
        // Default to Ready, or explicit Ready
        flags.push(but_gerrit::PushFlag::Ready);
    }

    // Handle hashtags - can be multiple
    for hashtag in &args.hashtag {
        if hashtag.trim().is_empty() {
            return Err(anyhow::anyhow!("Hashtag cannot be empty"));
        }
        flags.push(but_gerrit::PushFlag::Hashtag(hashtag.clone()));
    }

    // Handle topic - at most one
    if let Some(topic) = &args.topic {
        if topic.trim().is_empty() {
            return Err(anyhow::anyhow!("Topic cannot be empty"));
        }
        flags.push(but_gerrit::PushFlag::Topic(topic.clone()));
    } else if args.topic_from_branch {
        flags.push(but_gerrit::PushFlag::Topic(branch_name.to_string()));
    }

    // Handle private flag
    if args.private {
        flags.push(but_gerrit::PushFlag::Private);
    }

    Ok(flags)
}

fn resolve_branch_name(
    ctx: &mut Context,
    id_map: &IdMap,
    branch_id: &str,
) -> anyhow::Result<String> {
    // Try to resolve as CliId first
    let cli_ids = id_map.resolve_entity_to_ids(branch_id)?;

    if cli_ids.is_empty() {
        // If no CliId matches, treat as literal branch name but validate it exists
        let available_branches = get_available_branch_names(ctx)?;
        if !available_branches.contains(&branch_id.to_string()) {
            return Err(anyhow::anyhow!(
                "Branch '{}' not found. Available branches:\n{}",
                branch_id,
                format_branch_suggestions(&available_branches)
            ));
        }
        return Ok(branch_id.to_string());
    }

    if cli_ids.len() > 1 {
        let branch_names: Vec<String> = cli_ids
            .iter()
            .filter_map(|id| match id {
                CliId::Branch { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();

        if !branch_names.is_empty() {
            return Err(anyhow::anyhow!(
                "Ambiguous branch identifier '{}'. Did you mean one of:\n{}",
                branch_id,
                format_branch_suggestions(&branch_names)
            ));
        } else {
            return Err(anyhow::anyhow!(
                "Identifier '{}' matches multiple non-branch items. Please use a branch name or branch CLI ID.",
                branch_id
            ));
        }
    }

    match &cli_ids[0] {
        CliId::Branch { name, .. } => Ok(name.clone()),
        _ => Err(anyhow::anyhow!(
            "Expected branch identifier, got {}. Please use a branch name or branch CLI ID.",
            cli_ids[0].kind_for_humans()
        )),
    }
}

fn get_available_branch_names(ctx: &Context) -> anyhow::Result<Vec<String>> {
    let stacks = crate::legacy::commits::stacks(ctx)?;
    let mut branch_names = Vec::new();

    for stack in stacks {
        for head in &stack.heads {
            branch_names.push(head.name.to_string());
        }
    }

    branch_names.sort();
    branch_names.dedup();
    Ok(branch_names)
}

fn format_branch_suggestions(branches: &[String]) -> String {
    if branches.is_empty() {
        return "  (no branches available)".to_string();
    }

    branches
        .iter()
        .map(|name| format!("  - {}", name))
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_stack_id_by_branch_name(project: &Project, branch_name: &str) -> anyhow::Result<StackId> {
    let stacks = but_api::legacy::workspace::stacks(
        project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Find which stack this branch belongs to
    for stack_entry in &stacks {
        if stack_entry.heads.iter().any(|b| b.name == branch_name) && stack_entry.id.is_some() {
            return Ok(stack_entry.id.unwrap());
        }
    }

    // This should rarely happen now since we validate branch existence earlier,
    // but provide a helpful error just in case
    let available_branches: Vec<String> = stacks
        .iter()
        .flat_map(|s| s.heads.iter().map(|h| h.name.to_string()))
        .collect();

    Err(anyhow::anyhow!(
        "Branch '{}' not found in any stack. Available branches:\n{}",
        branch_name,
        format_branch_suggestions(&available_branches)
    ))
}
