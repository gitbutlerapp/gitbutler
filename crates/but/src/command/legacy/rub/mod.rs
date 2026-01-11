use anyhow::bail;
use bstr::BStr;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use colored::Colorize;
mod amend;
mod assign;
mod commits;
mod move_commit;
mod squash;
mod undo;
pub(crate) use assign::branch_name_to_stack_id;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

use crate::{CliId, IdMap, utils::OutputChannel};

pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
    target_str: &str,
) -> anyhow::Result<()> {
    let mut id_map = IdMap::new_from_context(ctx, None)?;
    id_map.add_committed_file_info_from_context(ctx)?;
    let (sources, target) = ids(ctx, &id_map, source_str, target_str)?;

    for source in sources {
        match (source, &target) {
            (CliId::Uncommitted(uncommitted_cli_id), CliId::Unassigned { .. }) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::unassign_uncommitted(ctx, uncommitted_cli_id, out)?;
            }
            (CliId::Uncommitted(uncommitted_cli_id), CliId::Commit { commit_id: oid, .. }) => {
                create_snapshot(ctx, OperationKind::AmendCommit);
                amend::uncommitted_to_commit(ctx, uncommitted_cli_id, oid, out)?;
            }
            (CliId::Uncommitted(uncommitted_cli_id), CliId::Branch { name, .. }) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_uncommitted_to_branch(ctx, uncommitted_cli_id, name, out)?;
            }
            (CliId::Unassigned { .. }, CliId::Commit { commit_id: oid, .. }) => {
                create_snapshot(ctx, OperationKind::AmendCommit);
                amend::assignments_to_commit(ctx, None, oid, out)?;
            }
            (CliId::Unassigned { .. }, CliId::Branch { name: to, .. }) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(ctx, None, Some(to), out)?;
            }
            (CliId::Commit { commit_id: oid, .. }, CliId::Unassigned { .. }) => {
                create_snapshot(ctx, OperationKind::UndoCommit);
                undo::commit(ctx, &oid, out)?;
            }
            (
                CliId::Commit {
                    commit_id: source, ..
                },
                CliId::Commit {
                    commit_id: destination,
                    ..
                },
            ) => {
                create_snapshot(ctx, OperationKind::SquashCommit);
                squash::commits(ctx, &source, destination, None, out)?;
            }
            (CliId::Commit { commit_id: oid, .. }, CliId::Branch { name, .. }) => {
                create_snapshot(ctx, OperationKind::MoveCommit);
                move_commit::to_branch(ctx, &oid, name, out)?;
            }
            (CliId::Branch { name: from, .. }, CliId::Unassigned { .. }) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(ctx, Some(&from), None, out)?;
            }
            (CliId::Branch { name, .. }, CliId::Commit { commit_id: oid, .. }) => {
                create_snapshot(ctx, OperationKind::AmendCommit);
                amend::assignments_to_commit(ctx, Some(&name), oid, out)?;
            }
            (CliId::Branch { name: from, .. }, CliId::Branch { name: to, .. }) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(ctx, Some(&from), Some(to), out)?;
            }
            (
                CliId::CommittedFile {
                    path,
                    commit_id: commit_oid,
                    ..
                },
                CliId::Branch { name, .. },
            ) => {
                create_snapshot(ctx, OperationKind::FileChanges);
                commits::uncommit_file(ctx, path.as_ref(), commit_oid, Some(name), out)?;
            }
            (
                CliId::CommittedFile {
                    path,
                    commit_id: commit_oid,
                    ..
                },
                CliId::Commit { commit_id: oid, .. },
            ) => {
                create_snapshot(ctx, OperationKind::FileChanges);
                commits::commited_file_to_another_commit(
                    ctx,
                    path.as_ref(),
                    commit_oid,
                    *oid,
                    out,
                )?;
            }
            (
                CliId::CommittedFile {
                    path,
                    commit_id: commit_oid,
                    ..
                },
                CliId::Unassigned { .. },
            ) => {
                create_snapshot(ctx, OperationKind::FileChanges);
                commits::uncommit_file(ctx, path.as_ref(), commit_oid, None, out)?;
            }
            (source, target) => {
                bail!(makes_no_sense_error(&source, target))
            }
        }
    }
    Ok(())
}

