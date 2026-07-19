mod json;

use std::fmt::Write;

use anyhow::bail;
use bstr::ByteSlice;
use but_core::{DryRun, RepositoryExt};
use but_ctx::Context;
use json::{BaseBranchInfo, BranchStatusInfo, PullCheckOutput, UpstreamCommit, UpstreamInfo};
use serde::{Deserialize, Serialize};

use crate::{
    command::legacy::upstream::{
        self, BranchStatus as PullBranchStatus, BranchStatusInfo as PullBranchStatusInfo,
    },
    theme::{self, Paint},
    utils::{OutputChannel, shorten_hex_object_id},
};

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
    #[serde(default)]
    commits: Vec<ConflictedCommitInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConflictedCommitInfo {
    id: String,
    message: String,
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
    ctx: &mut Context,
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
    let t = theme::get();
    let mut progress = out.progress_channel();

    writeln!(progress, "Fetching from upstream remotes...")?;

    let base_branch =
        but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("auto".to_string()))?;

    let should_check_integration = if base_branch.behind == 0 {
        let current_head_info = but_api::legacy::workspace::head_info(ctx)?;
        upstream::has_cleanup_candidate(&current_head_info)
    } else {
        true
    };
    let (has_worktree_conflicts, statuses) = if should_check_integration {
        let preview = upstream::dry_run_integration(ctx)?;
        (
            !preview.outcome.worktree_conflicts.is_empty(),
            preview.statuses,
        )
    } else {
        (false, Vec::new())
    };
    let up_to_date = base_branch.behind == 0 && !statuses_need_update(&statuses);
    if !up_to_date {
        writeln!(progress, "Checking integration statuses...")?;
    }

    if let Some(out) = out.for_json() {
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
            branch_statuses: check_branch_statuses(&statuses),
            up_to_date,
            has_worktree_conflicts,
        };
        out.write_value(output)?;
    } else if let Some(out) = out.for_human() {
        writeln!(
            progress,
            "{}",
            t.important.paint("Checking base branch status...")
        )?;
        writeln!(
            out,
            "\n{}\t{}",
            t.hint.paint("Base branch:"),
            t.remote_branch.paint(&base_branch.branch_name)
        )?;
        let upstream_label = format!(
            "{} new commits on {}",
            base_branch.behind, base_branch.branch_name
        );
        writeln!(
            out,
            "{}\t{}",
            t.hint.paint("Upstream:"),
            if base_branch.behind > 0 {
                t.attention.paint(&upstream_label)
            } else {
                t.success.paint(&upstream_label)
            }
        )?;

        if !base_branch.upstream_commits.is_empty() {
            let repo = ctx.repo.get()?.clone().for_commit_shortening();
            writeln!(out)?;
            let commits = base_branch.upstream_commits.iter().take(3);
            for commit in commits {
                let commit_short = shorten_hex_object_id(&repo, &commit.id);
                let msg: String = commit
                    .description
                    .to_string()
                    .replace('\n', " ")
                    .chars()
                    .take(72)
                    .collect();
                writeln!(
                    out,
                    "  {} {}",
                    t.commit_id.paint(&commit_short),
                    t.hint.paint(&msg)
                )?;
            }
            let hidden_commits = base_branch.behind.saturating_sub(3);
            if hidden_commits > 0 {
                writeln!(
                    out,
                    "  {}",
                    t.hint.paint(format!("... ({hidden_commits} more)"))
                )?;
            }
        }

        if up_to_date {
            writeln!(out, "\n{}", t.success.paint("Up to date"))?;
        } else {
            if has_worktree_conflicts {
                writeln!(
                    out,
                    "\n{}",
                    t.attention
                        .paint("Warning: uncommitted changes may conflict with updates.")
                )?;
            }
            if !statuses.is_empty() {
                writeln!(out, "\n{}", t.important.paint("Branch Status"))?;
                for branch_status in statuses {
                    let status_text = match branch_status.status {
                        PullBranchStatus::Clear | PullBranchStatus::Empty => {
                            t.success.paint("[ok]")
                        }
                        PullBranchStatus::Integrated => t.info.paint("[integrated]"),
                        PullBranchStatus::Conflicted => t.attention.paint("[conflict - rebasable]"),
                    };
                    writeln!(out, "  {} {}", status_text, branch_status.name)?;
                }
            }
            writeln!(
                out,
                "\n{}",
                t.hint.paint("Run `but pull` to update your branches")
            )?;
        }
    }
    Ok(())
}

