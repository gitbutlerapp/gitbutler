use std::collections::{BTreeMap, HashSet};

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api::legacy::modes::{
    abort_edit_and_return_to_workspace, edit_initial_index_state, enter_edit_mode,
    save_edit_and_return_to_workspace_with_output,
};
use but_ctx::Context;
use colored::Colorize;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_operating_modes::OperatingMode;
use std::fmt::Write;

use crate::{IdMap, args::resolve::Subcommands, id::CliId, utils::OutputChannel};

pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    cmd: Option<Subcommands>,
    commit_id: Option<String>,
) -> Result<()> {
    match cmd {
        Some(Subcommands::Status) => show_status(ctx, out),
        Some(Subcommands::Finish) => finish_resolution(ctx, out),
        Some(Subcommands::Cancel) => cancel_resolution(ctx, out),
        None => {
            // Default action: enter resolution mode for the specified commit
            if let Some(commit_id_str) = commit_id {
                enter_resolution(ctx, out, &commit_id_str)
            } else {
                // Check if we're already in edit mode
                let mode = gitbutler_operating_modes::operating_mode(ctx);
                if matches!(mode, OperatingMode::Edit(_)) {
                    // If in edit mode, show status instead of help
                    show_status(ctx, out)
                } else {
                    // Not in edit mode and no commit specified - check for conflicted commits
                    check_and_prompt_for_conflicts(ctx, out)
                }
            }
        }
    }
}

fn enter_resolution(ctx: &mut Context, out: &mut OutputChannel, commit_id_str: &str) -> Result<()> {
    // Create an IdMap to resolve commit IDs (supports both CLI IDs and partial SHAs)
    let id_map = IdMap::new_from_context(ctx, None)?;

    // Resolve the commit ID using the IdMap
    let matches = id_map.resolve_entity_to_ids(commit_id_str)?;

    if matches.is_empty() {
        bail!(
            "Commit '{}' not found. Try running 'but status' to see available commits.",
            commit_id_str
        );
    }

    if matches.len() > 1 {
        bail!(
            "Commit ID '{}' is ambiguous. Please provide more characters to uniquely identify the commit.",
            commit_id_str
        );
    }

    // Extract the commit OID from the matched CliId
    let commit_oid = match &matches[0] {
        CliId::Commit { commit_id, .. } => git2::Oid::from_bytes(commit_id.as_slice())?,
        _ => bail!("'{}' does not refer to a commit", commit_id_str),
    };

    // Get the commit and check if it's conflicted
    let git2_repo = ctx.git2_repo.get()?;
    let commit = git2_repo
        .find_commit(commit_oid)
        .context("Failed to find commit")?;

    if !commit.is_conflicted() {
        bail!(
            "Commit {} is not in a conflicted state. Only conflicted commits can be resolved.",
            &commit_oid.to_string()[..7]
        );
    }

    // Find which stack this commit belongs to
    let stacks = but_api::legacy::workspace::stacks(ctx.legacy_project.id, None)?;
    let mut found_stack_id = None;
    for stack in &stacks {
        // Check if this commit is in any of the stack's heads
        for head in &stack.heads {
            let head_oid = git2::Oid::from_bytes(head.tip.as_slice())?;
            // Walk the commit history to see if our commit is in this stack
            let mut revwalk = git2_repo.revwalk()?;
            revwalk.push(head_oid)?;
            revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

            for oid in revwalk {
                if oid? == commit_oid {
                    found_stack_id = stack.id;
                    break;
                }
            }
            if found_stack_id.is_some() {
                break;
            }
        }
        if found_stack_id.is_some() {
            break;
        }
    }

    let stack_id = found_stack_id.ok_or_else(|| {
        anyhow::anyhow!(
            "Could not find stack containing commit {}",
            &commit_oid.to_string()[..7]
        )
    })?;

    // Enter edit mode
    enter_edit_mode(ctx.legacy_project.id, commit_oid.to_string(), stack_id)
        .context("Failed to enter edit mode")?;

    // Drop the git2 objects to release the borrow
    drop(commit);
    drop(git2_repo);

    // Show checkout message
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} {}",
            "Checking out conflicted commit".bold(),
            commit_oid.to_string()[..7].cyan()
        )?;
    }

    // Now show the same status as `but resolve status` would show
    show_status(ctx, out)
}

