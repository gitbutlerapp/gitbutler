use anyhow::bail;
use but_ctx::Context;
use but_oxidize::TimeExt;
use colored::Colorize;
use gitbutler_oplog::entry::{OperationKind, Snapshot};
use gix::date::time::CustomFormat;

use crate::utils::OutputChannel;

pub const ISO8601_NO_TZ: CustomFormat = CustomFormat::new("%Y-%m-%d %H:%M:%S");

pub(crate) fn show_oplog(
    ctx: &mut Context,
    out: &mut OutputChannel,
    since: Option<&str>,
) -> anyhow::Result<()> {
    let snapshots = if let Some(since_sha) = since {
        // Get all snapshots first to find the starting point
        let all_snapshots =
            but_api::legacy::oplog::list_snapshots(ctx.legacy_project.id, 1000, None, None)?; // Get a large number to find the SHA
        let mut found_index = None;

        // Find the snapshot that matches the since SHA (partial match supported)
        for (index, snapshot) in all_snapshots.iter().enumerate() {
            let snapshot_sha = snapshot.commit_id.to_string();
            if snapshot_sha.starts_with(since_sha) {
                found_index = Some(index);
                break;
            }
        }

        match found_index {
            Some(index) => {
                // Take 20 entries starting from the found index
                all_snapshots.into_iter().skip(index).take(20).collect()
            }
            None => {
                return Err(anyhow::anyhow!(
                    "No oplog entry found matching SHA: {}",
                    since_sha
                ));
            }
        }
    } else {
        but_api::legacy::oplog::list_snapshots(ctx.legacy_project.id, 20, None, None)?
    };

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
                    _ => "OTHER",
                };
                (op_type, details.title.clone())
            } else {
                ("UNKNOWN", "Unknown operation".to_string())
            };

            let operation_colored = match operation_type {
                "CREATE" => operation_type.green(),
                "AMEND" | "REWORD" => operation_type.yellow(),
                "UNDO" | "RESTORE" => operation_type.red(),
                "BRANCH" | "CHECKOUT" => operation_type.purple(),
                "MOVE" | "REORDER" | "MOVE_HUNK" => operation_type.cyan(),
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
    ctx: &mut Context,
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

    if let Some(out) = out.for_human() {
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
            use std::io::Write;
            let mut stdout = std::io::stdout();

            writeln!(
                stdout,
                "\n{}",
                "⚠️  This will overwrite your current workspace state."
                    .yellow()
                    .bold()
            )?;
            write!(stdout, "Continue with restore? [y/N]: ")?;
            std::io::stdout().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            let input = input.trim().to_lowercase();
            if input != "y" && input != "yes" {
                writeln!(stdout, "{}", "Restore cancelled.".yellow())?;
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
    ctx: &mut Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Get the last two snapshots - restore to the second one back
    let snapshots = but_api::legacy::oplog::list_snapshots(ctx.legacy_project.id, 2, None, None)?;

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
    ctx: &mut Context,
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
