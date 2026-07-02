mod json;

use std::{collections::HashMap, fmt::Write};

use bstr::ByteSlice;
use but_api::workspace::WorkspaceIntegrateUpstreamOutcome;
use but_api::workspace::json::{BottomUpdate, BottomUpdateKind};
use but_core::{DryRun, RepositoryExt};
use but_ctx::Context;
use but_workspace::{
    RefInfo,
    branch::Stack,
    ref_info::{LocalCommitRelation, Segment},
    ui::PushStatus,
};
use json::{BaseBranchInfo, BranchStatusInfo, PullCheckOutput, UpstreamCommit, UpstreamInfo};
use serde::{Deserialize, Serialize};

use crate::{
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
    let t = theme::get();
    let mut progress = out.progress_channel();

    writeln!(progress, "Fetching from upstream remotes...")?;

    let base_branch =
        but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("auto".to_string()))?;

    let should_check_integration = if base_branch.behind == 0 {
        let current_head_info = but_api::legacy::workspace::head_info(ctx)?;
        head_info_has_cleanup_candidate(&current_head_info)
    } else {
        true
    };
    let (has_worktree_conflicts, statuses) = if should_check_integration {
        let (_current_head_info, _updates, preview, statuses) = dry_run_upstream_integration(ctx)?;
        (!preview.worktree_conflicts.is_empty(), statuses)
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
                for status in statuses {
                    for bs in status.branch_statuses {
                        let status_text = match bs.status {
                            PullBranchStatus::Clear => t.success.paint("[ok]"),
                            PullBranchStatus::Integrated => t.info.paint("[integrated]"),
                            PullBranchStatus::Conflicted => {
                                t.attention.paint("[conflict - rebasable]")
                            }
                        };
                        writeln!(out, "  {} {}", status_text, bs.name)?;
                    }
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

async fn handle_pull(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
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
        head_info_has_cleanup_candidate(&current_head_info)
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
    let (current_head_info, updates, preview, statuses) = dry_run_upstream_integration(ctx)?;

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

    let resolutions = if !preview.worktree_conflicts.is_empty() {
        pull_result.status = "worktree_conflicts".to_string();
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "\n{}",
                t.error.paint("There are uncommitted changes in the worktree that may conflict with the updates.")
            )?;
            writeln!(
                out,
                "   {}",
                t.attention
                    .paint("Please commit or stash them and try again.")
            )?;
        }
        if let Some(out) = out.for_json() {
            out.write_value(&pull_result)?;
        }
        None
    } else {
        pull_result.status = "updating".to_string();

        let mut branches_to_update = 0;
        let mut integrated_branches = vec![];
        for status in &statuses {
            for branch_status in &status.branch_statuses {
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
                    PullBranchStatus::Clear => {
                        pull_result.summary.branches_updated += 1;
                    }
                }

                pull_result.branches_to_update.push(branch_info);
            }
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

        pull_result.integrated_branches = integrated_branches.clone();

        Some((updates, statuses))
    };

    // Step 3: Actually perform the integration
    if let Some((updates, statuses)) = resolutions {
        let integration_result = {
            let mut ctx = ctx.to_sync().into_thread_local();
            but_api::workspace::workspace_integrate_upstream(&mut ctx, updates, DryRun::No)
        };

        match integration_result {
            Ok(outcome) => {
                let post_statuses = derive_upstream_integration_statuses(
                    &current_head_info,
                    &outcome.workspace_state.head_info,
                );
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
                    || post_statuses.iter().any(|status| {
                        status
                            .branch_statuses
                            .iter()
                            .any(|bs| matches!(bs.status, PullBranchStatus::Conflicted))
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
                    }

                    // Conflict resolution instructions
                    if has_conflicts {
                        writeln!(out)?;
                        writeln!(out, "{}", t.important.paint("To resolve conflicts:"))?;
                        writeln!(
                            out,
                            "  1. Run {} to see conflicted commits",
                            t.command_suggestion.paint("`but status`")
                        )?;
                        writeln!(
                            out,
                            "  2. Run {} to enter resolution mode on any conflicted commit",
                            t.command_suggestion.paint("`but resolve <commit>`")
                        )?;
                        writeln!(out, "  3. Edit files to resolve the conflicts")?;
                        writeln!(
                            out,
                            "  4. Run {} to finalize the resolution",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PullBranchStatus {
    Clear,
    Integrated,
    Conflicted,
}

impl PullBranchStatus {
    fn as_str(self) -> &'static str {
        match self {
            PullBranchStatus::Clear => "updatable",
            PullBranchStatus::Integrated => "integrated",
            PullBranchStatus::Conflicted => "conflicted_rebasable",
        }
    }
}

#[derive(Debug, Clone)]
struct PullBranchStatusInfo {
    name: String,
    status: PullBranchStatus,
}

#[derive(Debug, Clone)]
struct PullStackStatusInfo {
    branch_statuses: Vec<PullBranchStatusInfo>,
}

/// Preview upstream integration through the workspace API without materializing it.
///
/// This keeps `but pull --check` and `but pull` on the same branch selectors and
/// status derivation path as the desktop app.
fn dry_run_upstream_integration(
    ctx: &Context,
) -> anyhow::Result<(
    RefInfo,
    Vec<BottomUpdate>,
    WorkspaceIntegrateUpstreamOutcome,
    Vec<PullStackStatusInfo>,
)> {
    let current_head_info = but_api::legacy::workspace::head_info(ctx)?;
    let updates = build_upstream_integration_updates(&current_head_info)?;
    let preview = {
        let mut ctx = ctx.to_sync().into_thread_local();
        but_api::workspace::workspace_integrate_upstream(&mut ctx, updates.clone(), DryRun::Yes)?
    };
    let statuses = derive_upstream_integration_statuses(
        &current_head_info,
        &preview.workspace_state.head_info,
    );
    Ok((current_head_info, updates, preview, statuses))
}

fn check_branch_statuses(statuses: &[PullStackStatusInfo]) -> Vec<BranchStatusInfo> {
    statuses
        .iter()
        .flat_map(|stack_status| {
            stack_status.branch_statuses.iter().map(|branch_status| {
                let (status, rebasable) = match branch_status.status {
                    PullBranchStatus::Clear => ("updatable", None),
                    PullBranchStatus::Integrated => ("integrated", None),
                    PullBranchStatus::Conflicted => ("conflicted", Some(true)),
                };
                BranchStatusInfo {
                    name: branch_status.name.clone(),
                    status: status.to_string(),
                    rebasable,
                }
            })
        })
        .collect()
}

fn collect_materialized_rebase_results(
    pre_integration_statuses: &[PullStackStatusInfo],
    post_integration_statuses: &[PullStackStatusInfo],
    successful_rebases: &mut Vec<String>,
    conflicted_rebases: &mut Vec<String>,
) {
    for stack_status in pre_integration_statuses {
        for branch_status in &stack_status.branch_statuses {
            if matches!(branch_status.status, PullBranchStatus::Integrated) {
                continue;
            }

            match post_branch_status(post_integration_statuses, branch_status.name.as_str()) {
                Some(PullBranchStatus::Conflicted) => {
                    conflicted_rebases.push(branch_status.name.clone());
                }
                Some(PullBranchStatus::Clear | PullBranchStatus::Integrated) | None => {
                    successful_rebases.push(branch_status.name.clone());
                }
            }
        }
    }
}

fn post_branch_status(
    post_integration_statuses: &[PullStackStatusInfo],
    branch_name: &str,
) -> Option<PullBranchStatus> {
    post_integration_statuses
        .iter()
        .flat_map(|stack_status| &stack_status.branch_statuses)
        .find(|branch_status| branch_status.name == branch_name)
        .map(|branch_status| branch_status.status)
}

fn statuses_need_update(statuses: &[PullStackStatusInfo]) -> bool {
    statuses.iter().any(|stack_status| {
        stack_status
            .branch_statuses
            .iter()
            .any(|branch_status| branch_status.status != PullBranchStatus::Clear)
    })
}

fn head_info_has_cleanup_candidate(head_info: &RefInfo) -> bool {
    head_info
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .any(|segment| {
            matches!(segment.push_status, PushStatus::Integrated)
                || segment
                    .commits
                    .iter()
                    .any(|commit| matches!(commit.relation, LocalCommitRelation::Integrated(_)))
                || (segment.commits.is_empty() && segment.remote_tracking_ref_name.is_some())
        })
}

/// Build the stack-bottom update requests that the upstream integration API expects.
///
/// The desktop client derives the same list from the current workspace preview before
/// calling `workspace_integrate_upstream`; keeping the CLI on the same selector shape
/// makes the dry run and materialization paths classify the same branches.
fn build_upstream_integration_updates(head_info: &RefInfo) -> anyhow::Result<Vec<BottomUpdate>> {
    let mut updates = Vec::new();
    for stack in &head_info.stacks {
        if let Some(update) = bottom_update_for_stack(stack)? {
            updates.push(update);
        }
    }
    Ok(updates)
}

/// Select the bottom-most commit, or the empty bottom branch reference, for a stack.
///
/// Upstream integration rebases from the stack bottom. Empty branches have no commit to
/// select, so they are represented by their branch reference instead.
fn bottom_update_for_stack(stack: &Stack) -> anyhow::Result<Option<BottomUpdate>> {
    let Some(segment) = stack.segments.last() else {
        return Ok(None);
    };
    let selector = if let Some(commit) = segment.commits.last() {
        but_api::commit::json::RelativeTo::Commit(commit.id)
    } else {
        let Some(ref_info) = segment.ref_info.as_ref() else {
            return Ok(None);
        };
        but_api::commit::json::RelativeTo::ReferenceBytes(ref_info.ref_name.clone())
    };

    Ok(Some(BottomUpdate {
        kind: BottomUpdateKind::Rebase,
        selector,
    }))
}

/// Compare the current workspace to the dry-run or post-integration workspace.
///
/// Missing preview segments are treated as integrated, while surviving segments with
/// conflicted commits are reported as conflicted. This mirrors the desktop status
/// derivation without depending on the UI projection types.
fn derive_upstream_integration_statuses(
    current: &RefInfo,
    preview: &RefInfo,
) -> Vec<PullStackStatusInfo> {
    let preview_segments = preview_segments_by_ref_name(preview);

    current
        .stacks
        .iter()
        .map(|stack| {
            let branch_statuses = stack
                .segments
                .iter()
                .map(|segment| derive_branch_status(segment, &preview_segments))
                .collect::<Vec<_>>();

            PullStackStatusInfo { branch_statuses }
        })
        .collect()
}

/// Index preview segments by their full branch ref so current segments can be matched.
fn preview_segments_by_ref_name(preview: &RefInfo) -> HashMap<Vec<u8>, &Segment> {
    let mut segments = HashMap::new();
    for stack in &preview.stacks {
        for segment in &stack.segments {
            if let Some(ref_info) = &segment.ref_info {
                segments.insert(ref_info.ref_name.as_bstr().to_vec(), segment);
            }
        }
    }
    segments
}

/// Derive the pull status for a single current segment from its preview counterpart.
fn derive_branch_status(
    segment: &Segment,
    preview_segments: &HashMap<Vec<u8>, &Segment>,
) -> PullBranchStatusInfo {
    let name = branch_display_name(segment);
    let Some(ref_info) = &segment.ref_info else {
        return PullBranchStatusInfo {
            name,
            status: PullBranchStatus::Clear,
        };
    };

    let Some(preview_segment) = preview_segments.get(ref_info.ref_name.as_bstr().as_bytes()) else {
        return PullBranchStatusInfo {
            name,
            status: PullBranchStatus::Integrated,
        };
    };

    let has_conflicts = preview_segment
        .commits
        .iter()
        .any(|commit| commit.has_conflicts);

    PullBranchStatusInfo {
        name,
        status: if has_conflicts {
            PullBranchStatus::Conflicted
        } else {
            PullBranchStatus::Clear
        },
    }
}

/// Return a human-readable branch name for CLI output.
fn branch_display_name(segment: &Segment) -> String {
    segment
        .ref_info
        .as_ref()
        .map(|ref_info| ref_info.ref_name.shorten().to_string())
        .unwrap_or_else(|| "Unnamed segment".to_string())
}
