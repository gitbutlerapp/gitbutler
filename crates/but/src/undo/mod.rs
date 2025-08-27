use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{OplogExt, entry::OperationKind};
use gitbutler_project::Project;
use std::path::Path;

pub(crate) fn undo_last_operation(repo_path: &Path, _json: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    // Get more snapshots to account for restore operations
    let snapshots = ctx.list_snapshots(10, None, vec![])?;

    if snapshots.is_empty() {
        println!("{}", "No previous operations to undo.".yellow());
        return Ok(());
    }

    // Find the target snapshot by skipping restore operations
    // We want to undo the last non-restore operation
    let mut target_snapshot_index = None;
    
    for (i, snapshot) in snapshots.iter().enumerate() {
        let is_restore = snapshot
            .details
            .as_ref()
            .map(|d| d.operation == OperationKind::RestoreFromSnapshot)
            .unwrap_or(false);
        
        if i == 0 {
            // Skip the current snapshot
            continue;
        } else if !is_restore {
            // Found the first non-restore snapshot after current
            target_snapshot_index = Some(i);
            break;
        }
    }

    let target_snapshot = match target_snapshot_index {
        Some(idx) => &snapshots[idx],
        None => {
            println!("{}", "No previous operations to undo.".yellow());
            return Ok(());
        }
    };

    let current_snapshot = &snapshots[0];

    let current_operation = current_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = chrono::DateTime::from_timestamp(target_snapshot.created_at.seconds(), 0)
        .ok_or(anyhow::anyhow!("Could not parse timestamp"))?
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    println!("{}", "Undoing operation...".blue().bold());
    println!("  Current: {}", current_operation.yellow());
    println!(
        "  Reverting to: {} ({})",
        target_operation.green(),
        target_time.dimmed()
    );

    // Get exclusive access to the worktree
    let mut guard = project.exclusive_worktree_access();

    // Restore to the previous snapshot
    let restore_commit_id =
        ctx.restore_snapshot(target_snapshot.commit_id, guard.write_permission())?;

    let restore_commit_short = format!(
        "{}{}",
        &restore_commit_id.to_string()[..7].blue().underline(),
        &restore_commit_id.to_string()[7..12].blue().dimmed()
    );

    println!(
        "{} Undo completed successfully! New snapshot: {}",
        "✓".green().bold(),
        restore_commit_short
    );

    Ok(())
}
