use colored::Colorize;
use gitbutler_oplog::entry::OperationKind;
use gitbutler_project::Project;
use std::path::Path;

pub(crate) fn show_oplog(repo_path: &Path, json: bool, since: Option<&str>) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path)?;

    let snapshots = if let Some(since_sha) = since {
        // Get all snapshots first to find the starting point
        let all_snapshots = but_api::undo::list_snapshots(project.id, 1000, None, None)?; // Get a large number to find the SHA
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
        but_api::undo::list_snapshots(project.id, 20, None, None)?
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
            let time_string = chrono::DateTime::from_timestamp(snapshot.created_at.seconds(), 0)
                .ok_or(anyhow::anyhow!("Could not parse timestamp"))?
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();

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
    repo_path: &Path,
    _json: bool,
    oplog_sha: &str,
) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path)?;

    // Parse the oplog SHA (support partial SHAs)
    let commit_sha_string = if oplog_sha.len() >= 7 {
        // Try to find a snapshot that starts with this SHA
        let snapshots = but_api::undo::list_snapshots(project.id, 100, None, None)?;

        let matching_snapshot = snapshots
            .iter()
            .find(|snapshot| snapshot.commit_id.to_string().starts_with(oplog_sha))
            .ok_or_else(|| anyhow::anyhow!("No oplog snapshot found matching '{}'", oplog_sha))?;

        matching_snapshot.commit_id.to_string()
    } else {
        anyhow::bail!("Oplog SHA must be at least 7 characters long");
    };

    // Get information about the target snapshot
    let snapshots = but_api::undo::list_snapshots(project.id, 100, None, None)?;
    let target_snapshot = snapshots
        .iter()
        .find(|snapshot| snapshot.commit_id.to_string() == commit_sha_string)
        .ok_or_else(|| anyhow::anyhow!("Snapshot {} not found in oplog", commit_sha_string))?;

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = chrono::DateTime::from_timestamp(target_snapshot.created_at.seconds(), 0)
        .ok_or(anyhow::anyhow!("Could not parse timestamp"))?
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    println!("{}", "Restoring to oplog snapshot...".blue().bold());
    println!(
        "  Target: {} ({})",
        target_operation.green(),
        target_time.dimmed()
    );
    println!("  Snapshot: {}", commit_sha_string[..7].cyan().underline());

    // Confirm the restoration (safety check)
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

    // Restore to the target snapshot using the but-api crate
    but_api::undo::restore_snapshot(project.id, commit_sha_string)?;

    println!("\n{} Restore completed successfully!", "✓".green().bold(),);

    println!(
        "{}",
        "\nWorkspace has been restored to the selected snapshot.".green()
    );

    Ok(())
}