fn show_status(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    show_status_impl(ctx, out, true)
}

/// Public function to show resolve status without prompting (for use by `but status`)
pub(crate) fn show_resolve_status(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    show_status_impl(ctx, out, false)
}

fn show_status_impl(
    ctx: &mut Context,
    out: &mut OutputChannel,
    prompt_to_finalize: bool,
) -> Result<()> {
    // Check if we're in edit mode
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if !matches!(mode, OperatingMode::Edit(_)) {
        // Not in edit mode, show the workflow help instead
        return show_workflow_help(out);
    }

    let mut progress = out.progress_channel();

    if out.for_human().is_some() {
        writeln!(
            progress,
            "{}\n - resolve all conflicts \n - finalize with {} \n - OR cancel with {}\n",
            "You are currently in conflict resolution mode.".bold(),
            "but resolve finish".green().bold(),
            "but resolve cancel".red().bold()
        )?;
    }

    let all_resolved = show_conflicted_files(ctx, out)?;

    // If all conflicts are resolved and we're in human mode, offer to finalize
    if all_resolved && out.for_human().is_some() && prompt_to_finalize {
        // Use a separate scope for the prompt
        writeln!(progress)?;
        writeln!(
            progress,
            "{}",
            "All conflicts have been resolved!".green().bold()
        )?;

        // Prepare for terminal input - this flushes the output and allows us to prompt
        let should_finalize = if let Some(mut _io) = out.prepare_for_terminal_input() {
            write!(progress, "Finalize the resolution now? [Y/n]: ")?;

            let mut response = String::new();
            std::io::stdin().read_line(&mut response)?;
            let response = response.trim().to_lowercase();

            if response.is_empty() || response == "y" || response == "yes" {
                writeln!(progress)?;
                true
            } else {
                writeln!(
                    progress,
                    "Resolution not finalized. Run {} when ready.",
                    "but resolve finish".green().bold()
                )?;
                false
            }
        } else {
            false
        };

        // Drop io before calling finish_resolution
        if should_finalize {
            return finish_resolution(ctx, out);
        }
    }

    Ok(())
}

fn show_conflicted_files(ctx: &mut Context, out: &mut OutputChannel) -> Result<bool> {
    let conflicted_files = edit_initial_index_state(ctx.legacy_project.id)
        .context("Failed to get conflicted files")?;

    let initially_conflicted: Vec<_> = conflicted_files
        .iter()
        .filter(|(_, conflict)| conflict.is_some())
        .collect();

    // Check which files still have conflict markers
    let git2_repo = ctx.git2_repo.get()?;
    let repo_path = git2_repo.workdir().context("No workdir")?;
    let mut still_conflicted = Vec::new();
    let mut resolved = Vec::new();

    for (change, _) in &initially_conflicted {
        let file_path = repo_path.join(change.path.to_str_lossy().as_ref());
        if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    if has_conflict_markers(&content) {
                        still_conflicted.push(change);
                    } else {
                        resolved.push(change);
                    }
                }
                Err(_) => {
                    // If we can't read the file, consider it still conflicted
                    still_conflicted.push(change);
                }
            }
        } else {
            // If file doesn't exist, it was deleted - consider it resolved
            resolved.push(change);
        }
    }

    let mut progress = out.progress_channel();

    let all_resolved = still_conflicted.is_empty();

    if all_resolved {
        if out.for_human().is_some() {
            writeln!(progress, "{}", "No conflicted files remaining!".green())?;
            if !resolved.is_empty() {
                writeln!(progress, "{} resolved:", "Files".green())?;
                for change in &resolved {
                    writeln!(
                        progress,
                        "  {} {}",
                        "✓".green(),
                        change.path.to_str_lossy().green()
                    )?;
                }
            }
        }
    } else if out.for_human().is_some() {
        writeln!(
            progress,
            "{}:",
            "Conflicted files remaining".yellow().bold()
        )?;
        for change in &still_conflicted {
            writeln!(
                progress,
                "  {} {}",
                "✗".red(),
                change.path.to_str_lossy().yellow()
            )?;
        }
        if !resolved.is_empty() {
            writeln!(progress, "\n{} resolved:", "Files".green())?;
            for change in &resolved {
                writeln!(
                    progress,
                    "  {} {}",
                    "✓".green(),
                    change.path.to_str_lossy().green()
                )?;
            }
        }
    } else if let Some(out) = out.for_json() {
        let conflicted_list: Vec<String> = still_conflicted
            .iter()
            .map(|change| change.path.to_str_lossy().to_string())
            .collect();
        let resolved_list: Vec<String> = resolved
            .iter()
            .map(|change| change.path.to_str_lossy().to_string())
            .collect();
        out.write_value(serde_json::json!({
            "conflicted_files": conflicted_list,
            "resolved_files": resolved_list,
            "conflicted_count": conflicted_list.len(),
            "resolved_count": resolved_list.len(),
            "all_resolved": all_resolved
        }))?;
    }

    Ok(all_resolved)
}

