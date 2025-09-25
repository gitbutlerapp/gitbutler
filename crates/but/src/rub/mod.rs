use std::path::Path;

use anyhow::bail;
use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
mod amend;
mod assign;
mod commits;
mod move_commit;
mod squash;
mod undo;
use crate::id::CliId;
pub(crate) use assign::branch_name_to_stack_id;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

pub(crate) fn handle(
    repo_path: &Path,
    _json: bool,
    source_str: &str,
    target_str: &str,
) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let (sources, target) = ids(ctx, source_str, target_str)?;

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
            (CliId::Unassigned, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Unassigned, CliId::Unassigned) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Unassigned, CliId::Commit { oid }) => {
                create_snapshot(ctx, &project, OperationKind::AmendCommit);
                amend::assignments_to_commit(ctx, None, oid)?;
            }
            (CliId::Unassigned, CliId::Branch { name: to }) => {
                create_snapshot(ctx, &project, OperationKind::MoveHunk);
                assign::assign_all(ctx, None, Some(to))?;
            }
            (CliId::Commit { .. }, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Commit { oid }, CliId::Unassigned) => {
                create_snapshot(ctx, &project, OperationKind::UndoCommit);
                undo::commit(ctx, oid)?;
            }
            (CliId::Commit { oid: source }, CliId::Commit { oid: destination }) => {
                create_snapshot(ctx, &project, OperationKind::SquashCommit);
                squash::commits(ctx, source, destination)?;
            }
            (CliId::Commit { oid }, CliId::Branch { name }) => {
                create_snapshot(ctx, &project, OperationKind::MoveCommit);
                move_commit::to_branch(ctx, oid, name)?;
            }
            (CliId::Branch { .. }, CliId::UncommittedFile { .. }) => {
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
            (CliId::UncommittedFile { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::CommittedFile { .. }, CliId::UncommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::CommittedFile { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::CommittedFile { path, commit_oid }, CliId::Branch { name }) => {
                create_snapshot(ctx, &project, OperationKind::FileChanges);
                commits::uncommit_file(ctx, path, *commit_oid, Some(name))?;
            }
            (CliId::CommittedFile { path, commit_oid }, CliId::Commit { oid }) => {
                create_snapshot(ctx, &project, OperationKind::FileChanges);
                commits::commited_file_to_another_commit(ctx, path, *commit_oid, *oid)?;
                todo!()
            }
            (CliId::CommittedFile { path, commit_oid }, CliId::Unassigned) => {
                create_snapshot(ctx, &project, OperationKind::FileChanges);
                commits::uncommit_file(ctx, path, *commit_oid, None)?;
            }
            (CliId::Branch { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Commit { .. }, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
            }
            (CliId::Unassigned, CliId::CommittedFile { .. }) => {
                bail!(makes_no_sense_error(&source, &target))
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

fn ids(
    ctx: &mut CommandContext,
    source: &str,
    target: &str,
) -> anyhow::Result<(Vec<CliId>, CliId)> {
    let sources = parse_sources(ctx, source)?;
    let target_result = crate::id::CliId::from_str(ctx, target)?;
    if target_result.len() != 1 {
        if target_result.is_empty() {
            return Err(anyhow::anyhow!(
                "Target '{}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.",
                target
            ));
        } else {
            let matches: Vec<String> = target_result
                .iter()
                .map(|id| match id {
                    CliId::Commit { oid } => {
                        format!("{} (commit {})", id, &oid.to_string()[..7])
                    }
                    CliId::Branch { name } => format!("{id} (branch '{name}')"),
                    _ => format!("{} ({})", id, id.kind()),
                })
                .collect();
            return Err(anyhow::anyhow!(
                "Target '{}' is ambiguous. Matches: {}. Try using more characters, a longer SHA, or the full branch name to disambiguate.",
                target,
                matches.join(", ")
            ));
        }
    }
    Ok((sources, target_result[0].clone()))
}

fn parse_sources(ctx: &mut CommandContext, source: &str) -> anyhow::Result<Vec<CliId>> {
    // Check if it's a range (contains '-')
    if source.contains('-') {
        parse_range(ctx, source)
    }
    // Check if it's a list (contains ',')
    else if source.contains(',') {
        parse_list(ctx, source)
    }
    // Single source
    else {
        let source_result = crate::id::CliId::from_str(ctx, source)?;
        if source_result.len() != 1 {
            if source_result.is_empty() {
                return Err(anyhow::anyhow!(
                    "Source '{}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.",
                    source
                ));
            } else {
                let matches: Vec<String> = source_result
                    .iter()
                    .map(|id| match id {
                        CliId::Commit { oid } => {
                            format!("{} (commit {})", id, &oid.to_string()[..7])
                        }
                        CliId::Branch { name } => format!("{id} (branch '{name}')"),
                        _ => format!("{} ({})", id, id.kind()),
                    })
                    .collect();
                return Err(anyhow::anyhow!(
                    "Source '{}' is ambiguous. Matches: {}. Try using more characters, a longer SHA, or the full branch name to disambiguate.",
                    source,
                    matches.join(", ")
                ));
            }
        }
        Ok(vec![source_result[0].clone()])
    }
}

fn parse_range(ctx: &mut CommandContext, source: &str) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = source.split('-').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Range format should be 'start-end', got '{}'",
            source
        ));
    }

    let start_str = parts[0];
    let end_str = parts[1];

    // Get the start and end IDs
    let start_matches = crate::id::CliId::from_str(ctx, start_str)?;
    let end_matches = crate::id::CliId::from_str(ctx, end_str)?;

    if start_matches.len() != 1 {
        return Err(anyhow::anyhow!(
            "Start of range '{}' must match exactly one item",
            start_str
        ));
    }
    if end_matches.len() != 1 {
        return Err(anyhow::anyhow!(
            "End of range '{}' must match exactly one item",
            end_str
        ));
    }

    let start_id = &start_matches[0];
    let end_id = &end_matches[0];

    // Get all files in display order (same order as shown in status)
    let all_files_in_order = get_all_files_in_display_order(ctx)?;

    // Find the positions of start and end in the ordered file list
    let start_pos = all_files_in_order.iter().position(|id| id == start_id);
    let end_pos = all_files_in_order.iter().position(|id| id == end_id);

    if let (Some(start_idx), Some(end_idx)) = (start_pos, end_pos) {
        if start_idx <= end_idx {
            return Ok(all_files_in_order[start_idx..=end_idx].to_vec());
        } else {
            return Ok(all_files_in_order[end_idx..=start_idx].to_vec());
        }
    }

    Err(anyhow::anyhow!(
        "Could not find range from '{}' to '{}' in the displayed file list",
        start_str,
        end_str
    ))
}
fn get_all_files_in_display_order(ctx: &mut CommandContext) -> anyhow::Result<Vec<CliId>> {
    use bstr::BString;
    use but_hunk_assignment::HunkAssignment;
    use std::collections::BTreeMap;

    let project = gitbutler_project::Project::from_path(&ctx.project().path)?;
    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(project.path.clone())?.changes;
    let (assignments, _) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;

    // Group assignments by file, same as status display logic
    let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
    for assignment in &assignments {
        by_file
            .entry(assignment.path_bytes.clone())
            .or_default()
            .push(assignment.clone());
    }

    let mut all_files = Vec::new();

    // First, get files assigned to branches (they appear first in status display)
    let stacks = crate::log::stacks(ctx)?;
    for stack in stacks {
        if let Some((_stack_id, details_result)) = stack
            .id
            .map(|id| (stack.id, crate::log::stack_details(ctx, id)))
            && let Ok(details) = details_result
        {
            for _branch in &details.branch_details {
                for assignments in by_file.values() {
                    for assignment in assignments {
                        if let Some(stack_id) = assignment.stack_id
                            && stack.id == Some(stack_id)
                        {
                            let file_id = CliId::file_from_assignment(assignment);
                            if !all_files.contains(&file_id) {
                                all_files.push(file_id);
                            }
                        }
                    }
                }
            }
        }
    }

    // Then add unassigned files (they appear last in status display)
    for assignments in by_file.values() {
        for assignment in assignments {
            if assignment.stack_id.is_none() {
                let file_id = CliId::file_from_assignment(assignment);
                if !all_files.contains(&file_id) {
                    all_files.push(file_id);
                }
            }
        }
    }

    Ok(all_files)
}

fn parse_list(ctx: &mut CommandContext, source: &str) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = source.split(',').collect();
    let mut result = Vec::new();

    for part in parts {
        let part = part.trim();
        let matches = crate::id::CliId::from_str(ctx, part)?;
        if matches.len() != 1 {
            if matches.is_empty() {
                return Err(anyhow::anyhow!(
                    "Item '{}' in list not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.",
                    part
                ));
            } else {
                return Err(anyhow::anyhow!(
                    "Item '{}' in list is ambiguous. Try using more characters to disambiguate.",
                    part
                ));
            }
        }
        result.push(matches[0].clone());
    }

    Ok(result)
}

fn create_snapshot(ctx: &mut CommandContext, project: &Project, operation: OperationKind) {
    let mut guard = project.exclusive_worktree_access();
    let _snapshot = ctx
        .create_snapshot(SnapshotDetails::new(operation), guard.write_permission())
        .ok(); // Ignore errors for snapshot creation
}
