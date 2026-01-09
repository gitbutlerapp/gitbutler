use anyhow::{Context, bail};
use but_oxidize::TimeExt;
use colored::Colorize;
use gitbutler_oplog::entry::{OperationKind, Snapshot};
use gix::date::time::CustomFormat;

use crate::utils::OutputChannel;

pub const ISO8601_NO_TZ: CustomFormat = CustomFormat::new("%Y-%m-%d %H:%M:%S");

/// Filter for oplog entries by operation kind
#[derive(Debug, Clone, Copy)]
pub enum OplogFilter {
    /// Show only on-demand snapshot entries
    Snapshot,
}

impl OplogFilter {
    /// Convert the filter to a list of OperationKind to include
    fn to_include_kinds(self) -> Vec<OperationKind> {
        match self {
            OplogFilter::Snapshot => vec![OperationKind::OnDemandSnapshot],
        }
    }
}

pub(crate) fn show_oplog(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    since: Option<&str>,
    filter: Option<OplogFilter>,
) -> anyhow::Result<()> {
    // Convert filter to include_kind parameter for the API
    let include_kind = filter.map(|f| f.to_include_kinds());

    // Resolve partial SHA to full SHA using rev_parse if provided
    let since_sha = if let Some(sha_prefix) = since {
        let repo = ctx.repo.get()?;
        let resolved = repo
            .rev_parse_single(sha_prefix)
            .map_err(|_| anyhow::anyhow!("No oplog entry found matching SHA: {}", sha_prefix))?;
        Some(resolved.detach().to_string())
    } else {
        None
    };

    let snapshots = but_api::legacy::oplog::list_snapshots(
        ctx.legacy_project.id,
        20,
        since_sha,
        None,
        include_kind,
    )?;

    if snapshots.is_empty() {
        if let Some(out) = out.for_json() {
            out.write_value(&snapshots)?;
        } else if let Some(out) = out.for_human() {
            writeln!(out, "No operations found in history.")?;
        }
        return Ok(());
    }

    if let Some(out) = out.for_json() {
        out.write_value(&snapshots)?;
    } else if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Operations History".blue().bold())?;
        writeln!(out, "{}", "─".repeat(50).dimmed())?;

        for snapshot in snapshots {
            let time_string = snapshot_time_string(&snapshot);

            let commit_id = format!(
                "{}{}",
                &snapshot.commit_id.to_string()[..7].blue().underline(),
                &snapshot.commit_id.to_string()[7..12].blue().dimmed()
            );

            let (operation_type, title) = if let Some(details) = &snapshot.details {
                let op_type = match details.operation {
                    OperationKind::CreateCommit => "CREATE",
                    OperationKind::CreateBranch => "BRANCH",
                    OperationKind::AmendCommit => "AMEND",
                    OperationKind::Absorb => "ABSORB",
                    OperationKind::UndoCommit => "UNDO",
                    OperationKind::SquashCommit => "SQUASH",
                    OperationKind::UpdateCommitMessage => "REWORD",
                    OperationKind::MoveCommit => "MOVE",
                    OperationKind::RestoreFromSnapshot => "RESTORE",
                    OperationKind::ReorderCommit => "REORDER",
                    OperationKind::InsertBlankCommit => "INSERT",
                    OperationKind::MoveHunk => "MOVE_HUNK",
                    OperationKind::ReorderBranches => "REORDER_BRANCH",
                    OperationKind::UpdateWorkspaceBase => "UPDATE_BASE",
                    OperationKind::UpdateBranchName => "RENAME",
                    OperationKind::GenericBranchUpdate => "BRANCH_UPDATE",
                    OperationKind::ApplyBranch => "APPLY",
                    OperationKind::UnapplyBranch => "UNAPPLY",
                    OperationKind::DeleteBranch => "DELETE",
                    OperationKind::DiscardChanges => "DISCARD",
                    OperationKind::Discard => "DISCARD",
                    OperationKind::OnDemandSnapshot => "SNAPSHOT",
                    _ => "OTHER",
                };
                // For OnDemandSnapshot, show the message (body) if available
                // For Discard, show file names from trailers if available
                let display_title = if details.operation == OperationKind::OnDemandSnapshot {
                    details
                        .body
                        .as_ref()
                        .filter(|b| !b.is_empty())
                        .cloned()
                        .unwrap_or_else(|| details.title.clone())
                } else if details.operation == OperationKind::Discard {
                    // Extract file names from trailers
                    let file_names: Vec<String> = details
                        .trailers
                        .iter()
                        .filter(|t| t.key == "file")
                        .map(|t| t.value.clone())
                        .collect();

                    if !file_names.is_empty() {
                        format!("{} ({})", details.title, file_names.join(", "))
                    } else {
                        details.title.clone()
                    }
                } else {
                    details.title.clone()
                };

                // Truncate display_title to 80 characters
                let display_title = if display_title.chars().count() > 80 {
                    let truncated: String = display_title.chars().take(77).collect();
                    format!("{}...", truncated)
                } else {
                    display_title
                };

                (op_type, display_title)
            } else {
                ("UNKNOWN", "Unknown operation".to_string())
            };

            let operation_colored = match operation_type {
                "CREATE" => operation_type.green(),
                "AMEND" | "REWORD" => operation_type.yellow(),
                "UNDO" | "RESTORE" => operation_type.red(),
                "DISCARD" => operation_type.red().bold(),
                "BRANCH" | "CHECKOUT" => operation_type.purple(),
                "MOVE" | "REORDER" | "MOVE_HUNK" => operation_type.cyan(),
                "SNAPSHOT" => operation_type.bright_magenta(),
                _ => operation_type.normal(),
            };

            writeln!(
                out,
                "{} {} [{}] {}",
                commit_id,
                time_string.dimmed(),
                operation_colored,
                title
            )?;
        }
    }

    Ok(())
}