/// Check if a file contains git conflict markers
/// Matches the logic from the GUI's looksConflicted() function
fn has_conflict_markers(content: &str) -> bool {
    content.lines().any(|line| line.starts_with("<<<<<<<"))
}

fn finish_resolution(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    // Check if we're in edit mode
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if !matches!(mode, OperatingMode::Edit(_)) {
        // Not in edit mode, show the workflow help instead
        return show_workflow_help(out);
    }

    // Capture conflicted commits BEFORE the rebase
    let conflicts_before = find_conflicted_commits(ctx)?;

    // Save and return to workspace, capturing the rebase output
    save_edit_and_return_to_workspace_with_output(ctx.legacy_project.id)
        .context("Failed to save resolution and return to workspace")?;

    if let Some(human_out) = out.for_human() {
        writeln!(
            human_out,
            "{}",
            "✓ Conflict resolution finalized successfully!"
                .green()
                .bold()
        )?;
        writeln!(
            human_out,
            "The commit has been updated with your resolved changes."
        )?;
    }

    // Check for new conflicts introduced during the rebase
    check_for_new_conflicts_after_rebase(ctx, out, conflicts_before)?;

    Ok(())
}

fn cancel_resolution(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    // Check if we're in edit mode
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if !matches!(mode, OperatingMode::Edit(_)) {
        // Not in edit mode, show the workflow help instead
        return show_workflow_help(out);
    }

    // Abort and return to workspace
    abort_edit_and_return_to_workspace(ctx.legacy_project.id)
        .context("Failed to cancel resolution and return to workspace")?;

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Conflict resolution cancelled.".yellow())?;
        writeln!(
            out,
            "All changes made during resolution have been discarded."
        )?;
    }

    Ok(())
}

/// Structure to hold information about a conflicted commit
#[derive(Debug)]
struct ConflictedCommit {
    commit_oid: git2::Oid,
    commit_short_id: String,
    commit_message: String,
}