async fn handle_pull(ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    let t = theme::get();
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
    writeln!(
        progress,
        "{}",
        t.progress.paint("Fetching newest data from remotes...")
    )?;

    // Fetch from remotes to get latest upstream info
    let base_branch =
        but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("pull".to_string()))?;

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
        writeln!(progress, "   Checking: {}", t.link.paint(&upstream_url))?;

        if base_branch.behind > 0 {
            writeln!(
                out,
                "\n{} {} upstream commits on {}",
                t.important.paint("Found"),
                t.attention.paint(base_branch.behind.to_string()),
                t.remote_branch.paint(&base_branch.branch_name)
            )?;

            // Show upstream commits (actual new commits to integrate)
            let repo = ctx.repo.get()?.clone().for_commit_shortening();
            for commit_info in &pull_result.recent_commits {
                let msg = commit_info
                    .message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(65)
                    .collect::<String>();
                let commit_short = shorten_hex_object_id(&repo, &commit_info.id);
                writeln!(out, "   {} {}", t.hint.paint(&commit_short), msg)?;
            }

            let hidden = base_branch.behind.saturating_sub(commits_to_show);
            if hidden > 0 {
                writeln!(out, "   ... and {} more", t.hint.paint(hidden.to_string()))?;
            }
        } else {
            writeln!(
                out,
                "\n{}",
                t.success.paint("No new upstream commits found")
            )?;
        }

        if base_branch.behind > 0 {
            writeln!(progress, "   Checking integration statuses...")?;
        }
    }

    let should_check_integration = if base_branch.behind == 0 {
        let current_head_info = but_api::legacy::workspace::head_info(ctx)?;
        upstream::has_cleanup_candidate(&current_head_info)
    } else {
        true
    };
    if !should_check_integration {
        pull_result.status = "up_to_date".to_string();
        if let Some(out) = out.for_human() {
            writeln!(out, "\n{}", t.success.paint("Everything is up to date"))?;
        }
        if let Some(out) = out.for_json() {
            out.write_value(&pull_result)?;
        }
        return Ok(());
    }

    // Step 2: Dry-run integration and derive statuses from the preview, like the desktop app.
    let upstream::IntegrationPreview {
        current: current_head_info,
        outcome: preview,
        statuses,
    } = upstream::dry_run_integration(ctx)?;

    if base_branch.behind == 0 && !statuses_need_update(&statuses) {
        pull_result.status = "up_to_date".to_string();
        if let Some(out) = out.for_human() {
            writeln!(out, "\n{}", t.success.paint("Everything is up to date"))?;
        }
        if let Some(out) = out.for_json() {
            out.write_value(&pull_result)?;
        }
        return Ok(());
    }

    let statuses_to_apply = if !preview.worktree_conflicts.is_empty() {
        pull_result.status = "worktree_conflicts".to_string();
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "\n{}",
                t.error.paint(
                    "There are uncommitted changes in the worktree that conflict with the updates:"
                )
            )?;
            for path in &preview.worktree_conflicts {
                writeln!(out, "  {}", t.attention.paint(path.to_str_lossy()))?;
            }
            writeln!(
                out,
                "{}",
                t.important
                    .paint("To update anyway, park them on a temporary commit first:")
            )?;
            writeln!(
                out,
                "  1. Run {} with the files listed above ({} shows their IDs)",
                t.command_suggestion
                    .paint("`but commit <branch> --changes <file-id...> -m \"wip\"`"),
                t.command_suggestion.paint("`but diff`")
            )?;
            writeln!(
                out,
                "  2. Run {} again; the parked commit may come back conflicted, ready for {}",
                t.command_suggestion.paint("`but pull`"),
                t.command_suggestion.paint("`but resolve`")
            )?;
            writeln!(
                out,
                "  3. Run {} afterwards to make those changes uncommitted again",
                t.command_suggestion.paint("`but uncommit <commit>`")
            )?;
        }
        if let Some(out) = out.for_json() {
            out.write_value(&pull_result)?;
        }
        bail!("nothing was updated; uncommitted changes conflict with the incoming updates");
    } else {
        pull_result.status = "updating".to_string();

        let mut branches_to_update = 0;
        let mut integrated_branches = vec![];
        for branch_status in &statuses {
            branches_to_update += 1;

            let branch_info = BranchUpdateInfo {
                name: branch_status.name.clone(),
                status: branch_status.status.as_str().to_string(),
                commit_count: 0, // TODO: Get actual commit count
                conflicts: vec![],
            };

            match branch_status.status {
                PullBranchStatus::Integrated => {
                    integrated_branches.push(branch_status.name.clone());
                    pull_result.summary.branches_integrated += 1;
                }
                PullBranchStatus::Conflicted => {
                    pull_result.summary.branches_conflicted += 1;
                }
                PullBranchStatus::Clear | PullBranchStatus::Empty => {
                    pull_result.summary.branches_updated += 1;
                }
            }

            pull_result.branches_to_update.push(branch_info);
        }

        if let Some(out) = out.for_human()
            && branches_to_update > 0
        {
            writeln!(
                out,
                "\n{} {} active branches...",
                t.progress.paint("Updating"),
                t.attention.paint(branches_to_update.to_string())
            )?;
        }

        pull_result.integrated_branches = integrated_branches;

        Some(statuses)
    };

    // Step 3: Actually perform the integration
    if let Some(statuses) = statuses_to_apply {
        let integration_result = {
            let updates = but_api::workspace::rebase_stack_bottoms(&current_head_info);
            let mut ctx = ctx.to_sync().into_thread_local();
            let mut guard = ctx.exclusive_worktree_access();
            but_api::workspace::workspace_integrate_upstream_with_perm(
                &mut ctx,
                updates,
                DryRun::No,
                guard.write_permission(),
            )
        };

        match integration_result {
            Ok(outcome) => {
                let post_statuses =
                    upstream::classify(&current_head_info, &outcome.workspace_state);
                // Report detailed results for each resolution
                let mut successful_rebases: Vec<String> = Vec::new();
                let mut conflicted_rebases: Vec<String> = Vec::new();
                collect_materialized_rebase_results(
                    &statuses,
                    &post_statuses,
                    &mut successful_rebases,
                    &mut conflicted_rebases,
                );

                // Check if there are any conflicted files
                let has_conflicts = !conflicted_rebases.is_empty()
                    || post_statuses
                        .iter()
                        .any(|status| matches!(status.status, PullBranchStatus::Conflicted));

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

                // The integration ran on its own thread-local context, so this
                // context's cached repository and workspace still predate the
                // rebase. Reload before rendering, or the conflicted commits
                // fall back to sha refs that don't match what status shows.
                if has_conflicts {
                    let mut guard = ctx.exclusive_worktree_access();
                    if let Err(err) =
                        ctx.reload_repo_and_invalidate_workspace(guard.write_permission())
                    {
                        tracing::warn!(?err, "could not reload the workspace after integration");
                    }
                }

                // Look up the conflicted commits so the output can name them
                // directly instead of sending callers through `but status`.
                let conflicted_commits = if has_conflicts {
                    crate::command::legacy::resolve::find_conflicted_commits(ctx).unwrap_or_else(
                        |err| {
                            tracing::warn!(?err, "could not look up conflicted commits");
                            Default::default()
                        },
                    )
                } else {
                    Default::default()
                };

                // Populate conflicts info
                for branch_name in &conflicted_rebases {
                    pull_result.conflicts.push(ConflictInfo {
                        branch: branch_name.clone(),
                        files: vec![], // TODO: Get actual conflicted files
                        upstream_commit: None,
                        commits: conflicted_commits
                            .get(branch_name)
                            .map(|commits| {
                                commits
                                    .iter()
                                    .map(|commit| ConflictedCommitInfo {
                                        id: commit.commit_short_id.clone(),
                                        message: commit.commit_message.clone(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    });
                }

                // Show results for each branch
                if let Some(out) = out.for_human() {
                    writeln!(out)?;

                    if has_conflicts {
                        writeln!(
                            out,
                            "{}",
                            t.attention.paint("Rebase resulted in some conflicts")
                        )?;
                    } else {
                        writeln!(out, "{}", t.success.paint("Rebase successful"))?;
                    }

                    // Report on integrated branches
                    if !pull_result.integrated_branches.is_empty() {
                        writeln!(out)?;
                        for branch in &pull_result.integrated_branches {
                            writeln!(
                                out,
                                "{} {} has been integrated upstream and removed locally",
                                t.important.paint("Branch"),
                                t.local_branch.paint(branch)
                            )?;
                        }
                    }

                    // Final summary
                    writeln!(out, "\n{}", t.important.paint("Summary"))?;
                    writeln!(out, "────────")?;

                    // List each branch with color-coded status
                    for branch in &successful_rebases {
                        writeln!(
                            out,
                            "  {} - {}",
                            t.local_branch.paint(branch),
                            t.success.paint("rebased")
                        )?;
                    }

                    for branch in &pull_result.integrated_branches {
                        writeln!(
                            out,
                            "  {} - {}",
                            t.local_branch.paint(branch),
                            t.info.paint("integrated")
                        )?;
                    }

                    for branch in &conflicted_rebases {
                        writeln!(
                            out,
                            "  {} - {}",
                            t.local_branch.paint(branch),
                            t.error.paint("conflicted")
                        )?;
                        for commit in conflicted_commits.get(branch).into_iter().flatten() {
                            writeln!(
                                out,
                                "      {} {}",
                                t.change_id.paint(&commit.commit_short_id),
                                t.hint.paint(&commit.commit_message)
                            )?;
                        }
                    }

                    // Conflict resolution instructions
                    if has_conflicts {
                        writeln!(out)?;
                        writeln!(out, "{}", t.important.paint("To resolve conflicts:"))?;
                        writeln!(
                            out,
                            "  1. Run {} on a conflicted commit listed above — oldest first (they are listed bottom-up). Worktree files show no conflict markers until this checks the commit out",
                            t.command_suggestion.paint("`but resolve <commit>`")
                        )?;
                        writeln!(out, "  2. Edit files to resolve the conflicts")?;
                        writeln!(
                            out,
                            "  3. Run {} to finalize the resolution",
                            t.command_suggestion.paint("`but resolve finish`")
                        )?;
                    }

                    // Undo instructions
                    writeln!(out)?;
                    writeln!(out, "{}", t.important.paint("To undo this operation:"))?;
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
                    writeln!(out, "\n{}", t.error.paint("Failed to update branches"))?;
                    writeln!(out, "   {e}")?;
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

fn check_branch_statuses(statuses: &[PullBranchStatusInfo]) -> Vec<BranchStatusInfo> {
    statuses
        .iter()
        .map(|branch_status| {
            let (status, rebasable) = match branch_status.status {
                PullBranchStatus::Clear | PullBranchStatus::Empty => ("updatable", None),
                PullBranchStatus::Integrated => ("integrated", None),
                PullBranchStatus::Conflicted => ("conflicted", Some(true)),
            };
            BranchStatusInfo {
                name: branch_status.name.clone(),
                status: status.to_string(),
                rebasable,
            }
        })
        .collect()
}

fn collect_materialized_rebase_results(
    pre_integration_statuses: &[PullBranchStatusInfo],
    post_integration_statuses: &[PullBranchStatusInfo],
    successful_rebases: &mut Vec<String>,
    conflicted_rebases: &mut Vec<String>,
) {
    for branch_status in pre_integration_statuses {
        if matches!(branch_status.status, PullBranchStatus::Integrated) {
            continue;
        }

        match post_branch_status(post_integration_statuses, branch_status.name.as_str()) {
            Some(PullBranchStatus::Conflicted) => {
                conflicted_rebases.push(branch_status.name.clone());
            }
            Some(
                PullBranchStatus::Clear | PullBranchStatus::Integrated | PullBranchStatus::Empty,
            )
            | None => {
                successful_rebases.push(branch_status.name.clone());
            }
        }
    }
}

fn post_branch_status(
    post_integration_statuses: &[PullBranchStatusInfo],
    branch_name: &str,
) -> Option<PullBranchStatus> {
    post_integration_statuses
        .iter()
        .find(|branch_status| branch_status.name == branch_name)
        .map(|branch_status| branch_status.status)
}

fn statuses_need_update(statuses: &[PullBranchStatusInfo]) -> bool {
    statuses
        .iter()
        .any(|branch_status| branch_status.status.needs_update())
}