fn snapshot_time_string(snapshot: &Snapshot) -> String {
    let time = snapshot.created_at.to_gix();
    // TODO: use `format_or_unix`.
    time.format(ISO8601_NO_TZ)
        .unwrap_or_else(|_| time.seconds.to_string())
}

pub(crate) fn restore_to_oplog(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    oplog_sha: &str,
    force: bool,
) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let commit_id = repo.rev_parse_single(oplog_sha)?.detach();
    let target_snapshot =
        &but_api::legacy::oplog::get_snapshot(ctx.legacy_project.id, commit_id.to_string())?;

    let commit_sha_string = commit_id.to_string();

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = snapshot_time_string(target_snapshot);

    if let Some(mut out) = out.prepare_for_terminal_input() {
        use std::fmt::Write;
        writeln!(out, "{}", "Restoring to oplog snapshot...".blue().bold())?;
        writeln!(
            out,
            "  Target: {} ({})",
            target_operation.green(),
            target_time.dimmed()
        )?;
        writeln!(
            out,
            "  Snapshot: {}",
            commit_sha_string[..7].cyan().underline()
        )?;

        // Confirm the restoration (safety check)
        if !force {
            writeln!(
                out,
                "\n{}",
                "⚠️  This will overwrite your current workspace state."
                    .yellow()
                    .bold()
            )?;
            let input = out
                .prompt("Continue with restore? [y/N]: ")?
                .context("Restore cancelled.".yellow())?
                .to_lowercase();

            if input != "y" && input != "yes" {
                return Ok(());
            }
        }
    }

    // Restore to the target snapshot using the but-api crate
    if force {
        but_api::legacy::oplog::restore_snapshot(ctx.legacy_project.id, commit_sha_string)?;
    } else {
        bail!("Unable to possibly overwrite changes in the worktree without --force");
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "\n{} Restore completed successfully!",
            "✓".green().bold(),
        )?;

        writeln!(
            out,
            "{}",
            "\nWorkspace has been restored to the selected snapshot.".green()
        )?;
    }

    Ok(())
}

pub(crate) fn undo_last_operation(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Get the last two snapshots - restore to the second one back
    let snapshots =
        but_api::legacy::oplog::list_snapshots(ctx.legacy_project.id, 2, None, None, None)?;

    if snapshots.len() < 2 {
        if let Some(out) = out.for_human() {
            writeln!(out, "{}", "No previous operations to undo.".yellow())?;
        }
        return Ok(());
    }

    // TODO: Why the second most recent one, and not use the most recent one?
    let target_snapshot = &snapshots[1];

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = snapshot_time_string(target_snapshot);

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Undoing operation...".blue().bold())?;
        writeln!(
            out,
            "  Reverting to: {} ({})",
            target_operation.green(),
            target_time.dimmed()
        )?;
    }

    // Restore to the previous snapshot using the but_api
    // TODO: Why does this not require force? It will also overwrite user changes (I think).
    but_api::legacy::oplog::restore_snapshot(
        ctx.legacy_project.id,
        target_snapshot.commit_id.to_string(),
    )?;

    if let Some(out) = out.for_human() {
        let restore_commit_short = format!(
            "{}{}",
            &target_snapshot.commit_id.to_string()[..7]
                .blue()
                .underline(),
            &target_snapshot.commit_id.to_string()[7..12].blue().dimmed()
        );

        writeln!(
            out,
            "{} Undo completed successfully! Restored to snapshot: {}",
            "✓".green().bold(),
            restore_commit_short
        )?;
    }

    Ok(())
}

pub(crate) fn create_snapshot(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    message: Option<&str>,
) -> anyhow::Result<()> {
    let snapshot_id =
        but_api::legacy::oplog::create_snapshot(ctx.legacy_project.id, message.map(String::from))?;

    if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "snapshot_id": snapshot_id.to_string(),
            "message": message.unwrap_or(""),
            "operation": "create_snapshot"
        }))?;
    } else if let Some(out) = out.for_human() {
        writeln!(out, "{}", "Snapshot created successfully!".green().bold())?;

        if let Some(msg) = message {
            writeln!(out, "  Message: {}", msg.cyan())?;
        }

        writeln!(
            out,
            "  Snapshot ID: {}{}",
            snapshot_id.to_string()[..7].blue().underline(),
            snapshot_id.to_string()[7..12].blue().dimmed()
        )?;

        writeln!(
            out,
            "\n{} Use 'but restore {}' to restore to this snapshot later.",
            "💡".bright_blue(),
            &snapshot_id.to_string()[..7]
        )?;
    }

    Ok(())
}
