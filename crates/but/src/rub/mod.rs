use std::path::Path;

use anyhow::bail;
use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_oplog::{OplogExt, entry::{OperationKind, SnapshotDetails}};
mod amend;
mod assign;
mod move_commit;
mod squash;
mod undo;
mod uncommit;

use crate::id::CliId;

pub(crate) fn handle(
    repo_path: &Path,
    _json: bool,
    source_str: &str,
    target_str: &str,
) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let (sources, target) = ids(ctx, source_str, target_str)?;

    // Process each source with the target
    for source in sources {
        match (&source, &target) {
            (CliId::UncommittedFile { .. }, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::UncommittedFile { path, .. }, CliId::Unassigned) => {
                create_snapshot(ctx, &project, OperationKind::MoveHunk);
                assign::unassign_file(ctx, path)?;
            }
            (CliId::UncommittedFile { path, assignment }, CliId::Commit { oid }) => {
                create_snapshot(ctx, &project, OperationKind::AmendCommit);
                amend::file_to_commit(ctx, path, *assignment, oid)?;
            }
            (CliId::UncommittedFile { path, .. }, CliId::Branch { name }) => {
                create_snapshot(ctx, &project, OperationKind::MoveHunk);
                assign::assign_file_to_branch(ctx, path, name)?;
            }
            (CliId::UncommittedFile { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            // CommittedFile operations
            (CliId::CommittedFile { .. }, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::CommittedFile { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::CommittedFile { path, commit_oid }, CliId::Unassigned) => {
                create_snapshot(ctx, &project, OperationKind::FileChanges);
                uncommit::file_from_commit(ctx, path, commit_oid)?;
            }
            (CliId::CommittedFile { .. }, CliId::Branch { .. }) => {
                // Extract file from commit to branch - for now, not implemented  
                bail!("Extracting files from commits is not yet supported. Use git commands to extract file changes.")
            }
            (CliId::CommittedFile { .. }, CliId::Commit { .. }) => {
                // Move file from one commit to another - for now, not implemented
                bail!("Moving files between commits is not yet supported. Use git commands to modify commits.")
            }
            (CliId::Unassigned, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Unassigned, CliId::Unassigned) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Unassigned, CliId::Commit { oid }) => {
                create_snapshot(ctx, &project, OperationKind::AmendCommit);
                amend::assignments_to_commit(ctx, None, oid)?;
            },
            (CliId::Unassigned, CliId::Branch { name: to }) => {
                create_snapshot(ctx, &project, OperationKind::MoveHunk);
                assign::assign_all(ctx, None, Some(to))?;
            },
            (CliId::Unassigned, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Commit { .. }, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Commit { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Commit { oid }, CliId::Unassigned) => {
                create_snapshot(ctx, &project, OperationKind::UndoCommit);
                undo::commit(ctx, oid)?;
            },
            (CliId::Commit { oid: source_oid }, CliId::Commit { oid: destination }) => {
                create_snapshot(ctx, &project, OperationKind::SquashCommit);
                squash::commits(ctx, source_oid, destination)?;
            }
            (CliId::Commit { oid }, CliId::Branch { name }) => {
                create_snapshot(ctx, &project, OperationKind::MoveCommit);
                move_commit::to_branch(ctx, oid, name)?;
            },
            (CliId::Branch { .. }, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Branch { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Branch { name: from }, CliId::Unassigned) => {
                create_snapshot(ctx, &project, OperationKind::MoveHunk);
                assign::assign_all(ctx, Some(from), None)?;
            }
            (CliId::Branch { name }, CliId::Commit { oid }) => {
                create_snapshot(ctx, &project, OperationKind::AmendCommit);
                amend::assignments_to_commit(ctx, Some(name), oid)?;
            }
            (CliId::Branch { name: from }, CliId::Branch { name: to }) => {
                create_snapshot(ctx, &project, OperationKind::MoveHunk);
                assign::assign_all(ctx, Some(from), Some(to))?;
            }
        }
    }

    Ok(())
}

fn makes_no_sense_error(source: &CliId, target: &CliId) -> String {
    format!(
        "Operation doesn't make sense. Source {} is {} and target {} is {}.",
        source.to_string().blue().underline(),
        source.kind().yellow(),
        target.to_string().blue().underline(),
        target.kind().yellow()
    )
}

fn ids(ctx: &mut CommandContext, source: &str, target: &str) -> anyhow::Result<(CliId, CliId)> {
    let source_result = crate::id::CliId::from_str(ctx, source)?;
    if source_result.len() != 1 {
        if source_result.is_empty() {
            return Err(anyhow::anyhow!(
                "Source '{}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.", 
                source
            ));
        } else {
            let matches: Vec<String> = source_result.iter().map(|id| {
                match id {
                    CliId::Commit { oid } => format!("{} (commit {})", id.to_string(), &oid.to_string()[..7]),
                    CliId::Branch { name } => format!("{} (branch '{}')", id.to_string(), name),
                    _ => format!("{} ({})", id.to_string(), id.kind())
                }
            }).collect();
            return Err(anyhow::anyhow!(
                "Source '{}' is ambiguous. Matches: {}. Try using more characters, a longer SHA, or the full branch name to disambiguate.",
                source,
                matches.join(", ")
            ));
        }
    }
    let target_result = crate::id::CliId::from_str(ctx, target)?;
    if target_result.len() != 1 {
        if target_result.is_empty() {
            return Err(anyhow::anyhow!(
                "Target '{}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.", 
                target
            ));
        } else {
            let matches: Vec<String> = target_result.iter().map(|id| {
                match id {
                    CliId::Commit { oid } => format!("{} (commit {})", id.to_string(), &oid.to_string()[..7]),
                    CliId::Branch { name } => format!("{} (branch '{}')", id.to_string(), name),
                    _ => format!("{} ({})", id.to_string(), id.kind())
                }
            }).collect();
            return Err(anyhow::anyhow!(
                "Target '{}' is ambiguous. Matches: {}. Try using more characters, a longer SHA, or the full branch name to disambiguate.",
                target,
                matches.join(", ")
            ));
        }
    }
    Ok((source_result[0].clone(), target_result[0].clone()))
}

fn create_snapshot(ctx: &mut CommandContext, project: &Project, operation: OperationKind) {
    let mut guard = project.exclusive_worktree_access();
    let _snapshot = ctx
        .create_snapshot(
            SnapshotDetails::new(operation),
            guard.write_permission(),
        )
        .ok(); // Ignore errors for snapshot creation
}
