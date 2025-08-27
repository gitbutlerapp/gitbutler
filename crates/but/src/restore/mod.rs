use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::OplogExt;
use gitbutler_project::Project;
use std::path::Path;

pub(crate) fn restore_to_oplog(repo_path: &Path, _json: bool, oplog_sha: &str) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    // Parse the oplog SHA (support partial SHAs)
    let commit_id = if oplog_sha.len() >= 7 {
        // Try to find a snapshot that starts with this SHA
        let snapshots = ctx.list_snapshots(100, None, vec![])?;
        
        let matching_snapshot = snapshots
            .iter()
            .find(|snapshot| snapshot.commit_id.to_string().starts_with(oplog_sha))
            .ok_or_else(|| anyhow::anyhow!("No oplog snapshot found matching '{}'", oplog_sha))?;

        matching_snapshot.commit_id
    } else {
        anyhow::bail!("Oplog SHA must be at least 7 characters long");
    };

    // Get information about the target snapshot
    let snapshots = ctx.list_snapshots(100, None, vec![])?;
    let target_snapshot = snapshots
        .iter()
        .find(|snapshot| snapshot.commit_id == commit_id)
        .ok_or_else(|| anyhow::anyhow!("Snapshot {} not found in oplog", commit_id))?;

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
    println!(
        "  Snapshot: {}",
        commit_id.to_string()[..7].cyan().underline()
    );

    // Confirm the restoration (safety check)
    println!("\n{}", "⚠️  This will overwrite your current workspace state.".yellow().bold());
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

    // Get exclusive access to the worktree
    let mut guard = project.exclusive_worktree_access();

    // Restore to the target snapshot
    let restore_commit_id = ctx.restore_snapshot(commit_id, guard.write_permission())?;

    let restore_commit_short = format!(
        "{}{}",
        &restore_commit_id.to_string()[..7].blue().underline(),
        &restore_commit_id.to_string()[7..12].blue().dimmed()
    );

    println!(
        "\n{} Restore completed successfully! New snapshot: {}",
        "✓".green().bold(),
        restore_commit_short
    );

    println!("{}", "\nWorkspace has been restored to the selected snapshot.".green());

    Ok(())
}