/// Check for new conflicts introduced during rebase and report them
fn check_for_new_conflicts_after_rebase(
    ctx: &mut Context,
    out: &mut OutputChannel,
    conflicts_before: BTreeMap<String, Vec<ConflictedCommit>>,
) -> Result<()> {
    // Get the current list of conflicted commits after the rebase
    let conflicts_after = find_conflicted_commits(ctx)?;

    // Build a set of commit OIDs that were conflicted before
    let mut oids_before = HashSet::new();
    for commits in conflicts_before.values() {
        for commit in commits {
            oids_before.insert(commit.commit_oid);
        }
    }

    // Find newly conflicted commits (present after but not before)
    let mut newly_conflicted: Vec<&ConflictedCommit> = Vec::new();
    for commits in conflicts_after.values() {
        for commit in commits {
            if !oids_before.contains(&commit.commit_oid) {
                newly_conflicted.push(commit);
            }
        }
    }

    // Report newly conflicted commits if any
    if !newly_conflicted.is_empty() {
        if let Some(human_out) = out.for_human() {
            writeln!(human_out)?;
            writeln!(
                human_out,
                "{}",
                "⚠ Warning: New conflicts were introduced during the rebase:"
                    .yellow()
                    .bold()
            )?;
            writeln!(human_out)?;

            for commit in &newly_conflicted {
                writeln!(
                    human_out,
                    "  {} {} {}",
                    "●".red(),
                    commit.commit_short_id.dimmed(),
                    commit.commit_message
                )?;
            }

            writeln!(human_out)?;
            writeln!(
                human_out,
                "Run {} to see all conflicted commits, or {} to resolve them.",
                "but status".green(),
                "but resolve <commit>".green()
            )?;
        } else if let Some(json_out) = out.for_json() {
            let newly_conflicted_json: Vec<serde_json::Value> = newly_conflicted
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "commit_id": c.commit_oid.to_string(),
                        "commit_short_id": c.commit_short_id,
                        "commit_message": c.commit_message,
                    })
                })
                .collect();

            json_out.write_value(serde_json::json!({
                "newly_conflicted_commits": newly_conflicted_json,
                "count": newly_conflicted.len(),
            }))?;
        }
    }

    Ok(())
}

/// Find all conflicted commits across all stacks, grouped by branch
fn find_conflicted_commits(ctx: &mut Context) -> Result<BTreeMap<String, Vec<ConflictedCommit>>> {
    let stacks = but_api::legacy::workspace::stacks(ctx.legacy_project.id, None)?;
    let git2_repo = ctx.git2_repo.get()?;
    let mut conflicts_by_branch: BTreeMap<String, Vec<ConflictedCommit>> = BTreeMap::new();

    for stack in &stacks {
        // Check commits in each head of the stack
        for head in &stack.heads {
            let branch_name = head.name.to_str_lossy().to_string();
            let head_oid = git2::Oid::from_bytes(head.tip.as_slice())?;

            // Walk the commit history to find conflicted commits
            let mut revwalk = git2_repo.revwalk()?;
            revwalk.push(head_oid)?;
            revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

            for oid_result in revwalk {
                let oid = oid_result?;
                let commit = git2_repo.find_commit(oid)?;

                if commit.is_conflicted() {
                    let message = commit
                        .message()
                        .unwrap_or("")
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(50)
                        .collect::<String>();

                    let conflicted = ConflictedCommit {
                        commit_oid: oid,
                        commit_short_id: oid.to_string()[..7].to_string(),
                        commit_message: message,
                    };

                    conflicts_by_branch
                        .entry(branch_name.clone())
                        .or_default()
                        .push(conflicted);
                }
            }
        }
    }

    Ok(conflicts_by_branch)
}

