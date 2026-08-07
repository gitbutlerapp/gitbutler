use std::fmt::Write;

use anyhow::Context as _;
use but_core::{RepositoryExt, extract_remote_name_and_short_name, sync::RepoShared};
use but_ctx::Context;
use gitbutler_git::PushResult;
use serde::Serialize;

use crate::{
    CliId, CliResultExt as _, IdMap,
    args::{push, push::Command},
    command::legacy::workspace_target,
    theme::{self, Paint},
    utils::{
        OutputChannel, merged_upstream::MergedUpstream, shorten_hex_object_id, shorten_object_id,
    },
};

/// The branches to push, resolved from the argument or interactive selection
enum BranchSelection {
    /// Push these branches (explicit argument or picker selection)
    Selected(Vec<String>),
    /// Push everything with unpushed commits (non-interactive default)
    All,
    /// Nothing to push, or the user declined
    None,
}

/// Batch push result for JSON output
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPushResult {
    /// Successfully pushed branches
    pushed: Vec<but_api::legacy::workspace::WorkspaceBranchAndAncestorsPushOutcome>,
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

    let id_map = {
        let guard = ctx.shared_worktree_access();
        IdMap::new_from_context(ctx, guard.read_permission())?
    };

    // If no branch_id is provided, show all branches and prompt or push all
    let branch_selection = if let Some(ref branch_id) = args.branch_id {
        // Resolve branch_id to actual branch name
        let branch_name = resolve_branch_name(ctx, &id_map, branch_id)?;
        BranchSelection::Selected(vec![branch_name])
    } else {
        handle_no_branch_specified(ctx, out)?
    };

    // Everything between here and the actual push (merged-upstream check,
    // conflict check, the push's own workspace computation) is silent and can
    // take a while; print feedback first so it doesn't look like a hang. The
    // All path prints its own progress and may turn out to be a no-op.
    if matches!(branch_selection, BranchSelection::Selected(_)) {
        let mut progress = out.progress_channel();
        writeln!(progress)?;
        writeln!(
            progress,
            "{}",
            theme::get().progress.paint("Preparing push...")
        )?;
    }

    // Pushing a branch that already landed in the target recreates or rewrites
    // remote state for work that is finished. The lower push layer silently
    // skips branches with an integrated push status, but that misses branches
    // that never had a remote; refuse explicitly-selected merged branches here.
    if let BranchSelection::Selected(names) = &branch_selection {
        let merged = MergedUpstream::from_ctx(ctx, args.allow_merged)?;
        for name in names {
            let full_name = gix::refs::FullName::try_from(format!("refs/heads/{name}"))?;
            merged
                .ensure_branch_not_merged(full_name.as_ref())
                .into_internal_error()?;
        }
    }

    match branch_selection {
        BranchSelection::All => {
            push_all_branches(ctx, &args, gerrit_mode, out).await?;
        }
        BranchSelection::Selected(branch_names) => {
            for branch_name in branch_names {
                push_single_branch(ctx, &branch_name, &args, gerrit_mode, out).await?;
            }
        }
        BranchSelection::None => {}
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
    upstream_commits: Vec<DryRunCommit>,
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
        let id_map = IdMap::legacy_new_from_context(ctx)?;
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

        if let Some(human) = out.for_human() {
            writeln!(
                human,
                "{}",
                t.hint.paint("No branches have unpushed commits.")
            )?;
        }
        return Ok(());
    }

    // Get detailed information for each branch
    let mut dry_run_infos = Vec::new();

    let stacks = crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx)?;

    // Limit the shared lock to target resolution before continuing with dry-run analysis.
    let fallback_remote = {
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
            if stack_entry.id.is_some()
                && let Some(branch_detail) = stack_entry.branch(branch_name)
            {
                let remote_ref = match repo
                    .branch_remote_tracking_ref_name(
                        branch_detail.reference.as_ref(),
                        gix::remote::Direction::Fetch,
                    )
                    .transpose()?
                {
                    Some(remote_ref) => remote_ref,
                    None => dry_run_remote_ref(&branch_detail.name, &fallback_remote)?,
                };
                let (remote, _) =
                    extract_remote_name_and_short_name(remote_ref.as_ref(), &remote_names)
                        .with_context(|| {
                            format!("Failed to determine remote for dry-run ref `{remote_ref}`")
                        })?;

                // Collect commit information
                let commits: Vec<DryRunCommit> = branch_detail
                    .commits
                    .iter()
                    .filter(|c| matches!(c.state, but_workspace::ui::CommitState::LocalOnly))
                    .take(10) // Limit to first 10 commits for display
                    .map(|c| dry_run_commit(&repo, c.id, &c.message))
                    .collect();

                // Collect upstream commits (commits on remote but not local)
                let upstream_commits: Vec<DryRunCommit> = branch_detail
                    .upstream_commits
                    .iter()
                    .take(10) // Limit to first 10 commits for display
                    .map(|c| dry_run_commit(&repo, c.id, &c.message))
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
                let stacked_on = stack_entry
                    .branches
                    .iter()
                    .find(|b| b.tip == branch_detail.base_commit && b.name != branch_detail.name)
                    .map(|b| b.name.clone());

                dry_run_infos.push(DryRunBranchInfo {
                    branch_name: branch_name.clone(),
                    stack_name: stack_name.clone(),
                    unpushed_commits: *unpushed_count,
                    remote,
                    remote_ref,
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

    // Output JSON if requested
    if let Some(out) = out.for_json() {
        out.write_value(&DryRunResult {
            branches: dry_run_infos.clone(),
        })?;
    }

    let Some(human) = out.for_human() else {
        return Ok(());
    };

    writeln!(human)?;
    writeln!(
        human,
        "{} {}",
        t.important.paint("Dry run:"),
        t.hint.paint("Showing what would be pushed")
    )?;
    writeln!(human)?;

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
                human,
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
                writeln!(human, "{}", t.hint.paint("│"))?;
            } else {
                writeln!(human)?;
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
                    human,
                    "{} {} {} {} {}",
                    t.hint.paint(gutter),
                    t.important.paint("Branch:"),
                    t.local_branch.paint(&info.branch_name),
                    t.hint.paint("↑"),
                    t.info.paint(format!("(on top of {stacked_on})"))
                )?;
            } else {
                writeln!(
                    human,
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
                human,
                "{}  {} {} {}",
                t.hint.paint(line_prefix),
                t.success.paint("→"),
                t.hint.paint("Would push to:"),
                t.remote_branch
                    .paint(format!("{}/{}", info.remote, branch_name))
            )?;
            writeln!(
                human,
                "{}  {} {} unpushed commit{}",
                t.hint.paint(line_prefix),
                t.hint.paint("Commits:"),
                info.unpushed_commits,
                if info.unpushed_commits == 1 { "" } else { "s" }
            )?;

            if !info.commits.is_empty() {
                if is_in_stack {
                    writeln!(human, "{}", t.hint.paint(line_prefix))?;
                } else {
                    writeln!(human)?;
                }
                for commit in &info.commits {
                    writeln!(
                        human,
                        "{}    {} {}",
                        t.hint.paint(line_prefix),
                        t.commit_id.paint(&commit.sha_short),
                        t.hint.paint(&commit.message)
                    )?;
                }

                if info.unpushed_commits > info.commits.len() {
                    writeln!(
                        human,
                        "{}    ... and {} more",
                        t.hint.paint(line_prefix),
                        info.unpushed_commits - info.commits.len()
                    )?;
                }
            }

            // Show upstream commits if any
            if !info.upstream_commits.is_empty() {
                writeln!(human)?;
                writeln!(
                    human,
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
                writeln!(human)?;
                for commit in &info.upstream_commits {
                    writeln!(
                        human,
                        "{}    {} {}",
                        t.hint.paint(line_prefix),
                        t.error.paint(&commit.sha_short),
                        t.hint.paint(&commit.message)
                    )?;
                }
            }

            // Show warning if present
            if let Some(warning) = &info.warning {
                writeln!(human)?;
                writeln!(
                    human,
                    "{}  {} {}",
                    t.hint.paint(line_prefix),
                    t.sym().warning.error(),
                    t.error.paint(warning)
                )?;
            }

            // Show force push indicator
            if info.requires_force {
                writeln!(human)?;
                writeln!(
                    human,
                    "{}  {} {}",
                    t.hint.paint(line_prefix),
                    t.sym().lightning,
                    t.attention.paint("Force push required")
                )?;
            }
        }

        writeln!(human)?;
    }

    let total_commits: usize = dry_run_infos.iter().map(|i| i.unpushed_commits).sum();
    let total_branches = dry_run_infos.len();

    writeln!(human)?;
    writeln!(
        human,
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
    writeln!(human)?;
    writeln!(
        human,
        "{}",
        t.hint.paint("Run without --dry-run to push these changes.")
    )?;

    Ok(())
}

fn dry_run_commit(
    repo: &gix::Repository,
    id: gix::ObjectId,
    message: &bstr::BString,
) -> DryRunCommit {
    DryRunCommit {
        sha_short: shorten_object_id(repo, id),
        sha: id.to_string(),
        message: message.to_string().lines().next().unwrap_or("").to_string(),
    }
}

fn dry_run_remote_ref(branch_name: &str, remote: &str) -> anyhow::Result<gix::refs::FullName> {
    Ok(format!("refs/remotes/{remote}/{branch_name}").try_into()?)
}

async fn push_single_branch(
    ctx: &mut Context,
    branch_name: &str,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let t = theme::get();
    let result = push_single_branch_impl(ctx, branch_name, args, gerrit_mode).await?;

    if let Some(out) = out.for_json() {
        out.write_value(&result)?;
    }

    if let Some(human) = out.for_human() {
        writeln!(human)?;
        writeln!(human, "{} Push completed successfully", t.sym().success)?;
        writeln!(human)?;
        if !result.push.branch_sha_updates.is_empty() {
            let repo = ctx.repo.get()?.clone().for_commit_shortening();
            let gerrit_review_ref = if gerrit_mode {
                let guard = ctx.shared_worktree_access();
                Some(gerrit_review_ref(ctx, guard.read_permission(), &repo)?)
            } else {
                None
            };
            for (branch, before_sha, after_sha) in &result.push.branch_sha_updates {
                let before_str = if before_sha == "0000000000000000000000000000000000000000" {
                    "(new branch)".to_string()
                } else {
                    shorten_hex_object_id(&repo, before_sha)
                };
                let after_str = shorten_hex_object_id(&repo, after_sha);
                let remote_ref = branch_remote_ref_for_display(
                    &result.push,
                    branch,
                    gerrit_review_ref.as_deref(),
                );

                writeln!(
                    human,
                    "  {} -> {} ({} -> {})",
                    t.local_branch.paint(branch),
                    t.hint.paint(&remote_ref),
                    t.hint.paint(&before_str),
                    t.commit_id.paint(&after_str)
                )?;
            }
        }
        write_review_sync_warning(human, &result.review_sync)?;
    }

    Ok(())
}

// Shared implementation for pushing a single branch
async fn push_single_branch_impl(
    ctx: &mut Context,
    branch_name: &str,
    args: &Command,
    gerrit_mode: bool,
) -> anyhow::Result<but_api::legacy::workspace::WorkspaceBranchAndAncestorsPushOutcome> {
    // Check for conflicted commits before pushing
    check_for_conflicted_commits(ctx, branch_name)?;

    // Convert CLI args to gerrit flags with validation
    let gerrit_flags = get_gerrit_flags(args, branch_name, gerrit_mode)?;

    let branch = gix::refs::Category::LocalBranch.to_full_name(branch_name)?;
    but_api::legacy::workspace::workspace_branch_and_ancestors_push(
        ctx.to_sync(),
        args.with_force,
        args.skip_force_push_protection,
        branch.to_string(),
        !args.no_hooks,
        gerrit_flags,
    )
    .await
}

/// Errors if any attempted branch failed to push, so the process exits
/// non-zero; per-branch failures are still printed and reported via JSON
/// first. An up-to-date workspace with nothing to push is not an error.
async fn push_all_branches(
    ctx: &mut Context,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let t = theme::get();
    let mut progress = out.progress_channel();
    let branches_to_push = get_push_candidates(ctx)?;

    if branches_to_push.is_empty() {
        // Output empty result for JSON
        if let Some(out) = out.for_json() {
            let batch_result = BatchPushResult {
                pushed: vec![],
                failed: vec![],
            };
            out.write_value(&batch_result)?;
        }

        if let Some(human) = out.for_human() {
            writeln!(
                human,
                "{}",
                t.hint.paint("No branches have unpushed commits.")
            )?;
        }
        return Ok(());
    }

    writeln!(progress)?;
    writeln!(progress, "{}", t.progress.paint("Pushing branches..."))?;
    writeln!(progress)?;

    let mut total_commits_pushed = 0;
    let mut pushed_results = Vec::new();
    let mut failed_branches = Vec::new();

    for candidate in branches_to_push {
        let branch_name = candidate.branch_name;
        let unpushed_count = candidate.unpushed_commits;
        write!(
            progress,
            "  {} {}... ",
            t.info.paint("→"),
            t.important.paint(&branch_name)
        )?;

        match push_single_branch_impl(ctx, &branch_name, args, gerrit_mode).await {
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

    if let Some(human) = out.for_human() {
        writeln!(human)?;

        if !pushed_results.is_empty() {
            writeln!(
                human,
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
            writeln!(human)?;

            // Print combined branch, remote, and SHA information for all pushed branches
            let repo = ctx.repo.get()?.clone().for_commit_shortening();
            let gerrit_review_ref = if gerrit_mode {
                let guard = ctx.shared_worktree_access();
                Some(gerrit_review_ref(ctx, guard.read_permission(), &repo)?)
            } else {
                None
            };
            for result in &pushed_results {
                for (branch, before_sha, after_sha) in &result.push.branch_sha_updates {
                    let before_str = if before_sha == "0000000000000000000000000000000000000000" {
                        "(new branch)".to_string()
                    } else {
                        shorten_hex_object_id(&repo, before_sha)
                    };
                    let after_str = shorten_hex_object_id(&repo, after_sha);
                    let remote_ref = branch_remote_ref_for_display(
                        &result.push,
                        branch,
                        gerrit_review_ref.as_deref(),
                    );

                    writeln!(
                        human,
                        "  {} -> {} ({} -> {})",
                        t.local_branch.paint(branch),
                        t.hint.paint(&remote_ref),
                        t.hint.paint(&before_str),
                        t.commit_id.paint(&after_str)
                    )?;
                }
                write_review_sync_warning(human, &result.review_sync)?;
            }
        }

        if !failed_branches.is_empty() {
            writeln!(human)?;
            writeln!(
                human,
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
                    human,
                    "    {} - {}",
                    t.error.paint(&failed.branch_name),
                    t.hint.paint(&failed.error)
                )?;
            }
        }
    }

    if !failed_branches.is_empty() {
        let attempted = failed_branches.len() + pushed_results.len();
        anyhow::bail!(
            "failed to push {} of {} branch{}",
            failed_branches.len(),
            attempted,
            if attempted == 1 { "" } else { "es" }
        );
    }

    Ok(())
}

fn handle_no_branch_specified(
    ctx: &Context,
    out: &mut OutputChannel,
) -> anyhow::Result<BranchSelection> {
    let t = theme::get();

    // Check if we're in an interactive terminal with human output format.
    // This comes first: push_all_branches computes its own candidates, so
    // computing them here just to return All would be wasted work.
    if !out.can_prompt() {
        tracing::info!(
            "Non-interactive mode detected. Pushing all branches with unpushed commits..."
        );
        // Non-interactive mode: push all branches with unpushed commits
        return Ok(BranchSelection::All);
    }

    let candidates = get_push_candidates(ctx)?;

    // Interactive mode: show branches and prompt for selection
    let mut progress = out.progress_channel();

    // Covers the empty workspace too, where this is vacuously true; either
    // way `but push` is a safe no-op rather than an error.
    if candidates.is_empty() {
        writeln!(progress)?;
        writeln!(
            progress,
            "{}",
            t.success
                .paint("✓ All branches are up to date with the remote.")
        )?;
        return Ok(BranchSelection::None);
    }

    // A single candidate that covers only itself is unambiguous; push it
    // without prompting. If it would fold in stack ancestors, still show the
    // picker so the user sees what else the push covers.
    if candidates.len() == 1 && candidates[0].includes.is_empty() {
        return Ok(BranchSelection::Selected(vec![
            candidates[0].branch_name.clone(),
        ]));
    }

    writeln!(progress)?;

    // Multiple pushable branches - let the prompt handle it
    let options = candidates
        .iter()
        .map(|candidate| {
            let mut label = format!(
                "{} - {} unpushed commit{}",
                candidate.branch_name,
                candidate.unpushed_commits,
                if candidate.unpushed_commits == 1 {
                    ""
                } else {
                    "s"
                }
            );
            if !candidate.includes.is_empty() {
                label.push_str(&format!(" (also pushes {})", candidate.includes.join(", ")));
            }
            (label, candidate.branch_name.clone())
        })
        .collect::<Vec<_>>();
    let options = nonempty::NonEmpty::from_vec(options).context("No branches available to push")?;
    let mut input = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal")?;

    // Preselect everything so plain Enter pushes all candidates; the picker
    // starts empty otherwise, making Enter a silent no-op.
    let selected_branches = input
        .prompt_multi_select_with_help(
            "Which branch(es) would you like to push?",
            &options,
            (0..options.len()).collect(),
            Vec::new(),
            |_| None,
        )?
        .ok_or_else(|| anyhow::anyhow!("Selection aborted"))?
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();

    if selected_branches.is_empty() {
        Ok(BranchSelection::None)
    } else {
        Ok(BranchSelection::Selected(selected_branches))
    }
}

/// A branch worth offering for push: the topmost branch with unpushed commits
/// in its stack. Pushing a branch always pushes its stack ancestors too, so
/// lower branches of the same stack are folded into one candidate instead of
/// being offered (and pushed) separately.
struct PushCandidate {
    branch_name: String,
    /// Unpushed commits across this branch and its ancestors.
    unpushed_commits: usize,
    /// Ancestor branches with unpushed commits that this push also covers.
    includes: Vec<String>,
}

/// Returns one push candidate per stack that has unpushed commits.
fn get_push_candidates(ctx: &Context) -> anyhow::Result<Vec<PushCandidate>> {
    let stacks = crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx)?;

    let mut candidates = Vec::new();
    for stack in &stacks {
        if stack.id.is_none() {
            continue;
        }
        // Branches are ordered topmost-first; the first one with unpushed
        // commits is the candidate, everything below it comes along.
        let mut unpushed = stack.branches.iter().filter_map(|branch| {
            let count = branch_unpushed_count(branch);
            (count > 0).then(|| (branch.name.clone(), count))
        });
        if let Some((branch_name, count)) = unpushed.next() {
            let mut unpushed_commits = count;
            let mut includes = Vec::new();
            for (name, count) in unpushed {
                unpushed_commits += count;
                includes.push(name);
            }
            candidates.push(PushCandidate {
                branch_name,
                unpushed_commits,
                includes,
            });
        }
    }
    Ok(candidates)
}

fn branch_unpushed_count(branch: &crate::legacy::workspace::HeadInfoBranch) -> usize {
    // Count only commits that are LocalOnly (not pushed to remote).
    // LocalAndRemote means it exists on both, Integrated means it's already in base.
    let local_only_count = branch
        .commits
        .iter()
        .filter(|c| matches!(c.state, but_workspace::ui::CommitState::LocalOnly))
        .count();

    // Additionally check if push_status indicates there are unpushed commits
    // even if we don't find any LocalOnly commits (e.g., for new branches).
    match branch.push_status {
        but_workspace::ui::PushStatus::CompletelyUnpushed => {
            // All commits on the branch need to be pushed
            branch.commits.len().max(local_only_count)
        }
        but_workspace::ui::PushStatus::UnpushedCommits
        | but_workspace::ui::PushStatus::UnpushedCommitsRequiringForce => {
            // There are commits to push
            local_only_count.max(1) // At least 1 if push_status says so
        }
        _ => local_only_count,
    }
}

fn get_branches_with_unpushed_info(ctx: &Context) -> anyhow::Result<Vec<(String, usize, String)>> {
    let stacks = crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx)?;

    let mut branches_info = Vec::new();

    for stack in stacks {
        if stack.id.is_some() {
            let stack_name = stack
                .top_branch_name()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "unnamed".to_string());

            // Get branch names from the heads
            for branch in &stack.branches {
                branches_info.push((
                    branch.name.clone(),
                    branch_unpushed_count(branch),
                    stack_name.clone(),
                ));
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
                CliId::Branch(branch) => Some(branch.name.clone()),
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
        CliId::Branch(branch) => Ok(branch.name.clone()),
        _ => Err(anyhow::anyhow!(
            "Expected branch identifier, got {}. Please use a branch name or branch CLI ID.",
            cli_ids[0].kind_for_humans()
        )),
    }
}

fn get_available_branch_names(ctx: &Context) -> anyhow::Result<Vec<String>> {
    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;
    let mut branch_names = Vec::new();

    for stack in stacks {
        for branch in &stack.branches {
            branch_names.push(branch.name.clone());
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

fn write_review_sync_warning(
    out: &mut dyn std::fmt::Write,
    outcome: &but_forge::ReviewSyncOutcome,
) -> std::fmt::Result {
    if let but_forge::ReviewSyncOutcome::Failed { message } = outcome {
        let t = theme::get();
        writeln!(
            out,
            "{} Push succeeded, but review synchronization failed: {}",
            t.sym().warning,
            t.hint.paint(message)
        )?;
    }
    Ok(())
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

/// Check if a push of this branch would include any conflicted commits.
/// The push covers the branch and its stack ancestors, so those are checked
/// too. Returns an error if conflicted commits are found.
fn check_for_conflicted_commits(ctx: &Context, branch_name: &str) -> anyhow::Result<()> {
    let stacks = crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx)?;

    let repo = ctx.repo.get()?.clone().for_commit_shortening();
    // Find the stack containing this branch.
    for stack in &stacks {
        if stack.id.is_some()
            && let Some(position) = stack.branches.iter().position(|b| b.name == branch_name)
        {
            // Branches are ordered topmost-first; the push includes the
            // branch and everything below it.
            let conflicted: Vec<gix::ObjectId> = stack.branches[position..]
                .iter()
                .flat_map(|branch| &branch.commits)
                .filter(|c| c.has_conflicts)
                .map(|c| c.id)
                .collect();
            // Only pay for the map when the error actually prints.
            let id_map = (!conflicted.is_empty())
                .then(|| crate::IdMap::legacy_new_from_context(ctx).ok())
                .flatten();
            let conflicted_commits: Vec<String> = conflicted
                .iter()
                .map(|id| {
                    id_map
                        .as_ref()
                        .and_then(|id_map| id_map.change_id_ref(*id))
                        .map(|change_id| change_id.padded_short_id())
                        .unwrap_or_else(|| shorten_object_id(&repo, *id))
                })
                .collect();

            if !conflicted_commits.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot push branch '{}': the push would include {} conflicted commit{}.\n\
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
