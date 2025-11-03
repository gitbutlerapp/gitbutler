use colored::Colorize;
use gitbutler_oplog::entry::OperationKind;
use gitbutler_oxidize::TimeExt;
use gitbutler_project::Project;

pub(crate) fn show_oplog(project: &Project, json: bool, since: Option<&str>) -> anyhow::Result<()> {
    let snapshots = if let Some(since_sha) = since {
        // Get all snapshots first to find the starting point
        let all_snapshots = but_api::oplog::list_snapshots(project.id, 1000, None, None)?; // Get a large number to find the SHA
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
        but_api::oplog::list_snapshots(project.id, 20, None, None)?
    };

    if snapshots.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No operations found in history.");
        }
        return Ok(());
    }

    if json {
        // Output JSON format
        let json_output = serde_json::to_string_pretty(&snapshots)?;
        println!("{json_output}");
    } else {
        // Output human-readable format
        println!("{}", "Operations History".blue().bold());
        println!("{}", "─".repeat(50).dimmed());

        for snapshot in snapshots {
            let time_string = snapshot
                .created_at
                .to_gix()
                .format(gix::date::time::format::ISO8601);

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

            println!(
                "{} {} [{}] {}",
                commit_id,
                time_string.dimmed(),
                operation_colored,
                title
            );
        }
    }

    Ok(())
}

pub(crate) fn restore_to_oplog(
    project: &Project,
    _json: bool,
    oplog_sha: &str,
    force: bool,
) -> anyhow::Result<()> {
    let snapshots = but_api::oplog::list_snapshots(project.id, 1000, None, None)?;

    // Parse the oplog SHA (support partial SHAs)
    let commit_sha_string = if oplog_sha.len() >= 7 {
        // Try to find a snapshot that starts with this SHA

        let matching_snapshot = snapshots
            .iter()
            .find(|snapshot| snapshot.commit_id.to_string().starts_with(oplog_sha))
            .ok_or_else(|| anyhow::anyhow!("No oplog snapshot found matching '{}'", oplog_sha))?;

        matching_snapshot.commit_id.to_string()
    } else {
        anyhow::bail!("Oplog SHA must be at least 7 characters long");
    };

    // Get information about the target snapshot
    let target_snapshot = snapshots
        .iter()
        .find(|snapshot| snapshot.commit_id.to_string() == commit_sha_string)
        .ok_or_else(|| anyhow::anyhow!("Snapshot {} not found in oplog", commit_sha_string))?;

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = target_snapshot
        .created_at
        .to_gix()
        .format(gix::date::time::format::ISO8601);

    println!("{}", "Restoring to oplog snapshot...".blue().bold());
    println!(
        "  Target: {} ({})",
        target_operation.green(),
        target_time.dimmed()
    );
    println!("  Snapshot: {}", commit_sha_string[..7].cyan().underline());

    // Confirm the restoration (safety check)
    if !force {
        println!(
            "\n{}",
            "⚠️  This will overwrite your current workspace state."
                .yellow()
                .bold()
        );
        print!("Continue with restore? [y/N]: ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("{}", "Restore cancelled.".yellow());
            return Ok(());
        }
    }

    // Restore to the target snapshot using the but-api crate
    but_api::oplog::restore_snapshot(project.id, commit_sha_string)?;

    println!("\n{} Restore completed successfully!", "✓".green().bold(),);

    println!(
        "{}",
        "\nWorkspace has been restored to the selected snapshot.".green()
    );

    Ok(())
}

pub(crate) fn undo_last_operation(project: &Project, _json: bool) -> anyhow::Result<()> {
    // Get the last two snapshots - restore to the second one back
    let snapshots = but_api::oplog::list_snapshots(project.id, 2, None, None)?;

    if snapshots.len() < 2 {
        println!("{}", "No previous operations to undo.".yellow());
        return Ok(());
    }

    let target_snapshot = &snapshots[1];

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = target_snapshot
        .created_at
        .to_gix()
        .format(gix::date::time::format::ISO8601);

    println!("{}", "Undoing operation...".blue().bold());
    println!(
        "  Reverting to: {} ({})",
        target_operation.green(),
        target_time.dimmed()
    );

    // Restore to the previous snapshot using the but_api
    but_api::oplog::restore_snapshot(project.id, target_snapshot.commit_id.to_string())?;

    let restore_commit_short = format!(
        "{}{}",
        &target_snapshot.commit_id.to_string()[..7]
            .blue()
            .underline(),
        &target_snapshot.commit_id.to_string()[7..12].blue().dimmed()
    );

    println!(
        "{} Undo completed successfully! Restored to snapshot: {}",
        "✓".green().bold(),
        restore_commit_short
    );

    Ok(())
}

pub(crate) fn create_snapshot(
    project: &Project,
    json: bool,
    message: Option<&str>,
) -> anyhow::Result<()> {
    let snapshot_id = but_api::oplog::create_snapshot(project.id, message.map(String::from))?;

    if json {
        let output = serde_json::json!({
            "snapshot_id": snapshot_id.to_string(),
            "message": message.unwrap_or(""),
            "operation": "create_snapshot"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", "Snapshot created successfully!".green().bold());

        if let Some(msg) = message {
            println!("  Message: {}", msg.cyan());
        }

        println!(
            "  Snapshot ID: {}{}",
            snapshot_id.to_string()[..7].blue().underline(),
            snapshot_id.to_string()[7..12].blue().dimmed()
        );

        println!(
            "\n{} Use 'but restore {}' to restore to this snapshot later.",
            "💡".bright_blue(),
            &snapshot_id.to_string()[..7]
        );
    }

    Ok(())
}
