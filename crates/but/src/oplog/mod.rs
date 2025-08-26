use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{OplogExt, entry::OperationKind};
use gitbutler_project::Project;
use std::path::Path;

pub(crate) fn show_oplog(repo_path: &Path, _json: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let snapshots = ctx.list_snapshots(20, None, vec![])?;

    if snapshots.is_empty() {
        println!("No operations found in history.");
        return Ok(());
    }

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
            "{} {} {} {}",
            commit_id,
            time_string.dimmed(),
            format!("[{}]", operation_colored),
            title
        );
    }

    Ok(())
}
