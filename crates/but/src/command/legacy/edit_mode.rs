use std::fmt::Write;

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api::legacy::modes::{
    abort_edit_and_return_to_workspace, enter_edit_mode,
    save_edit_and_return_to_workspace_with_output,
};
use but_core::ui::TreeStatus;
use but_ctx::Context;
use colored::Colorize;
use gitbutler_edit_mode::commands::changes_from_initial;
use gitbutler_operating_modes::OperatingMode;

use crate::{IdMap, args::edit_mode::Subcommands, id::CliId, utils::OutputChannel};

/// Handle the `but edit-mode` command and its subcommands.
pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    cmd: Option<Subcommands>,
    commit_id: Option<String>,
) -> Result<()> {
    match cmd {
        Some(Subcommands::Status) => show_status(ctx, out),
        Some(Subcommands::Finish) => finish_edit(ctx, out),
        Some(Subcommands::Cancel { force }) => cancel_edit(ctx, out, force),
        None => {
            if let Some(commit_id_str) = commit_id {
                enter_edit(ctx, out, &commit_id_str)
            } else {
                let mode = gitbutler_operating_modes::operating_mode(ctx);
                if matches!(mode, OperatingMode::Edit(_)) {
                    show_status(ctx, out)
                } else {
                    show_workflow_help(out)
                }
            }
        }
    }
}

fn enter_edit(ctx: &mut Context, out: &mut OutputChannel, commit_id_str: &str) -> Result<()> {
    use gix::{prelude::ObjectIdExt as _, revision::walk::Sorting};

    let id_map = IdMap::new_from_context(ctx, None)?;
    let matches = id_map.parse_using_context(commit_id_str, ctx)?;

    if matches.is_empty() {
        bail!(
            "Commit '{commit_id_str}' not found. Try running 'but status' to see available commits."
        );
    }

    if matches.len() > 1 {
        bail!(
            "Commit ID '{commit_id_str}' is ambiguous. Please provide more characters to uniquely identify the commit."
        );
    }

    let commit_gix_oid = match &matches[0] {
        CliId::Commit { commit_id, .. } => *commit_id,
        _ => bail!("'{commit_id_str}' does not refer to a commit"),
    };

    // Find which stack this commit belongs to
    let stacks = but_api::legacy::workspace::stacks(ctx, None)?;
    let gix_repo = ctx.repo.get()?;
    let mut found_stack_id = None;
    'outer: for stack in &stacks {
        for head in &stack.heads {
            let traversal = head
                .tip
                .attach(&gix_repo)
                .ancestors()
                .sorting(Sorting::BreadthFirst)
                .all()?;

            for info in traversal {
                let info = info?;
                if info.id == commit_gix_oid {
                    found_stack_id = stack.id;
                    break 'outer;
                }
            }
        }
    }

    let stack_id = found_stack_id.ok_or_else(|| {
        anyhow::anyhow!(
            "Could not find stack containing commit {}",
            &commit_gix_oid.to_string()[..7]
        )
    })?;

    drop(gix_repo);

    enter_edit_mode(ctx, commit_gix_oid, stack_id).context("Failed to enter edit mode")?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} {}",
            "Entering edit mode for commit".bold(),
            commit_gix_oid.to_string()[..7].cyan()
        )?;
    }

    show_status(ctx, out)
}

fn show_status(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    show_status_impl(ctx, out)
}

/// Show edit status without prompting (for use by `but status`).
pub(crate) fn show_edit_status(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    show_status_impl(ctx, out)
}

fn show_status_impl(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if !matches!(mode, OperatingMode::Edit(_)) {
        return show_workflow_help(out);
    }

    let mut progress = out.progress_channel();

    writeln!(
        progress,
        "{}\n - make your changes\n - save with {}\n - OR cancel with {}\n",
        "You are currently in edit mode.".bold(),
        "but edit-mode finish".green().bold(),
        "but edit-mode cancel".red().bold()
    )?;

    show_changed_files(ctx, out)?;

    Ok(())
}

