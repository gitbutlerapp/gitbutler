use std::fmt::Write;

use but_core::{RepositoryExt, ref_metadata::StackId, sync::RepoShared};
use but_ctx::Context;
use cli_prompts::DisplayPrompt;
use gitbutler_git::PushResult;
use serde::Serialize;

use crate::{
    CliId, IdMap,
    args::{push, push::Command},
    command::legacy::workspace_target,
    theme::{self, Paint},
    utils::{OutputChannel, shorten_hex_object_id, shorten_object_id},
};

/// Represents the result of branch selection when no branch is specified
enum BranchSelection {
    /// Push a single branch
    Single(String),
    /// Push all branches with unpushed commits
    All,
    /// User declined to push
    None,
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

pub async fn handle(
    args: push::Command,
    ctx: &mut Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Check gerrit mode early
    let gerrit_mode = {
        let repo = ctx.repo.get()?;
        repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false)
    };

    // If dry-run, show what would be pushed
    if args.dry_run {
        return handle_dry_run(ctx, &args.branch_id, out);
    }

    let guard = ctx.shared_worktree_access();
    let perm = guard.read_permission();
    let id_map = IdMap::new_from_context(ctx, None, perm)?;

    // If no branch_id is provided, show all branches and prompt or push all
    let branch_selection = if let Some(ref branch_id) = args.branch_id {
        // Resolve branch_id to actual branch name
        let branch_name = resolve_branch_name(ctx, &id_map, branch_id)?;
        BranchSelection::Single(branch_name)
    } else {
        handle_no_branch_specified(ctx, out)?
    };

    // Handle branch selection
    let had_successful_push = match branch_selection {
        BranchSelection::All => push_all_branches(ctx, perm, &args, gerrit_mode, out)?,
        BranchSelection::Single(branch_name) => {
            push_single_branch(ctx, perm, &branch_name, &args, gerrit_mode, out)?;
            true
        }
        BranchSelection::None => return Ok(()),
    };

    // Best-effort: update PR/MR target branches to match the current stack structure.
    if had_successful_push && let Err(err) = update_review_targets_for_stacks(ctx, perm).await {
        tracing::warn!(?err, "Failed to update review target branches after push");
    }

    Ok(())
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
    remote_ref: gix::refs::FullName,
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

/// Dry-run push destination details derived from branch metadata.
#[derive(Debug, Clone)]
struct DryRunPushDetails {
    /// The remote-tracking reference that would be updated.
    remote_ref: gix::refs::FullName,
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
    branch_id: &Option<String>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let t = theme::get();
    let mut progress = out.progress_channel();

    // Fetch from remote first to get latest state
    writeln!(progress, "Fetching from remote...")?;

    but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("dry_run_push".into()))?;

    // Get all branches with info
    let branches_with_info = get_branches_with_unpushed_info(ctx)?;

    // Filter based on branch_id if provided
    let branches_to_show: Vec<_> = if let Some(branch_id) = branch_id {
        // Resolve branch name
        let id_map = IdMap::legacy_new_from_context(ctx, None)?;
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

        writeln!(
            progress,
            "{}",
            t.hint.paint("No branches have unpushed commits.")
        )?;
        return Ok(());
    }

    // Get detailed information for each branch
    let mut dry_run_infos = Vec::new();