/// Check for conflicted commits and prompt user to resolve them
fn check_and_prompt_for_conflicts(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    // Find all conflicted commits
    let conflicts_by_branch = find_conflicted_commits(ctx)?;

    if conflicts_by_branch.is_empty() {
        // No conflicts found, show the normal help text
        return show_workflow_help(out);
    }

    let mut progress = out.progress_channel();

    // We have conflicts - show them grouped by branch
    if out.for_human().is_some() {
        writeln!(progress, "{}", "Found conflicted commits:".yellow().bold())?;
        writeln!(progress)?;

        let mut all_commits: Vec<&ConflictedCommit> = vec![];

        for (branch_name, commits) in &conflicts_by_branch {
            writeln!(progress, "{} {}", "Branch:".bold(), branch_name.green())?;
            for commit in commits {
                writeln!(
                    progress,
                    "  {} {} {}",
                    "●".red(),
                    commit.commit_short_id.dimmed(),
                    commit.commit_message
                )?;
                all_commits.push(commit);
            }
            writeln!(progress)?;
        }

        // Prompt user to select a commit to resolve
        writeln!(
            progress,
            "{}",
            "Would you like to start resolving these conflicts?".bold()
        )?;

        // Find the bottom-most commit (first in topological order) on the first branch
        let default_commit = all_commits.first();

        if let Some(default) = default_commit {
            write!(
                progress,
                "Enter commit ID to resolve [default: {}]: ",
                default.commit_short_id.cyan()
            )?;

            let mut response = String::new();
            std::io::stdin().read_line(&mut response)?;
            let response = response.trim();

            let commit_id_to_resolve = if response.is_empty() {
                default.commit_short_id.clone()
            } else {
                response.to_string()
            };

            // Enter resolution mode for the selected commit
            writeln!(progress)?;
            return enter_resolution(ctx, out, &commit_id_to_resolve);
        }
    } else if let Some(json_out) = out.for_json() {
        // JSON output mode
        let mut json_conflicts = serde_json::Map::new();

        for (branch_name, commits) in &conflicts_by_branch {
            let commits_array: Vec<serde_json::Value> = commits
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "commit_id": c.commit_oid.to_string(),
                        "commit_short_id": c.commit_short_id,
                        "commit_message": c.commit_message,
                    })
                })
                .collect();

            json_conflicts.insert(branch_name.clone(), serde_json::Value::Array(commits_array));
        }

        json_out.write_value(serde_json::json!({
            "conflicted_commits_by_branch": json_conflicts,
            "total_conflicted_commits": conflicts_by_branch.values().map(|v| v.len()).sum::<usize>(),
        }))?;
    }

    Ok(())
}

fn show_workflow_help(out: &mut OutputChannel) -> Result<()> {
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Conflict Resolution Workflow".bold().underline())?;
        writeln!(out)?;
        writeln!(
            out,
            "This command is used when you have a commit in a conflicted state"
        )?;
        writeln!(out)?;
        writeln!(out, "{}", "To resolve conflicts in a commit:".bold())?;
        writeln!(out)?;
        writeln!(
            out,
            "  {} Enter resolution mode for a conflicted commit:",
            "1.".bold()
        )?;
        writeln!(out, "     {}", "but resolve <commit>".green())?;
        writeln!(out)?;
        writeln!(
            out,
            "  {} Resolve conflicts in the conflicted files",
            "2.".bold()
        )?;
        writeln!(
            out,
            "     Edit the files to remove conflict markers ({}, {}, {})",
            "<<<<<<<".red(),
            "=======".yellow(),
            ">>>>>>>".red()
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "  {} Check which files are still conflicted:",
            "3.".bold()
        )?;
        writeln!(out, "     {}", "but resolve status".green())?;
        writeln!(out)?;
        writeln!(out, "  {} Finalize or cancel the resolution:", "4.".bold())?;
        writeln!(out, "     {}", "but resolve finish".green())?;
        writeln!(out, "     {}", "OR".dimmed())?;
        writeln!(out, "     {}", "but resolve cancel".green())?;
        writeln!(out)?;
        writeln!(out, "{}", "Example:".bold())?;
        writeln!(out, "  {} (find conflicted commits)", "but status".green())?;
        writeln!(
            out,
            "  {} (enter resolution mode)",
            "but resolve 55".green()
        )?;
        writeln!(
            out,
            "  {} (edit files to resolve conflicts)",
            "vim src/file.rs".dimmed()
        )?;
        writeln!(
            out,
            "  {} (check remaining conflicts)",
            "but resolve status".green()
        )?;
        writeln!(out, "  {} (finalize)", "but resolve finish".green())?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "workflow": [
                {
                    "step": 1,
                    "description": "Enter resolution mode for a conflicted commit",
                    "command": "but resolve <commit>"
                },
                {
                    "step": 2,
                    "description": "Resolve conflicts in the conflicted files",
                    "details": "Edit the files to remove conflict markers (<<<<<<<, =======, >>>>>>>)"
                },
                {
                    "step": 3,
                    "description": "Check which files are still conflicted",
                    "command": "but resolve status"
                },
                {
                    "step": 4,
                    "description": "Finalize the resolution",
                    "command": "but resolve finish"
                }
            ],
            "other_commands": {
                "cancel": "but resolve cancel",
                "view_status": "but status"
            }
        }))?;
    }

    Ok(())
}