fn makes_no_sense_error(source: &CliId, target: &CliId) -> String {
    format!(
        "Operation doesn't make sense. Source {} is {} and target {} is {}.",
        source.to_short_string().blue().underline(),
        source.kind_for_humans().yellow(),
        target.to_short_string().blue().underline(),
        target.kind_for_humans().yellow()
    )
}

fn ids(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
    target: &str,
) -> anyhow::Result<(Vec<CliId>, CliId)> {
    let sources = parse_sources(ctx, id_map, source)?;
    let target_result = id_map.resolve_entity_to_ids(target)?;
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
                    CliId::Commit { commit_id: oid, .. } => {
                        format!(
                            "{} (commit {})",
                            id.to_short_string(),
                            &oid.to_string()[..7]
                        )
                    }
                    CliId::Branch { name, .. } => {
                        format!("{} (branch '{}')", id.to_short_string(), name)
                    }
                    _ => format!("{} ({})", id.to_short_string(), id.kind_for_humans()),
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

pub(crate) fn parse_sources(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
) -> anyhow::Result<Vec<CliId>> {
    // Check if it's a range (contains '-')
    if source.contains('-') {
        parse_range(ctx, id_map, source)
    }
    // Check if it's a list (contains ',')
    else if source.contains(',') {
        parse_list(id_map, source)
    }
    // Single source
    else {
        let source_result = id_map.resolve_entity_to_ids(source)?;
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
                        CliId::Commit { commit_id: oid, .. } => {
                            format!(
                                "{} (commit {})",
                                id.to_short_string(),
                                &oid.to_string()[..7]
                            )
                        }
                        CliId::Branch { name, .. } => {
                            format!("{} (branch '{}')", id.to_short_string(), name)
                        }
                        _ => format!("{} ({})", id.to_short_string(), id.kind_for_humans()),
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

fn parse_range(ctx: &mut Context, id_map: &IdMap, source: &str) -> anyhow::Result<Vec<CliId>> {
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
    let start_matches = id_map.resolve_entity_to_ids(start_str)?;
    let end_matches = id_map.resolve_entity_to_ids(end_str)?;

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
    let all_files_in_order = get_all_files_in_display_order(ctx, id_map)?;

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

fn get_all_files_in_display_order(ctx: &mut Context, id_map: &IdMap) -> anyhow::Result<Vec<CliId>> {
    // First, files assigned to branches (they appear first in status display),
    // then unassigned files (they appear last in status display)
    let stack_ids: Vec<StackId> = crate::legacy::commits::stacks(ctx)?
        .iter()
        .filter_map(|stack_entry| stack_entry.id)
        .collect();
    let mut positioned_files: Vec<(usize, &BStr, CliId)> = id_map
        .uncommitted_files
        .iter()
        .flat_map(|(short_id, uncommitted_file)| {
            let position = match uncommitted_file.stack_id() {
                Some(stack_id) => stack_ids.iter().position(|e| *e == stack_id)?,
                None => usize::MAX,
            };
            Some((
                position,
                uncommitted_file.path(),
                uncommitted_file.to_cli_id(short_id.clone()),
            ))
        })
        .collect();
    positioned_files.sort_by(|(a_pos, a_path, _), (b_pos, b_path, _)| {
        a_pos.cmp(b_pos).then_with(|| a_path.cmp(b_path))
    });

    Ok(positioned_files
        .into_iter()
        .map(|(_, _, cli_id)| cli_id)
        .collect())
}

fn parse_list(id_map: &IdMap, source: &str) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = source.split(',').collect();
    let mut result = Vec::new();

    for part in parts {
        let part = part.trim();
        let matches = id_map.resolve_entity_to_ids(part)?;
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

fn create_snapshot(ctx: &mut Context, operation: OperationKind) {
    let mut guard = ctx.exclusive_worktree_access();
    let _snapshot = ctx
        .create_snapshot(SnapshotDetails::new(operation), guard.write_permission())
        .ok(); // Ignore errors for snapshot creation
}

/// Handler for `but uncommit <source>` - runs `but rub <source> zz`
/// Validates that source is a commit or file-in-commit.
pub(crate) fn handle_uncommit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let sources = parse_sources(ctx, &id_map, source_str)?;

    // Validate that all sources are commits or committed files
    for source in &sources {
        match source {
            CliId::Commit { .. } | CliId::CommittedFile { .. } => {
                // Valid types for uncommit
            }
            _ => {
                bail!(
                    "Cannot uncommit {} - it is {}. Only commits and files-in-commits can be uncommitted.",
                    source.to_short_string().blue().underline(),
                    source.kind_for_humans().yellow()
                );
            }
        }
    }

    // Call the main rub handler with "zz" as target
    handle(ctx, out, source_str, "zz")
}

/// Handler for `but amend <file> <commit>` - runs `but rub <file> <commit>`
/// Validates that file is an uncommitted file/hunk and commit is a commit.
pub(crate) fn handle_amend(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_str: &str,
    commit_str: &str,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let files = parse_sources(ctx, &id_map, file_str)?;
    let commit_matches = id_map.resolve_entity_to_ids(commit_str)?;

    // Validate that all files are uncommitted
    for file in &files {
        match file {
            CliId::Uncommitted(_) => {
                // Valid type for amend
            }
            _ => {
                bail!(
                    "Cannot amend {} - it is {}. Only uncommitted files and hunks can be amended.",
                    file.to_short_string().blue().underline(),
                    file.kind_for_humans().yellow()
                );
            }
        }
    }

    // Validate that commit is a commit
    if commit_matches.len() != 1 {
        if commit_matches.is_empty() {
            bail!("Commit '{}' not found.", commit_str);
        } else {
            bail!("Commit '{}' is ambiguous.", commit_str);
        }
    }

    match &commit_matches[0] {
        CliId::Commit { .. } => {
            // Valid type for target
        }
        other => {
            bail!(
                "Cannot amend into {} - it is {}. Target must be a commit.",
                other.to_short_string().blue().underline(),
                other.kind_for_humans().yellow()
            );
        }
    }

    // Call the main rub handler
    handle(ctx, out, file_str, commit_str)
}

/// Handler for `but stage <file_or_hunk> <branch>` - runs `but rub <file_or_hunk> <branch>`
/// Validates that file_or_hunk is uncommitted and branch is a branch.
pub(crate) fn handle_stage(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_or_hunk_str: &str,
    branch_str: &str,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let files = parse_sources(ctx, &id_map, file_or_hunk_str)?;
    let branch_matches = id_map.resolve_entity_to_ids(branch_str)?;

    // Validate that all files are uncommitted
    for file in &files {
        match file {
            CliId::Uncommitted(_) => {
                // Valid type for stage
            }
            _ => {
                bail!(
                    "Cannot stage {} - it is {}. Only uncommitted files and hunks can be staged.",
                    file.to_short_string().blue().underline(),
                    file.kind_for_humans().yellow()
                );
            }
        }
    }

    // Validate that branch is a branch
    if branch_matches.len() != 1 {
        if branch_matches.is_empty() {
            bail!("Branch '{}' not found.", branch_str);
        } else {
            bail!("Branch '{}' is ambiguous.", branch_str);
        }
    }

    match &branch_matches[0] {
        CliId::Branch { .. } => {
            // Valid type for target
        }
        other => {
            bail!(
                "Cannot stage to {} - it is {}. Target must be a branch.",
                other.to_short_string().blue().underline(),
                other.kind_for_humans().yellow()
            );
        }
    }

    // Call the main rub handler
    handle(ctx, out, file_or_hunk_str, branch_str)
}

/// Handler for `but unstage <file_or_hunk> [branch]` - runs `but rub <file_or_hunk> zz`
/// Validates that file_or_hunk is uncommitted. Optionally validates it's assigned to the specified branch.
pub(crate) fn handle_unstage(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_or_hunk_str: &str,
    branch_str: Option<&str>,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let files = parse_sources(ctx, &id_map, file_or_hunk_str)?;

    // Validate that all files are uncommitted
    for file in &files {
        match file {
            CliId::Uncommitted(_) => {
                // Valid type for unstage
            }
            _ => {
                bail!(
                    "Cannot unstage {} - it is {}. Only uncommitted files and hunks can be unstaged.",
                    file.to_short_string().blue().underline(),
                    file.kind_for_humans().yellow()
                );
            }
        }
    }

    // If a branch is specified, validate it exists (but we don't strictly require the file to be assigned to it)
    if let Some(branch_name) = branch_str {
        let branch_matches = id_map.resolve_entity_to_ids(branch_name)?;
        if branch_matches.is_empty() {
            bail!("Branch '{}' not found.", branch_name);
        }
        if branch_matches.len() > 1 {
            bail!("Branch '{}' is ambiguous.", branch_name);
        }
        match &branch_matches[0] {
            CliId::Branch { .. } => {
                // Valid - branch exists
            }
            other => {
                bail!(
                    "Cannot unstage from {} - it is {}. Target must be a branch.",
                    other.to_short_string().blue().underline(),
                    other.kind_for_humans().yellow()
                );
            }
        }
    }

    // Call the main rub handler with "zz" as target to unassign
    handle(ctx, out, file_or_hunk_str, "zz")
}

/// Handler for `but squash <commit1> <commit2>` - runs `but rub <commit1> <commit2>`
/// Validates that both arguments are commits.
pub(crate) fn handle_squash(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit1_str: &str,
    commit2_str: &str,
    drop_message: bool,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let commit1_matches = id_map.resolve_entity_to_ids(commit1_str)?;
    let commit2_matches = id_map.resolve_entity_to_ids(commit2_str)?;

    // Validate that commit1 is a commit
    if commit1_matches.len() != 1 {
        if commit1_matches.is_empty() {
            bail!("First commit '{}' not found.", commit1_str);
        } else {
            bail!("First commit '{}' is ambiguous.", commit1_str);
        }
    }

    let commit1_oid = match &commit1_matches[0] {
        CliId::Commit { commit_id, .. } => *commit_id,
        other => {
            bail!(
                "Cannot squash {} - it is {}. First argument must be a commit.",
                other.to_short_string().blue().underline(),
                other.kind_for_humans().yellow()
            );
        }
    };

    // Validate that commit2 is a commit
    if commit2_matches.len() != 1 {
        if commit2_matches.is_empty() {
            bail!("Second commit '{}' not found.", commit2_str);
        } else {
            bail!("Second commit '{}' is ambiguous.", commit2_str);
        }
    }

    let commit2_oid = match &commit2_matches[0] {
        CliId::Commit { commit_id, .. } => *commit_id,
        other => {
            bail!(
                "Cannot squash into {} - it is {}. Second argument must be a commit.",
                other.to_short_string().blue().underline(),
                other.kind_for_humans().yellow()
            );
        }
    };

    // If drop_message is true, get the message from commit2
    let custom_message = if drop_message {
        let repo = ctx.repo.get()?;
        let commit2 = repo.find_commit(commit2_oid)?;
        let message_ref = commit2.message()?;
        let full_message = if let Some(body) = message_ref.body {
            format!("{}\n\n{}", message_ref.title, body)
        } else {
            message_ref.title.to_string()
        };
        Some(full_message)
    } else {
        None
    };

    // Call the squash::commits function directly with the custom message
    squash::commits(
        ctx,
        &commit1_oid,
        &commit2_oid,
        custom_message.as_deref(),
        out,
    )
}
