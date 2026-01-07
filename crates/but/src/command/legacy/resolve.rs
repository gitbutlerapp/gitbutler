use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api::legacy::modes::{
    abort_edit_and_return_to_workspace, edit_initial_index_state, enter_edit_mode,
    save_edit_and_return_to_workspace,
};
use but_ctx::Context;
use colored::Colorize;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_operating_modes::OperatingMode;

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
                    // Otherwise, show helpful workflow guide
                    show_workflow_help(out)
                }
            }
        }
    }
}

fn enter_resolution(ctx: &mut Context, out: &mut OutputChannel, commit_id_str: &str) -> Result<()> {
    // Create an IdMap to resolve commit IDs (supports both CLI IDs and partial SHAs)
    let id_map = IdMap::new_from_context(ctx)?;

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
        CliId::Commit(oid) => git2::Oid::from_bytes(oid.as_slice())?,
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

    // Show user-friendly output
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} {}",
            "Checking out conflicted commit".bold(),
            commit_oid.to_string()[..7].cyan()
        )?;
        writeln!(
            out,
            "You are now in edit mode - resolve all conflicts and finalize with {} or cancel with {}",
            "but resolve finish".green().bold(),
            "but resolve cancel".red().bold()
        )?;
        writeln!(
            out,
            "  view remaining issues with {}",
            "but resolve status".yellow().bold()
        )?;
    }

    // Show initial conflicted files
    show_conflicted_files(ctx, out)?;

    Ok(())
}

fn show_status(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    // Check if we're in edit mode
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if !matches!(mode, OperatingMode::Edit(_)) {
        // Not in edit mode, show the workflow help instead
        return show_workflow_help(out);
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}\n - resolve all conflicts \n - finalize with {} \n - OR cancel with {}\n",
            "You are currently in conflict resolution mode.".bold(),
            "but resolve finish".green().bold(),
            "but resolve cancel".red().bold()
        )?;
    }

    show_conflicted_files(ctx, out)?;

    Ok(())
}

fn show_conflicted_files(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
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

    if still_conflicted.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "{}", "No conflicted files remaining!".green())?;
            if !resolved.is_empty() {
                writeln!(out, "{} resolved:", "Files".green())?;
                for change in &resolved {
                    writeln!(
                        out,
                        "  {} {}",
                        "✓".green(),
                        change.path.to_str_lossy().green()
                    )?;
                }
            }
            writeln!(
                out,
                "Run {} to finalize the resolution.",
                "but resolve finish".green().bold()
            )?;
        }
    } else {
        if let Some(out) = out.for_human() {
            writeln!(out, "{}:", "Conflicted files remaining".yellow().bold())?;
            for change in &still_conflicted {
                writeln!(
                    out,
                    "  {} {}",
                    "✗".red(),
                    change.path.to_str_lossy().yellow()
                )?;
            }
            if !resolved.is_empty() {
                writeln!(out, "\n{} resolved:", "Files".green())?;
                for change in &resolved {
                    writeln!(
                        out,
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
            out.write_value(&serde_json::json!({
                "conflicted_files": conflicted_list,
                "resolved_files": resolved_list,
                "conflicted_count": conflicted_list.len(),
                "resolved_count": resolved_list.len()
            }))?;
        }
    }

    Ok(())
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

    // Note: We don't check if conflicts are actually resolved because:
    // 1. The backend API doesn't validate this - it just commits the working directory
    // 2. There's no reliable way to detect if conflict markers were resolved
    // 3. The GUI trusts the user to resolve conflicts before clicking "Save"
    // The user can run `but resolve status` to see which files were originally conflicted

    // Save and return to workspace
    save_edit_and_return_to_workspace(ctx.legacy_project.id)
        .context("Failed to save resolution and return to workspace")?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            "✓ Conflict resolution finalized successfully!"
                .green()
                .bold()
        )?;
        writeln!(
            out,
            "The commit has been updated with your resolved changes."
        )?;
    }

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
        out.write_value(&serde_json::json!({
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