fn show_changed_files(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    let changes = changes_from_initial(ctx)?;
    let mut progress = out.progress_channel();

    if changes.is_empty() {
        writeln!(progress, "{}", "No changes made yet.".dimmed())?;
    } else {
        writeln!(progress, "{}:", "Changed files".yellow().bold())?;
        for change in &changes {
            let indicator = match &change.status {
                TreeStatus::Addition { .. } => "+".green(),
                TreeStatus::Deletion { .. } => "-".red(),
                TreeStatus::Modification { .. } => "~".yellow(),
                TreeStatus::Rename { .. } => "→".cyan(),
            };
            writeln!(progress, "  {} {}", indicator, change.path.to_str_lossy())?;
        }
    }

    if let Some(json_out) = out.for_json() {
        let files: Vec<String> = changes
            .iter()
            .map(|c| c.path.to_str_lossy().to_string())
            .collect();
        json_out.write_value(serde_json::json!({
            "changed_files": files,
            "changed_count": files.len(),
        }))?;
    }

    Ok(())
}

fn finish_edit(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if !matches!(mode, OperatingMode::Edit(_)) {
        return show_workflow_help(out);
    }

    // Capture conflicted commits before the rebase
    let conflicts_before = super::resolve::find_conflicted_commits(ctx)?;

    save_edit_and_return_to_workspace_with_output(ctx)
        .context("Failed to save edit and return to workspace")?;

    if let Some(human_out) = out.for_human() {
        writeln!(human_out, "{}", "Edit saved successfully!".green().bold())?;
        writeln!(human_out, "The commit has been updated with your changes.")?;
    }

    // Check for new conflicts introduced during the rebase
    super::resolve::check_for_new_conflicts_after_rebase(ctx, out, conflicts_before)?;

    Ok(())
}

fn cancel_edit(ctx: &mut Context, out: &mut OutputChannel, force: bool) -> Result<()> {
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if !matches!(mode, OperatingMode::Edit(_)) {
        return show_workflow_help(out);
    }

    if !force && !changes_from_initial(ctx)?.is_empty() {
        bail!(
            "There are changes that differ from the original commit you were editing. Canceling will drop those changes.\n\nIf you want to go through with this, please re-run with `--force`.\n\nIf you want to keep the changes you have made, consider finishing the edit with `but edit-mode finish`."
        )
    }

    abort_edit_and_return_to_workspace(ctx, force)
        .context("Failed to cancel edit and return to workspace")?;

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Edit cancelled.".yellow())?;
        writeln!(out, "All changes made during editing have been discarded.")?;
    }

    Ok(())
}

fn show_workflow_help(out: &mut OutputChannel) -> Result<()> {
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Edit Mode Workflow".bold().underline())?;
        writeln!(out)?;
        writeln!(out, "This command lets you edit any commit in your stack.")?;
        writeln!(out)?;
        writeln!(out, "{}", "To edit a commit:".bold())?;
        writeln!(out)?;
        writeln!(out, "  {} Enter edit mode for a commit:", "1.".bold())?;
        writeln!(out, "     {}", "but edit-mode <commit>".green())?;
        writeln!(out)?;
        writeln!(out, "  {} Make your changes to the files", "2.".bold())?;
        writeln!(out)?;
        writeln!(out, "  {} Check what has changed:", "3.".bold())?;
        writeln!(out, "     {}", "but edit-mode status".green())?;
        writeln!(out)?;
        writeln!(out, "  {} Save or cancel:", "4.".bold())?;
        writeln!(out, "     {}", "but edit-mode finish".green())?;
        writeln!(out, "     {}", "OR".dimmed())?;
        writeln!(out, "     {}", "but edit-mode cancel".green())?;
        writeln!(out)?;
        writeln!(out, "{}", "Example:".bold())?;
        writeln!(out, "  {} (find commits)", "but status".green())?;
        writeln!(out, "  {} (enter edit mode)", "but edit-mode 55".green())?;
        writeln!(out, "  {} (make changes)", "vim src/file.rs".dimmed())?;
        writeln!(out, "  {} (check changes)", "but edit-mode status".green())?;
        writeln!(out, "  {} (save)", "but edit-mode finish".green())?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "workflow": [
                {
                    "step": 1,
                    "description": "Enter edit mode for a commit",
                    "command": "but edit-mode <commit>"
                },
                {
                    "step": 2,
                    "description": "Make your changes to the files"
                },
                {
                    "step": 3,
                    "description": "Check what has changed",
                    "command": "but edit-mode status"
                },
                {
                    "step": 4,
                    "description": "Save or cancel",
                    "command": "but edit-mode finish"
                }
            ],
            "other_commands": {
                "cancel": "but edit-mode cancel",
                "view_status": "but status"
            }
        }))?;
    }

    Ok(())
}