    let stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Limit the shared lock to target resolution before continuing with dry-run analysis.
    let remote = {
        let guard = ctx.shared_worktree_access();
        workspace_target::ResolvedTarget::resolve_with_perm(ctx, guard.read_permission())?
            .push_remote_name()
            .map(str::to_owned)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to determine push remote for dry-run push: workspace target has no push remote."
                )
            })?
    };
    let repo = ctx.repo.get()?.clone().for_commit_shortening();
    let remote_names = repo.remote_names();
    for (branch_name, unpushed_count, stack_name) in &branches_to_show {
        // Find the stack containing this branch
        for stack_entry in &stacks {
            if let Some(stack_id) = stack_entry.id {
                let stack_details = but_api::legacy::workspace::stack_details(ctx, Some(stack_id))?;

                // Find the branch details
                if let Some(branch_detail) = stack_details
                    .branch_details
                    .iter()
                    .find(|b| b.name == branch_name.as_str())
                {
                    let push_details = dry_run_push_details(branch_detail, &remote)?;

                    // Collect commit information
                    let commits: Vec<DryRunCommit> = branch_detail
                        .commits
                        .iter()
                        .filter(|c| matches!(c.state, but_workspace::ui::CommitState::LocalOnly))
                        .take(10) // Limit to first 10 commits for display
                        .map(|c| {
                            let sha = c.id.to_string();
                            let sha_short = shorten_object_id(&repo, c.id);
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
                            let sha_short = shorten_object_id(&repo, c.id);
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
                        remote_ref: push_details.remote_ref,
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
    writeln!(progress)?;
    writeln!(
        progress,
        "{} {}",
        t.important.paint("Dry run:"),
        t.hint.paint("Showing what would be pushed")
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
                t.attention.paint("Stack:"),
                t.local_branch.paint(stack_name),
                t.hint.paint(format!("({} branches)", branches.len()))
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
                writeln!(progress, "{}", t.hint.paint("│"))?;
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
                    t.hint.paint(gutter),
                    t.important.paint("Branch:"),
                    t.local_branch.paint(&info.branch_name),
                    t.hint.paint("↑"),
                    t.info.paint(format!("(on top of {stacked_on})"))
                )?;
            } else {
                writeln!(
                    progress,
                    "{} {} {}",
                    t.hint.paint(gutter),
                    t.important.paint("Branch:"),
                    t.local_branch.paint(&info.branch_name)
                )?;
            }

            // Extract branch name from remote_ref (e.g., refs/remotes/origin/branch -> branch)
            let branch_name = but_core::extract_remote_name_and_short_name(
                info.remote_ref.as_ref(),
                &remote_names,
            )
            .map(|(_, short_name)| short_name.to_string())
            .unwrap_or_else(|| info.remote_ref.shorten().to_string());

            // Determine the line prefix for details (vertical line or space)
            // Show line if there are more branches after this one
            let line_prefix = if has_next { "│ " } else { "  " };

            writeln!(
                progress,
                "{}  {} {} {}",
                t.hint.paint(line_prefix),
                t.success.paint("→"),
                t.hint.paint("Would push to:"),
                t.remote_branch
                    .paint(format!("{}/{}", info.remote, branch_name))
            )?;
            writeln!(
                progress,
                "{}  {} {} unpushed commit{}",
                t.hint.paint(line_prefix),
                t.hint.paint("Commits:"),
                info.unpushed_commits,
                if info.unpushed_commits == 1 { "" } else { "s" }
            )?;

            if !info.commits.is_empty() {
                if is_in_stack {
                    writeln!(progress, "{}", t.hint.paint(line_prefix))?;
                } else {
                    writeln!(progress)?;
                }
                for commit in &info.commits {
                    writeln!(
                        progress,
                        "{}    {} {}",
                        t.hint.paint(line_prefix),
                        t.commit_id.paint(&commit.sha_short),
                        t.hint.paint(&commit.message)
                    )?;
                }

                if info.unpushed_commits > info.commits.len() {
                    writeln!(
                        progress,
                        "{}    ... and {} more",
                        t.hint.paint(line_prefix),
                        info.unpushed_commits - info.commits.len()
                    )?;
                }
            }

            // Show upstream commits if any
            if !info.upstream_commits.is_empty() {
                writeln!(progress)?;
                writeln!(
                    progress,
                    "{}  {} {} {} commit{}",
                    t.hint.paint(line_prefix),
                    t.sym().warning,
                    t.attention.paint("Upstream commits (on remote):"),
                    info.upstream_commits.len(),
                    if info.upstream_commits.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )?;
                writeln!(progress)?;
                for commit in &info.upstream_commits {
                    writeln!(
                        progress,
                        "{}    {} {}",
                        t.hint.paint(line_prefix),
                        t.error.paint(&commit.sha_short),
                        t.hint.paint(&commit.message)
                    )?;
                }
            }

            // Show warning if present
            if let Some(warning) = &info.warning {
                writeln!(progress)?;
                writeln!(
                    progress,
                    "{}  {} {}",
                    t.hint.paint(line_prefix),
                    t.sym().warning.error(),
                    t.error.paint(warning)
                )?;
            }

            // Show force push indicator
            if info.requires_force {
                writeln!(progress)?;
                writeln!(
                    progress,
                    "{}  {} {}",
                    t.hint.paint(line_prefix),
                    t.sym().lightning,
                    t.attention.paint("Force push required")
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
        t.important.paint("Summary:"),
        t.attention.paint(total_commits.to_string()),
        if total_commits == 1 {
            "commit"
        } else {
            "commits"
        },
        t.info.paint(total_branches.to_string()),
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
        t.hint.paint("Run without --dry-run to push these changes.")
    )?;

    Ok(())
}

/// Build dry-run push destination details from branch metadata.
fn dry_run_push_details(
    branch_detail: &but_workspace::ui::BranchDetails,
    remote: &str,
) -> anyhow::Result<DryRunPushDetails> {
    let remote_ref: gix::refs::FullName =
        format!("refs/remotes/{remote}/{}", branch_detail.name).try_into()?;
    Ok(DryRunPushDetails { remote_ref })
}

fn push_single_branch(
    ctx: &mut Context,
    perm: &RepoShared,
    branch_name: &str,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let t = theme::get();
    let result = push_single_branch_impl(ctx, perm, branch_name, args, gerrit_mode)?;
    let mut progress = out.progress_channel();

    if let Some(out) = out.for_json() {
        out.write_value(&result)?;
    }

    writeln!(progress)?;
    writeln!(progress, "{} Push completed successfully", t.sym().success)?;
    writeln!(progress)?;
    if !result.branch_sha_updates.is_empty() {
        let repo = ctx.repo.get()?.clone().for_commit_shortening();
        let gerrit_review_ref = if gerrit_mode {
            Some(gerrit_review_ref(ctx, perm, &repo)?)
        } else {
            None
        };
        for (branch, before_sha, after_sha) in &result.branch_sha_updates {
            let before_str = if before_sha == "0000000000000000000000000000000000000000" {
                "(new branch)".to_string()
            } else {
                shorten_hex_object_id(&repo, before_sha)
            };
            let after_str = shorten_hex_object_id(&repo, after_sha);
            let remote_ref =
                branch_remote_ref_for_display(&result, branch, gerrit_review_ref.as_deref());

            writeln!(
                progress,
                "  {} -> {} ({} -> {})",
                t.local_branch.paint(branch),
                t.hint.paint(&remote_ref),
                t.hint.paint(&before_str),
                t.commit_id.paint(&after_str)
            )?;
        }
    }

    Ok(())
}

// Shared implementation for pushing a single branch
fn push_single_branch_impl(
    ctx: &mut Context,
    perm: &RepoShared,
    branch_name: &str,
    args: &Command,
    gerrit_mode: bool,
) -> anyhow::Result<PushResult> {
    // Check for conflicted commits before pushing
    check_for_conflicted_commits(ctx, branch_name)?;

    // Find stack_id from branch name
    let stack_id = find_stack_id_by_branch_name(ctx, branch_name)?;

    // Convert CLI args to gerrit flags with validation
    let gerrit_flags = get_gerrit_flags(args, branch_name, gerrit_mode)?;

    // Call push_stack
    let result: PushResult = but_api::legacy::stack::push_stack_with_perm(
        ctx,
        stack_id,
        args.with_force,
        args.skip_force_push_protection,
        branch_name.to_string(),
        !args.no_hooks,
        gerrit_flags,
        perm,
    )?;

    Ok(result)
}

/// Returns `true` if at least one branch was pushed successfully.
fn push_all_branches(
    ctx: &mut Context,
    perm: &RepoShared,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<bool> {
    let t = theme::get();
    let mut progress = out.progress_channel();
    let branches_with_info = get_branches_with_unpushed_info(ctx)?;

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

        writeln!(
            progress,
            "{}",
            t.hint.paint("No branches have unpushed commits.")
        )?;
        return Ok(false);
    }

    writeln!(progress)?;
    writeln!(progress, "{}", t.progress.paint("Pushing branches..."))?;
    writeln!(progress)?;

    let mut total_commits_pushed = 0;
    let mut pushed_results = Vec::new();
    let mut failed_branches = Vec::new();

    for (branch_name, unpushed_count, _) in branches_to_push {
        write!(
            progress,
            "  {} {}... ",
            t.info.paint("→"),
            t.important.paint(&branch_name)
        )?;

        match push_single_branch_impl(ctx, perm, &branch_name, args, gerrit_mode) {
            Ok(result) => {
                total_commits_pushed += unpushed_count;
                writeln!(
                    progress,
                    "{} ({} commit{})",
                    t.sym().success,
                    t.attention.paint(unpushed_count.to_string()),
                    if unpushed_count == 1 { "" } else { "s" }
                )?;
                pushed_results.push(result);
            }
            Err(e) => {
                failed_branches.push(FailedBranch {
                    branch_name: branch_name.clone(),
                    error: e.to_string(),
                });
                writeln!(
                    progress,
                    "{} {}",
                    t.sym().error,
                    t.error.paint(e.to_string())
                )?;
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

    writeln!(progress)?;

    if !pushed_results.is_empty() {
        writeln!(
            progress,
            "{} {} {} {}",
            t.sym().success,
            t.success.paint("Successfully pushed"),
            t.attention.paint(total_commits_pushed.to_string()),
            if total_commits_pushed == 1 {
                "commit"
            } else {
                "commits"
            }
        )?;
        writeln!(progress)?;

        // Print combined branch, remote, and SHA information for all pushed branches
        let repo = ctx.repo.get()?.clone().for_commit_shortening();
        let gerrit_review_ref = if gerrit_mode {
            Some(gerrit_review_ref(ctx, perm, &repo)?)
        } else {
            None
        };
        for result in &pushed_results {
            for (branch, before_sha, after_sha) in &result.branch_sha_updates {
                let before_str = if before_sha == "0000000000000000000000000000000000000000" {
                    "(new branch)".to_string()
                } else {
                    shorten_hex_object_id(&repo, before_sha)
                };
                let after_str = shorten_hex_object_id(&repo, after_sha);
                let remote_ref =
                    branch_remote_ref_for_display(result, branch, gerrit_review_ref.as_deref());

                writeln!(
                    progress,
                    "  {} -> {} ({} -> {})",
                    t.local_branch.paint(branch),
                    t.hint.paint(&remote_ref),
                    t.hint.paint(&before_str),
                    t.commit_id.paint(&after_str)
                )?;
            }
        }
    }

    if !failed_branches.is_empty() {
        writeln!(progress)?;
        writeln!(
            progress,
            "{} Failed to push {} {}:",
            t.sym().error,
            t.error.paint(failed_branches.len().to_string()),
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
                t.error.paint(&failed.branch_name),
                t.hint.paint(&failed.error)
            )?;
        }
    }

    Ok(!pushed_results.is_empty())
}

fn handle_no_branch_specified(
    ctx: &Context,
    out: &mut OutputChannel,
) -> anyhow::Result<BranchSelection> {
    let t = theme::get();
    let branches_with_info = get_branches_with_unpushed_info(ctx)?;

    if branches_with_info.is_empty() {
        // Treat an empty workspace as a no-op push instead of an error.
        // This keeps `but push` safe to call even before any stack branches exist.
        return Ok(BranchSelection::All);
    }

    // Check if we're in an interactive terminal with human output format
    if !out.can_prompt() {
        tracing::info!(
            "Non-interactive mode detected. Pushing all branches with unpushed commits..."
        );
        // Non-interactive mode: push all branches with unpushed commits
        return Ok(BranchSelection::All);
    }

    // Interactive mode: show branches and prompt for selection
    let mut progress = out.progress_channel();
    // Collect branches with unpushed commits
    let branches_with_unpushed: Vec<_> = branches_with_info
        .iter()
        .filter(|(_, unpushed_count, _)| *unpushed_count > 0)
        .collect();

    if branches_with_unpushed.is_empty() {
        writeln!(progress)?;
        writeln!(
            progress,
            "{}",
            t.success
                .paint("✓ All branches are up to date with the remote.")
        )?;
        return Ok(BranchSelection::None);
    }

    // If there's only one branch with unpushed commits, push it automatically
    if branches_with_unpushed.len() == 1 {
        let (branch_name, _unpushed_count, _) = branches_with_unpushed[0];
        return Ok(BranchSelection::Single(branch_name.clone()));
    }

    writeln!(progress)?;

    // Multiple branches with unpushed commits - let the prompt handle it
    let mut options = vec!["all - Push all branches with unpushed commits".to_string()];
    for (branch_name, unpushed_count, _) in &branches_with_unpushed {
        options.push(format!(
            "{} - {} unpushed commit{}",
            branch_name,
            unpushed_count,
            if *unpushed_count == 1 { "" } else { "s" }
        ));
    }

    let prompt = cli_prompts::prompts::Selection::new(
        "Which branch(es) would you like to push?",
        options.into_iter(),
    );

    let selection = prompt
        .display()
        .map_err(|e| anyhow::anyhow!("Selection aborted: {e:?}"))?;

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
}

fn get_branches_with_unpushed_info(ctx: &Context) -> anyhow::Result<Vec<(String, usize, String)>> {
    let stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    let mut branches_info = Vec::new();

    for stack in stacks {
        if let Some(stack_id) = stack.id {
            let stack_details = but_api::legacy::workspace::stack_details(ctx, Some(stack_id))?;
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
    let cli_ids = id_map.parse_using_context(branch_id, ctx)?;

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
                "Identifier '{branch_id}' matches multiple non-branch items. Please use a branch name or branch CLI ID."
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
        .map(|name| format!("  - {name}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn branch_remote_ref_for_display(
    result: &PushResult,
    branch: &str,
    gerrit_review_ref: Option<&str>,
) -> String {
    if let Some(review_ref) = gerrit_review_ref {
        return review_ref.to_string();
    }

    result
        .branch_to_remote
        .iter()
        .find(|(pushed_branch, _)| pushed_branch == branch)
        .map(|(_, remote_ref)| remote_ref.shorten().to_string())
        .unwrap_or_else(|| format!("{}/{}", result.remote, branch))
}

fn gerrit_review_ref(
    ctx: &Context,
    perm: &RepoShared,
    repo: &gix::Repository,
) -> anyhow::Result<String> {
    let target_ref_name = workspace_target::ResolvedTarget::resolve_with_perm(ctx, perm)?
        .ref_name()
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("Failed to determine Gerrit target branch"))?;
    let remote_names = repo.remote_names();
    let target_branch =
        but_core::extract_remote_name_and_short_name(target_ref_name.as_ref(), &remote_names)
            .map(|(_, short_name)| short_name.to_string())
            .unwrap_or_else(|| target_ref_name.shorten().to_string());

    Ok(format!("refs/for/{target_branch}"))
}

fn find_stack_id_by_branch_name(ctx: &Context, branch_name: &str) -> anyhow::Result<StackId> {
    let stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Find which stack this branch belongs to
    for stack_entry in &stacks {
        if stack_entry.heads.iter().any(|b| b.name == branch_name)
            && let Some(id) = stack_entry.id
        {
            return Ok(id);
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

/// Update PR/MR target branches to match the current stack structure.
async fn update_review_targets_for_stacks(ctx: &Context, perm: &RepoShared) -> anyhow::Result<()> {
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx, perm)?;
    let stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    let mut target_updates = Vec::new();
    for stack in &stacks {
        // heads are ordered top-first, so iterate in reverse for bottom-to-top
        let heads: Vec<(String, Option<i64>)> = stack
            .heads
            .iter()
            .rev()
            .map(|h| (h.name.to_string(), h.review_id.map(|id| id as i64)))
            .collect();
        target_updates.extend(but_forge::compute_review_target_updates(
            &heads,
            &base_branch.short_name,
        ));
    }

    if target_updates.is_empty() {
        return Ok(());
    }

    let reviews: Vec<but_forge::ForgeReviewUpdate> =
        target_updates.into_iter().map(Into::into).collect();
    but_api::legacy::forge::update_review_footers(ctx.to_sync(), reviews).await
}

/// Check if a branch contains any conflicted commits
/// Returns an error if conflicted commits are found
fn check_for_conflicted_commits(ctx: &Context, branch_name: &str) -> anyhow::Result<()> {
    let stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    let repo = ctx.repo.get()?.clone().for_commit_shortening();
    // Find the stack containing this branch and get its details
    for stack in &stacks {
        if let Some(stack_id) = stack.id {
            // Check if this stack contains the branch we're looking for
            if stack.heads.iter().any(|h| h.name == branch_name) {
                let stack_details = but_api::legacy::workspace::stack_details(ctx, Some(stack_id))?;

                // Find the branch details
                if let Some(branch_detail) = stack_details
                    .branch_details
                    .iter()
                    .find(|b| b.name == branch_name)
                {
                    // Check for conflicted commits
                    let conflicted_commits: Vec<String> = branch_detail
                        .commits
                        .iter()
                        .filter(|c| c.has_conflicts)
                        .map(|c| shorten_object_id(&repo, c.id))
                        .collect();

                    if !conflicted_commits.is_empty() {
                        return Err(anyhow::anyhow!(
                            "Cannot push branch '{}': the branch contains {} conflicted commit{}.\n\
                             Conflicted commits: {}\n\
                             Please resolve conflicts before pushing using 'but resolve <commit>'.",
                            branch_name,
                            conflicted_commits.len(),
                            if conflicted_commits.len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            conflicted_commits.join(", ")
                        ));
                    }

                    return Ok(());
                }
            }
        }
    }

    // Branch not found - this shouldn't happen as we validate earlier
    Err(anyhow::anyhow!(
        "Branch '{branch_name}' not found when checking for conflicts"
    ))
}

#[cfg(test)]
mod tests {
    use super::branch_remote_ref_for_display;
    use gitbutler_git::PushResult;

    #[test]
    fn branch_remote_ref_display_uses_recorded_remote_ref() -> anyhow::Result<()> {
        let result = PushResult {
            remote: "origin".to_string(),
            branch_to_remote: vec![(
                "feature".to_string(),
                "refs/remotes/upstream/feature".try_into()?,
            )],
            branch_sha_updates: vec![],
        };

        assert_eq!(
            branch_remote_ref_for_display(&result, "feature", None),
            "upstream/feature"
        );
        Ok(())
    }

    #[test]
    fn branch_remote_ref_display_uses_gerrit_review_ref() {
        let result = PushResult {
            remote: "origin".to_string(),
            branch_to_remote: vec![],
            branch_sha_updates: vec![],
        };

        assert_eq!(
            branch_remote_ref_for_display(&result, "feature", Some("refs/for/main")),
            "refs/for/main"
        );
    }
}
