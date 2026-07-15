mod json;

use std::fmt::Write;

use anyhow::Context as _;
use bstr::ByteSlice;
use but_core::{DryRun, RepositoryExt};
use but_ctx::Context;
use gitbutler_oplog::{
    LocalTargetSnapshot, OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gix::refs::transaction::PreviousValue;
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
    local_target: Option<LocalTargetUpdateInfo>,
    recent_commits: Vec<CommitInfo>,
    branches_to_update: Vec<BranchUpdateInfo>,
    integrated_branches: Vec<String>,
    conflicts: Vec<ConflictInfo>,
    summary: PullSummary,
    undo_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalTargetUpdateInfo {
    status: String,
    branch: Option<String>,
    previous_sha: Option<String>,
    current_sha: String,
}

struct LocalTargetUpdatePlan {
    info: LocalTargetUpdateInfo,
    edit: Option<LocalTargetRefEdit>,
}

struct LocalTargetRefEdit {
    ref_name: gix::refs::FullName,
    tracking_ref: gix::refs::FullName,
    previous_tip: gix::ObjectId,
    current_tip: gix::ObjectId,
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

async fn handle_pull(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    let t = theme::get();
    let mut pull_result = PullResult {
        status: String::new(),
        upstream_url: None,
        upstream_commits_found: 0,
        local_target: None,
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
        pull_result.local_target = Some(update_local_target_with_snapshot(
            ctx,
            base_branch.current_sha,
        )?);
        write_local_target_update(out, pull_result.local_target.as_ref())?;
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
        pull_result.local_target = Some(update_local_target_with_snapshot(
            ctx,
            base_branch.current_sha,
        )?);
        write_local_target_update(out, pull_result.local_target.as_ref())?;
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
                t.attention
                    .paint("Please commit or stash them and try again.")
            )?;
        }
        None
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

    if statuses_to_apply.is_none() {
        pull_result.local_target = Some(update_local_target_with_snapshot(
            ctx,
            base_branch.current_sha,
        )?);
        write_local_target_update(out, pull_result.local_target.as_ref())?;
        if let Some(out) = out.for_json() {
            out.write_value(&pull_result)?;
        }
        return Ok(());
    }

    // Step 3: Actually perform the integration
    if let Some(statuses) = statuses_to_apply {
        let updates = but_api::workspace::rebase_stack_bottoms(&current_head_info);
        let integration_result =
            integrate_upstream_and_update_local_target(ctx, updates, base_branch.current_sha);

        match integration_result {
            Ok((outcome, local_target)) => {
                pull_result.local_target = Some(local_target);
                write_local_target_update(out, pull_result.local_target.as_ref())?;
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

fn plan_local_tracking_target_update(
    ctx: &Context,
    repo: &gix::Repository,
    target_tip: gix::ObjectId,
) -> anyhow::Result<LocalTargetUpdatePlan> {
    let target_ref_name = ctx.project_meta()?.target_ref_or_err()?.clone();

    let mut local_ref: Option<gix::Reference<'_>> = None;
    for candidate in repo.references()?.local_branches()? {
        let candidate = candidate.map_err(anyhow::Error::from_boxed)?;
        let tracks_target = match repo
            .branch_remote_tracking_ref_name(candidate.name(), gix::remote::Direction::Fetch)
            .transpose()
        {
            Ok(Some(tracking_ref)) => tracking_ref.as_ref() == target_ref_name.as_ref(),
            Ok(None) | Err(_) => false,
        };
        if !tracks_target {
            continue;
        }
        if let Some(existing) = &local_ref {
            anyhow::bail!(
                "Cannot fast-forward a local target: both '{}' and '{}' are configured to track '{}'",
                existing.name().shorten(),
                candidate.name().shorten(),
                target_ref_name.shorten()
            );
        }
        local_ref = Some(candidate);
    }
    let Some(mut local_ref) = local_ref else {
        return Ok(LocalTargetUpdatePlan {
            info: LocalTargetUpdateInfo {
                status: "not_configured".to_owned(),
                branch: None,
                previous_sha: None,
                current_sha: target_tip.to_string(),
            },
            edit: None,
        });
    };
    let local_ref_name = local_ref.name().to_owned();
    let local_tip = local_ref.peel_to_id()?.detach();
    if local_tip == target_tip {
        return Ok(LocalTargetUpdatePlan {
            info: LocalTargetUpdateInfo {
                status: "already_current".to_owned(),
                branch: Some(local_ref_name.shorten().to_string()),
                previous_sha: Some(local_tip.to_string()),
                current_sha: target_tip.to_string(),
            },
            edit: None,
        });
    }

    let merge_base = match repo.merge_base(local_tip, target_tip) {
        Ok(id) => Some(id.detach()),
        Err(gix::repository::merge_base::Error::FindMergeBase(_))
        | Err(gix::repository::merge_base::Error::NotFound { .. }) => None,
        Err(err) => return Err(err.into()),
    };
    if merge_base != Some(local_tip) {
        anyhow::bail!(
            "Cannot fast-forward local target '{}' from {local_tip} to {target_tip}: it has diverged from '{}'",
            local_ref_name.shorten(),
            target_ref_name.shorten()
        );
    }

    let checkout_probe = but_core::branch::SafeDelete::new(&repo)?;
    if let Some(paths) = checkout_probe.worktree_dirs_with_ref(&local_ref) {
        anyhow::bail!(
            "Cannot fast-forward local target '{}' because it is checked out in: {paths:?}",
            local_ref_name.shorten()
        );
    }

    Ok(LocalTargetUpdatePlan {
        info: LocalTargetUpdateInfo {
            status: "fast_forwarded".to_owned(),
            branch: Some(local_ref_name.shorten().to_string()),
            previous_sha: Some(local_tip.to_string()),
            current_sha: target_tip.to_string(),
        },
        edit: Some(LocalTargetRefEdit {
            ref_name: local_ref_name,
            tracking_ref: target_ref_name,
            previous_tip: local_tip,
            current_tip: target_tip,
        }),
    })
}

impl LocalTargetRefEdit {
    fn snapshot(&self) -> LocalTargetSnapshot {
        LocalTargetSnapshot {
            ref_name: self.ref_name.clone(),
            tracking_ref: self.tracking_ref.clone(),
            snapshot_tip: self.previous_tip,
            expected_tip: self.current_tip,
        }
    }

    fn apply(&self, repo: &gix::Repository) -> anyhow::Result<()> {
        repo.reference(
            self.ref_name.as_ref(),
            self.current_tip,
            PreviousValue::ExistingMustMatch(self.previous_tip.into()),
            "GitButler pull",
        )?;
        Ok(())
    }
}

fn update_local_target_with_snapshot(
    ctx: &Context,
    target_tip: gix::ObjectId,
) -> anyhow::Result<LocalTargetUpdateInfo> {
    let mut thread_ctx = ctx.to_sync().into_thread_local();
    let mut guard = thread_ctx.exclusive_worktree_access();
    let plan = {
        let repo = thread_ctx.repo.get()?;
        plan_local_tracking_target_update(&thread_ctx, &repo, target_tip)?
    };
    let Some(edit) = &plan.edit else {
        return Ok(plan.info.clone());
    };
    let snapshot_tree =
        thread_ctx.prepare_snapshot_with_local_target(&edit.snapshot(), guard.read_permission())?;
    let snapshot = thread_ctx.prepare_snapshot_commit(
        snapshot_tree,
        SnapshotDetails::new(OperationKind::MergeUpstream),
        guard.read_permission(),
    )?;
    let snapshot_id = snapshot.commit_id();
    fail_pull_before_local_target_effect()?;
    {
        let repo = thread_ctx.repo.get()?;
        edit.apply(&repo)?;
    }
    if let Err(publication_err) = thread_ctx.publish_snapshot(snapshot, guard.write_permission()) {
        thread_ctx
            .restore_snapshot_state(snapshot_id, guard.write_permission())
            .with_context(|| {
                format!(
                    "failed to roll back the pull after snapshot publication failed: {publication_err:#}"
                )
            })?;
        return Err(publication_err);
    }
    Ok(plan.info.clone())
}

fn integrate_upstream_and_update_local_target(
    ctx: &Context,
    updates: Vec<but_workspace::BottomUpdate>,
    target_tip: gix::ObjectId,
) -> anyhow::Result<(
    but_api::workspace::WorkspaceIntegrateUpstreamOutcome,
    LocalTargetUpdateInfo,
)> {
    let mut thread_ctx = ctx.to_sync().into_thread_local();
    let mut guard = thread_ctx.exclusive_worktree_access();
    let plan = {
        let repo = thread_ctx.repo.get()?;
        plan_local_tracking_target_update(&thread_ctx, &repo, target_tip)?
    };
    let Some(edit) = &plan.edit else {
        let outcome = but_api::workspace::workspace_integrate_upstream_with_perm(
            &mut thread_ctx,
            updates,
            DryRun::No,
            guard.write_permission(),
        )?;
        return Ok((outcome, plan.info));
    };

    let snapshot_tree =
        thread_ctx.prepare_snapshot_with_local_target(&edit.snapshot(), guard.read_permission())?;
    let snapshot = thread_ctx.prepare_snapshot_commit(
        snapshot_tree,
        SnapshotDetails::new(OperationKind::MergeUpstream),
        guard.read_permission(),
    )?;
    let snapshot_id = snapshot.commit_id();
    fail_pull_before_local_target_effect()?;
    {
        let repo = thread_ctx.repo.get()?;
        edit.apply(&repo)?;
    }
    let integration_result = but_api::workspace::workspace_integrate_upstream_only_with_perm(
        &mut thread_ctx,
        updates,
        DryRun::No,
        guard.write_permission(),
    );
    let outcome = match integration_result {
        Ok(outcome) => outcome,
        Err(integration_err) => {
            thread_ctx
                .restore_snapshot_state(snapshot_id, guard.write_permission())
                .with_context(|| {
                    format!("failed to roll back pull integration: {integration_err:#}")
                })?;
            return Err(integration_err);
        }
    };
    if let Err(publication_err) = thread_ctx.publish_snapshot(snapshot, guard.write_permission()) {
        thread_ctx
            .restore_snapshot_state(snapshot_id, guard.write_permission())
            .with_context(|| {
                format!(
                    "failed to roll back pull integration after snapshot publication failed: {publication_err:#}"
                )
            })?;
        return Err(publication_err);
    }
    Ok((outcome, plan.info))
}

fn fail_pull_before_local_target_effect() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    if std::env::var_os("GITBUTLER_TEST_PULL_FAILURE").as_deref()
        == Some(std::ffi::OsStr::new("before-local-target"))
    {
        anyhow::bail!("injected pull failure before applying the local target");
    }
    Ok(())
}

fn write_local_target_update(
    out: &mut OutputChannel,
    update: Option<&LocalTargetUpdateInfo>,
) -> anyhow::Result<()> {
    let Some(update) = update.filter(|update| update.status == "fast_forwarded") else {
        return Ok(());
    };
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "   Fast-forwarded local target {} to {}",
            update.branch.as_deref().unwrap_or("<unknown>"),
            update.current_sha
        )?;
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
