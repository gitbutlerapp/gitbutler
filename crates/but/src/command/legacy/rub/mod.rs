use anyhow::bail;
use bstr::BStr;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use colored::Colorize;
mod amend;
mod assign;
mod commits;
pub(crate) mod r#move;
mod move_commit;
pub(crate) mod squash;
mod undo;
pub(crate) use assign::branch_name_to_stack_id;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use nonempty::NonEmpty;

use crate::{
    CliId, IdMap,
    id::parser::{parse_sources_with_disambiguation, prompt_for_disambiguation},
    utils::OutputChannel,
};

/// Serialize a [`gitbutler_branch_actions::MoveCommitIllegalAction`] to a structured JSON value.
///
/// Shared between `move.rs` and `move_commit.rs` to avoid duplicated match arms.
pub(crate) fn illegal_move_to_json(
    action: &gitbutler_branch_actions::MoveCommitIllegalAction,
) -> serde_json::Value {
    let (reason, deps) = match action {
        gitbutler_branch_actions::MoveCommitIllegalAction::DependsOnCommits(deps) => {
            ("depends_on_commits", Some(deps.clone()))
        }
        gitbutler_branch_actions::MoveCommitIllegalAction::HasDependentChanges(deps) => {
            ("has_dependent_changes", Some(deps.clone()))
        }
        gitbutler_branch_actions::MoveCommitIllegalAction::HasDependentUncommittedChanges => {
            ("has_dependent_uncommitted_changes", None)
        }
    };
    serde_json::json!({
        "ok": false,
        "error": "illegal_move",
        "reason": reason,
        "dependencies": deps,
    })
}

/// A description of a set of hunks.
type Description = String;

/// Represents the operation to perform for a given source and target combination.
/// This enum serves as the single source of truth for valid rub operations.
#[derive(Debug)]
pub(crate) enum RubOperation<'a> {
    UnassignUncommitted(
        NonEmpty<&'a but_hunk_assignment::HunkAssignment>,
        Description,
    ),
    UncommittedToCommit(
        NonEmpty<&'a but_hunk_assignment::HunkAssignment>,
        Description,
        &'a gix::ObjectId,
    ),
    UncommittedToBranch(
        NonEmpty<&'a but_hunk_assignment::HunkAssignment>,
        Description,
        &'a str,
    ),
    UncommittedToStack(
        NonEmpty<&'a but_hunk_assignment::HunkAssignment>,
        Description,
        StackId,
    ),
    StackToUnassigned(StackId),
    StackToStack {
        from: StackId,
        to: StackId,
    },
    StackToBranch {
        from: StackId,
        to: &'a str,
    },
    UnassignedToCommit(&'a gix::ObjectId),
    UnassignedToBranch(&'a str),
    UnassignedToStack(StackId),
    UndoCommit(&'a gix::ObjectId),
    SquashCommits {
        source: &'a gix::ObjectId,
        destination: &'a gix::ObjectId,
    },
    MoveCommitToBranch(&'a gix::ObjectId, &'a str),
    BranchToUnassigned(&'a str),
    BranchToStack {
        from: &'a str,
        to: StackId,
    },
    BranchToCommit(&'a str, &'a gix::ObjectId),
    BranchToBranch {
        from: &'a str,
        to: &'a str,
    },
    CommittedFileToBranch(&'a BStr, &'a gix::ObjectId, &'a str),
    CommittedFileToCommit(&'a BStr, &'a gix::ObjectId, &'a gix::ObjectId),
    CommittedFileToUnassigned(&'a BStr, &'a gix::ObjectId),
}

impl<'a> RubOperation<'a> {
    /// Executes this operation, creating snapshots and performing the necessary actions.
    fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        match self {
            RubOperation::UnassignUncommitted(hunk_assignments, description) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::unassign_uncommitted(ctx, hunk_assignments, description, out)
            }
            RubOperation::UncommittedToCommit(hunk_assignments, description, oid) => {
                create_snapshot(ctx, OperationKind::AmendCommit);
                amend::uncommitted_to_commit(ctx, hunk_assignments, description, oid, out)
            }
            RubOperation::UncommittedToBranch(hunk_assignments, description, name) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_uncommitted_to_branch(ctx, hunk_assignments, description, name, out)
            }
            RubOperation::UncommittedToStack(hunk_assignments, description, stack_id) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_uncommitted_to_stack(
                    ctx,
                    hunk_assignments,
                    description,
                    &stack_id,
                    out,
                )
            }
            RubOperation::StackToUnassigned(stack_id) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(ctx, Some(assign::AssignTarget::Stack(&stack_id)), None, out)
            }
            RubOperation::StackToStack { from, to } => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(
                    ctx,
                    Some(assign::AssignTarget::Stack(&from)),
                    Some(assign::AssignTarget::Stack(&to)),
                    out,
                )
            }
            RubOperation::StackToBranch { from, to } => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(
                    ctx,
                    Some(assign::AssignTarget::Stack(&from)),
                    Some(assign::AssignTarget::Branch(to)),
                    out,
                )
            }
            RubOperation::UnassignedToCommit(oid) => {
                create_snapshot(ctx, OperationKind::AmendCommit);
                amend::assignments_to_commit(ctx, None, oid, out)
            }
            RubOperation::UnassignedToBranch(to) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(ctx, None, Some(assign::AssignTarget::Branch(to)), out)
            }
            RubOperation::UnassignedToStack(to) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(ctx, None, Some(assign::AssignTarget::Stack(&to)), out)
            }
            RubOperation::UndoCommit(oid) => {
                create_snapshot(ctx, OperationKind::UndoCommit);
                undo::commit(ctx, oid, out)
            }
            RubOperation::SquashCommits {
                source,
                destination,
            } => {
                create_snapshot(ctx, OperationKind::SquashCommit);
                squash::commits(ctx, source, destination, None, out)
            }
            RubOperation::MoveCommitToBranch(oid, name) => {
                create_snapshot(ctx, OperationKind::MoveCommit);
                move_commit::to_branch(ctx, oid, name, out)
            }
            RubOperation::BranchToUnassigned(from) => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(ctx, Some(assign::AssignTarget::Branch(from)), None, out)
            }
            RubOperation::BranchToStack { from, to } => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(
                    ctx,
                    Some(assign::AssignTarget::Branch(from)),
                    Some(assign::AssignTarget::Stack(&to)),
                    out,
                )
            }
            RubOperation::BranchToCommit(name, oid) => {
                create_snapshot(ctx, OperationKind::AmendCommit);
                amend::assignments_to_commit(ctx, Some(name), oid, out)
            }
            RubOperation::BranchToBranch { from, to } => {
                create_snapshot(ctx, OperationKind::MoveHunk);
                assign::assign_all(
                    ctx,
                    Some(assign::AssignTarget::Branch(from)),
                    Some(assign::AssignTarget::Branch(to)),
                    out,
                )
            }
            RubOperation::CommittedFileToBranch(path, commit_oid, name) => {
                create_snapshot(ctx, OperationKind::FileChanges);
                commits::uncommit_file(ctx, path, *commit_oid, Some(name), out)
            }
            RubOperation::CommittedFileToCommit(path, commit_oid, oid) => {
                create_snapshot(ctx, OperationKind::FileChanges);
                commits::commited_file_to_another_commit(ctx, path, *commit_oid, *oid, out)
            }
            RubOperation::CommittedFileToUnassigned(path, commit_oid) => {
                create_snapshot(ctx, OperationKind::FileChanges);
                commits::uncommit_file(ctx, path, *commit_oid, None, out)
            }
        }
    }
}

/// Determines the operation to perform for a given source and target combination.
/// Returns `Some(operation)` if the combination is valid, `None` otherwise.
///
/// This function is the single source of truth for what operations are valid.
/// Both `handle()` and disambiguation logic use this function.
#[allow(private_interfaces)]
pub(crate) fn route_operation<'a>(
    source: &'a CliId,
    target: &'a CliId,
) -> Option<RubOperation<'a>> {
    use CliId::*;

    match (source, target) {
        // Uncommitted -> *
        (Uncommitted(uncommitted), Unassigned { .. }) => {
            let hunk_assignments = uncommitted.hunk_assignments.as_ref();
            let description = uncommitted.describe();
            Some(RubOperation::UnassignUncommitted(
                hunk_assignments,
                description,
            ))
        }
        (Uncommitted(uncommitted), Commit { commit_id, .. }) => {
            let hunk_assignments = uncommitted.hunk_assignments.as_ref();
            let description = uncommitted.describe();
            Some(RubOperation::UncommittedToCommit(
                hunk_assignments,
                description,
                commit_id,
            ))
        }
        (Uncommitted(uncommitted), Branch { name, .. }) => {
            let hunk_assignments = uncommitted.hunk_assignments.as_ref();
            let description = uncommitted.describe();
            Some(RubOperation::UncommittedToBranch(
                hunk_assignments,
                description,
                name,
            ))
        }
        (Uncommitted(uncommitted), Stack { stack_id, .. }) => {
            let hunk_assignments = uncommitted.hunk_assignments.as_ref();
            let description = uncommitted.describe();
            Some(RubOperation::UncommittedToStack(
                hunk_assignments,
                description,
                *stack_id,
            ))
        }
        // Uncommitted path prefix -> *
        (
            PathPrefix {
                hunk_assignments, ..
            },
            Unassigned { .. },
        ) => {
            let hunk_assignments = hunk_assignments
                .as_ref()
                .map(|(_, hunk_assignment)| hunk_assignment);
            Some(RubOperation::UnassignUncommitted(
                hunk_assignments,
                "hunk(s)".to_string(),
            ))
        }
        (
            PathPrefix {
                hunk_assignments, ..
            },
            Commit { commit_id, .. },
        ) => {
            let hunk_assignments = hunk_assignments
                .as_ref()
                .map(|(_, hunk_assignment)| hunk_assignment);
            Some(RubOperation::UncommittedToCommit(
                hunk_assignments,
                "hunk(s)".to_string(),
                commit_id,
            ))
        }
        (
            PathPrefix {
                hunk_assignments, ..
            },
            Branch { name, .. },
        ) => {
            let hunk_assignments = hunk_assignments
                .as_ref()
                .map(|(_, hunk_assignment)| hunk_assignment);
            Some(RubOperation::UncommittedToBranch(
                hunk_assignments,
                "hunk(s)".to_string(),
                name,
            ))
        }
        (
            PathPrefix {
                hunk_assignments, ..
            },
            Stack { stack_id, .. },
        ) => {
            let hunk_assignments = hunk_assignments
                .as_ref()
                .map(|(_, hunk_assignment)| hunk_assignment);
            Some(RubOperation::UncommittedToStack(
                hunk_assignments,
                "hunk(s)".to_string(),
                *stack_id,
            ))
        }
        // Stack -> *
        (Stack { stack_id, .. }, Unassigned { .. }) => {
            Some(RubOperation::StackToUnassigned(*stack_id))
        }
        (Stack { stack_id: from, .. }, Stack { stack_id: to, .. }) => {
            Some(RubOperation::StackToStack {
                from: *from,
                to: *to,
            })
        }
        (Stack { stack_id: from, .. }, Branch { name: to, .. }) => {
            Some(RubOperation::StackToBranch { from: *from, to })
        }
        // Unassigned -> *
        (Unassigned { .. }, Commit { commit_id, .. }) => {
            Some(RubOperation::UnassignedToCommit(commit_id))
        }
        (Unassigned { .. }, Branch { name, .. }) => Some(RubOperation::UnassignedToBranch(name)),
        (Unassigned { .. }, Stack { stack_id, .. }) => {
            Some(RubOperation::UnassignedToStack(*stack_id))
        }
        // Commit -> *
        (Commit { commit_id, .. }, Unassigned { .. }) => Some(RubOperation::UndoCommit(commit_id)),
        (
            Commit {
                commit_id: source, ..
            },
            Commit {
                commit_id: destination,
                ..
            },
        ) => Some(RubOperation::SquashCommits {
            source,
            destination,
        }),
        (Commit { commit_id, .. }, Branch { name, .. }) => {
            Some(RubOperation::MoveCommitToBranch(commit_id, name))
        }
        // Branch -> *
        (Branch { name, .. }, Unassigned { .. }) => Some(RubOperation::BranchToUnassigned(name)),
        (Branch { name: from, .. }, Stack { stack_id, .. }) => Some(RubOperation::BranchToStack {
            from,
            to: *stack_id,
        }),
        (Branch { name, .. }, Commit { commit_id, .. }) => {
            Some(RubOperation::BranchToCommit(name, commit_id))
        }
        (Branch { name: from, .. }, Branch { name: to, .. }) => {
            Some(RubOperation::BranchToBranch { from, to })
        }
        // CommittedFile -> *
        (
            CommittedFile {
                path, commit_id, ..
            },
            Branch { name, .. },
        ) => Some(RubOperation::CommittedFileToBranch(
            path.as_ref(),
            commit_id,
            name,
        )),
        (
            CommittedFile {
                path,
                commit_id: source,
                ..
            },
            Commit {
                commit_id: target, ..
            },
        ) => Some(RubOperation::CommittedFileToCommit(
            path.as_ref(),
            source,
            target,
        )),
        (
            CommittedFile {
                path, commit_id, ..
            },
            Unassigned { .. },
        ) => Some(RubOperation::CommittedFileToUnassigned(
            path.as_ref(),
            commit_id,
        )),
        // All other combinations are invalid
        _ => None,
    }
}

pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
    target_str: &str,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let (sources, target) = ids(ctx, &id_map, source_str, target_str, out)?;

    for source in sources {
        let Some(operation) = route_operation(&source, &target) else {
            bail!(makes_no_sense_error(&source, &target))
        };

        operation.execute(ctx, out)?;
    }
    Ok(())
}

fn makes_no_sense_error(source: &CliId, target: &CliId) -> String {
    format!(
        "Operation doesn't make sense. Source {} is {} and target {} is {}.",
        source.to_short_string().blue().bold(),
        source.kind_for_humans().yellow(),
        target.to_short_string().blue().bold(),
        target.kind_for_humans().yellow()
    )
}

fn ids(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
    target: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<(Vec<CliId>, CliId)> {
    let sources = parse_sources_with_disambiguation(ctx, id_map, source, out)?;
    let target_result = id_map.parse_using_context(target, ctx)?;

    if target_result.is_empty() {
        return Err(anyhow::anyhow!(
            "Target '{target}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
        ));
    }

    if target_result.len() == 1 {
        return Ok((sources, target_result[0].clone()));
    }

    // Target is ambiguous - filter by checking validity with ALL sources
    // A target is only valid if it works with every source in the list
    let valid_targets: Vec<CliId> = target_result
        .into_iter()
        .filter(|target_candidate| {
            sources
                .iter()
                .all(|src| route_operation(src, target_candidate).is_some())
        })
        .collect();

    if valid_targets.is_empty() {
        // No valid operations found - this means all possible interpretations of the target
        // would result in invalid operations with at least one source.
        let source_summary = if sources.len() == 1 {
            format!("source {}", sources[0].to_short_string())
        } else {
            format!(
                "sources ({})",
                sources
                    .iter()
                    .map(|s| s.to_short_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        return Err(anyhow::anyhow!(
            "Target '{target}' matches multiple objects, but none would result in valid operations with all {source_summary}. Try using more characters or a different identifier."
        ));
    }

    if valid_targets.len() == 1 {
        // Disambiguation successful through validity filtering!
        return Ok((sources, valid_targets[0].clone()));
    }

    // Still ambiguous even after filtering by validity - prompt the user
    let selected_target = prompt_for_disambiguation(target, valid_targets, "the target", out)?;
    Ok((sources, selected_target))
}
fn create_snapshot(ctx: &mut Context, operation: OperationKind) {
    let mut guard = ctx.exclusive_worktree_access();
    let _snapshot = ctx
        .create_snapshot(SnapshotDetails::new(operation), guard.write_permission())
        .ok(); // Ignore errors for snapshot creation
}

/// Resolves a single entity string to a CliId with disambiguation support.
///
/// If the entity matches multiple IDs, this will prompt the user to disambiguate
/// in interactive mode, or error in non-interactive mode.
///
/// # Arguments
/// * `id_map` - The ID map to resolve against
/// * `entity_str` - The string to resolve (e.g., "ab", "main")
/// * `context` - Description for error messages (e.g., "commit", "branch")
/// * `out` - Output channel for interactive prompts
///
/// # Returns
/// The resolved CliId
fn resolve_single_id(
    ctx: &mut Context,
    id_map: &IdMap,
    entity_str: &str,
    context: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<CliId> {
    let matches = id_map.parse_using_context(entity_str, ctx)?;

    if matches.is_empty() {
        return Err(anyhow::anyhow!(
            "{context} '{entity_str}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
        ));
    }

    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }

    // Multiple matches - use disambiguation
    prompt_for_disambiguation(entity_str, matches, context, out)
}

/// Handler for `but uncommit <source>` - runs `but rub <source> zz`
/// Validates that source is a commit or file-in-commit.
pub(crate) fn handle_uncommit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let sources = parse_sources_with_disambiguation(ctx, &id_map, source_str, out)?;

    // Validate that all sources are commits or committed files
    for source in &sources {
        match source {
            CliId::Commit { .. } | CliId::CommittedFile { .. } => {
                // Valid types for uncommit
            }
            _ => {
                bail!(
                    "Cannot uncommit {} - it is {}. Only commits and files-in-commits can be uncommitted.",
                    source_str.blue().bold(),
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
    let files = parse_sources_with_disambiguation(ctx, &id_map, file_str, out)?;
    let commit = resolve_single_id(ctx, &id_map, commit_str, "Commit", out)?;

    // Validate that all files are uncommitted
    for file in &files {
        match file {
            CliId::Uncommitted(_) => {
                // Valid type for amend
            }
            _ => {
                bail!(
                    "Cannot amend {} - it is {}. Only uncommitted files and hunks can be amended.",
                    file.to_short_string().blue().bold(),
                    file.kind_for_humans().yellow()
                );
            }
        }
    }

    // Validate that commit is a commit
    match &commit {
        CliId::Commit { .. } => {
            // Valid type for target
        }
        other => {
            bail!(
                "Cannot amend into {} - it is {}. Target must be a commit.",
                other.to_short_string().blue().bold(),
                other.kind_for_humans().yellow()
            );
        }
    }

    // Call the main rub handler
    handle(ctx, out, file_str, commit_str)
}

/// Handler for `but stage <file_or_hunk> <branch>` - runs `but rub <file_or_hunk> <branch>`
/// Validates that file_or_hunk is uncommitted or a path prefix, and that branch is a branch.
pub(crate) fn handle_stage(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_or_hunk_str: &str,
    branch_str: &str,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let files = parse_sources_with_disambiguation(ctx, &id_map, file_or_hunk_str, out)?;
    let branch = resolve_single_id(ctx, &id_map, branch_str, "Branch", out)?;

    // Validate that all files are uncommitted or a path prefix
    for file in &files {
        match file {
            CliId::Uncommitted(_) | CliId::PathPrefix { .. } => {
                // Valid type for stage
            }
            _ => {
                bail!(
                    "Cannot stage {} - it is {}. Only uncommitted files and hunks can be staged.",
                    file.to_short_string().blue().bold(),
                    file.kind_for_humans().yellow()
                );
            }
        }
    }

    // Validate that branch is a branch
    match &branch {
        CliId::Branch { .. } => {
            // Valid type for target
        }
        other => {
            bail!(
                "Cannot stage to {} - it is {}. Target must be a branch.",
                other.to_short_string().blue().bold(),
                other.kind_for_humans().yellow()
            );
        }
    }

    // Call the main rub handler
    handle(ctx, out, file_or_hunk_str, branch_str)
}

/// Handler for `but stage --tui` - interactive hunk selection TUI.
/// If `branch_str` is None, prompts the user to select a branch.
pub(crate) fn handle_stage_tui(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_str: Option<&str>,
) -> anyhow::Result<()> {
    use crate::tui::stage_viewer::{StageFileEntry, StageResult};

    let id_map = IdMap::new_from_context(ctx, None)?;

    // Resolve branch: from flag, or interactive selection
    let branch_name = if let Some(branch_str) = branch_str {
        let branch = resolve_single_id(ctx, &id_map, branch_str, "Branch", out)?;
        match &branch {
            CliId::Branch { name, .. } => name.clone(),
            other => {
                bail!(
                    "Cannot stage to {} - it is {}. Target must be a branch.",
                    other.to_short_string().blue().bold(),
                    other.kind_for_humans().yellow()
                );
            }
        }
    } else {
        // Get available stacks, use top branch of each as the staging target
        let stacks = crate::legacy::commits::stacks(ctx)?;
        let stack_top_branches: Vec<String> = stacks
            .iter()
            .filter_map(|s| s.heads.first().map(|h| h.name.to_string()))
            .collect();

        if stack_top_branches.is_empty() {
            // Auto-create a branch with a generated name
            let branch_name = but_api::legacy::workspace::canned_branch_name(ctx)?;
            but_api::legacy::stack::create_reference(
                ctx,
                but_api::legacy::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor: None,
                },
            )?;
            if let Some(out) = out.for_human() {
                writeln!(out, "Created new branch '{branch_name}'")?;
            }
            branch_name
        } else if stack_top_branches.len() == 1 {
            stack_top_branches.into_iter().next().unwrap()
        } else {
            match crate::tui::stage_viewer::run_branch_selector(&stack_top_branches)? {
                Some(name) => name,
                None => {
                    if let Some(out) = out.for_human() {
                        writeln!(out, "Stage cancelled.")?;
                    }
                    return Ok(());
                }
            }
        }
    };

    let files = StageFileEntry::from_worktree(&id_map);

    if files.is_empty() {
        bail!("No uncommitted changes to stage.");
    }

    let result = crate::tui::stage_viewer::run_stage_viewer(files, &branch_name)?;

    match result {
        StageResult::Stage {
            selected,
            unselected,
        } => {
            if selected.is_empty() {
                if let Some(out) = out.for_human() {
                    writeln!(out, "No hunks selected. Nothing staged.")?;
                }
                return Ok(());
            }
            create_snapshot(ctx, OperationKind::MoveHunk);
            // Stage selected hunks to the target branch
            let mut reqs =
                assign::to_assignment_request(ctx, selected.into_iter(), Some(&branch_name))?;
            // Unassign deselected hunks (set stack_id to None)
            reqs.extend(assign::to_assignment_request(
                ctx,
                unselected.into_iter(),
                None,
            )?);
            assign::do_assignments(ctx, reqs, out)?;
            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "Staged selected hunks → {}.",
                    format!("[{branch_name}]").green()
                )?;
            }
            Ok(())
        }
        StageResult::Cancelled => {
            if let Some(out) = out.for_human() {
                writeln!(out, "Stage cancelled.")?;
            }
            Ok(())
        }
    }
}

/// Handler for `but unstage <file_or_hunk> [branch]` - runs `but rub <file_or_hunk> zz`
/// Validates that file_or_hunk is uncommitted or a path prefix. Optionally
/// validates it's assigned to the specified branch.
pub(crate) fn handle_unstage(
    ctx: &mut Context,
    out: &mut OutputChannel,
    file_or_hunk_str: &str,
    branch_str: Option<&str>,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let files = parse_sources_with_disambiguation(ctx, &id_map, file_or_hunk_str, out)?;

    // Validate that all files are uncommitted or a path prefix
    for file in &files {
        match file {
            CliId::Uncommitted(_) | CliId::PathPrefix { .. } => {
                // Valid type for unstage
            }
            _ => {
                bail!(
                    "Cannot unstage {} - it is {}. Only uncommitted files and hunks can be unstaged.",
                    file.to_short_string().blue().bold(),
                    file.kind_for_humans().yellow()
                );
            }
        }
    }

    // If a branch is specified, validate it exists (but we don't strictly require the file to be assigned to it)
    if let Some(branch_name) = branch_str {
        let branch = resolve_single_id(ctx, &id_map, branch_name, "Branch", out)?;
        match &branch {
            CliId::Branch { .. } => {
                // Valid - branch exists
            }
            other => {
                bail!(
                    "Cannot unstage from {} - it is {}. Target must be a branch.",
                    other.to_short_string().blue().bold(),
                    other.kind_for_humans().yellow()
                );
            }
        }
    }

    // Call the main rub handler with "zz" as target to unassign
    handle(ctx, out, file_or_hunk_str, "zz")
}

#[cfg(test)]
mod tests {
    use bstr::BString;
    use nonempty::NonEmpty;

    use super::*;

    // Helper to create test CliIds
    fn uncommitted_id() -> CliId {
        CliId::Uncommitted(crate::id::UncommittedCliId {
            id: "ab".to_string(),
            hunk_assignments: NonEmpty::new(but_hunk_assignment::HunkAssignment {
                id: None,
                hunk_header: None,
                path: "test.txt".to_string(),
                path_bytes: BString::from("test.txt"),
                stack_id: None,
                hunk_locks: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            }),
            is_entire_file: true,
        })
    }

    fn committed_file_id() -> CliId {
        CliId::CommittedFile {
            commit_id: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            path: BString::from("test.txt"),
            id: "cd".to_string(),
        }
    }

    fn branch_id() -> CliId {
        CliId::Branch {
            name: "main".to_string(),
            id: "ef".to_string(),
            stack_id: None,
        }
    }

    fn commit_id() -> CliId {
        CliId::Commit {
            commit_id: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            id: "gh".to_string(),
        }
    }

    fn unassigned_id() -> CliId {
        CliId::Unassigned {
            id: "zz".to_string(),
        }
    }

    fn stack_id() -> CliId {
        CliId::Stack {
            id: "ij".to_string(),
            stack_id: StackId::generate(),
        }
    }

    #[test]
    fn test_route_operation_uncommitted_to_targets() {
        let uncommitted = uncommitted_id();

        // Valid: Uncommitted -> Unassigned
        assert!(route_operation(&uncommitted, &unassigned_id()).is_some());

        // Valid: Uncommitted -> Commit
        assert!(route_operation(&uncommitted, &commit_id()).is_some());

        // Valid: Uncommitted -> Branch
        assert!(route_operation(&uncommitted, &branch_id()).is_some());

        // Valid: Uncommitted -> Stack
        assert!(route_operation(&uncommitted, &stack_id()).is_some());

        // Invalid: Uncommitted -> Uncommitted
        assert!(route_operation(&uncommitted, &uncommitted_id()).is_none());

        // Invalid: Uncommitted -> CommittedFile
        assert!(route_operation(&uncommitted, &committed_file_id()).is_none());
    }

    #[test]
    fn test_route_operation_commit_to_targets() {
        let commit = commit_id();

        // Valid: Commit -> Unassigned
        assert!(route_operation(&commit, &unassigned_id()).is_some());

        // Valid: Commit -> Commit
        assert!(route_operation(&commit, &commit_id()).is_some());

        // Valid: Commit -> Branch
        assert!(route_operation(&commit, &branch_id()).is_some());

        // Invalid: Commit -> Uncommitted
        assert!(route_operation(&commit, &uncommitted_id()).is_none());

        // Invalid: Commit -> Stack
        assert!(route_operation(&commit, &stack_id()).is_none());

        // Invalid: Commit -> CommittedFile
        assert!(route_operation(&commit, &committed_file_id()).is_none());
    }

    #[test]
    fn test_route_operation_branch_to_targets() {
        let branch = branch_id();

        // Valid: Branch -> Unassigned
        assert!(route_operation(&branch, &unassigned_id()).is_some());

        // Valid: Branch -> Stack
        assert!(route_operation(&branch, &stack_id()).is_some());

        // Valid: Branch -> Commit
        assert!(route_operation(&branch, &commit_id()).is_some());

        // Valid: Branch -> Branch
        assert!(route_operation(&branch, &branch_id()).is_some());

        // Invalid: Branch -> Uncommitted
        assert!(route_operation(&branch, &uncommitted_id()).is_none());

        // Invalid: Branch -> CommittedFile
        assert!(route_operation(&branch, &committed_file_id()).is_none());
    }

    #[test]
    fn test_route_operation_stack_to_targets() {
        let stack = stack_id();

        // Valid: Stack -> Unassigned
        assert!(route_operation(&stack, &unassigned_id()).is_some());

        // Valid: Stack -> Stack
        assert!(route_operation(&stack, &stack_id()).is_some());

        // Valid: Stack -> Branch
        assert!(route_operation(&stack, &branch_id()).is_some());

        // Invalid: Stack -> Uncommitted
        assert!(route_operation(&stack, &uncommitted_id()).is_none());

        // Invalid: Stack -> Commit
        assert!(route_operation(&stack, &commit_id()).is_none());

        // Invalid: Stack -> CommittedFile
        assert!(route_operation(&stack, &committed_file_id()).is_none());
    }

    #[test]
    fn test_route_operation_unassigned_to_targets() {
        let unassigned = unassigned_id();

        // Valid: Unassigned -> Commit
        assert!(route_operation(&unassigned, &commit_id()).is_some());

        // Valid: Unassigned -> Branch
        assert!(route_operation(&unassigned, &branch_id()).is_some());

        // Valid: Unassigned -> Stack
        assert!(route_operation(&unassigned, &stack_id()).is_some());

        // Invalid: Unassigned -> Uncommitted
        assert!(route_operation(&unassigned, &uncommitted_id()).is_none());

        // Invalid: Unassigned -> Unassigned
        assert!(route_operation(&unassigned, &unassigned_id()).is_none());

        // Invalid: Unassigned -> CommittedFile
        assert!(route_operation(&unassigned, &committed_file_id()).is_none());
    }

    #[test]
    fn test_route_operation_committed_file_to_targets() {
        let committed_file = committed_file_id();

        // Valid: CommittedFile -> Branch
        assert!(route_operation(&committed_file, &branch_id()).is_some());

        // Valid: CommittedFile -> Commit
        assert!(route_operation(&committed_file, &commit_id()).is_some());

        // Valid: CommittedFile -> Unassigned
        assert!(route_operation(&committed_file, &unassigned_id()).is_some());

        // Invalid: CommittedFile -> Uncommitted
        assert!(route_operation(&committed_file, &uncommitted_id()).is_none());

        // Invalid: CommittedFile -> Stack
        assert!(route_operation(&committed_file, &stack_id()).is_none());

        // Invalid: CommittedFile -> CommittedFile
        assert!(route_operation(&committed_file, &committed_file_id()).is_none());
    }

    /// Verifies that route_operation returns the correct variant (not just Some/None).
    /// This test ensures the routing logic maps to the right operation types.
    #[test]
    fn test_route_operation_returns_correct_variants() {
        let uncommitted = uncommitted_id();
        let committed_file = committed_file_id();
        let branch = branch_id();
        let commit = commit_id();
        let unassigned = unassigned_id();
        let stack = stack_id();

        // Test a representative sample of operations to verify correct variant matching
        // We use match with wildcard to verify the variant type without destructuring all fields

        // Uncommitted -> Unassigned should be UnassignUncommitted
        match route_operation(&uncommitted, &unassigned) {
            Some(RubOperation::UnassignUncommitted(..)) => {}
            _ => panic!("Expected UnassignUncommitted variant"),
        }

        // Uncommitted -> Commit should be UncommittedToCommit
        match route_operation(&uncommitted, &commit) {
            Some(RubOperation::UncommittedToCommit(..)) => {}
            _ => panic!("Expected UncommittedToCommit variant"),
        }

        // Commit -> Commit should be SquashCommits
        match route_operation(&commit, &commit_id()) {
            Some(RubOperation::SquashCommits { .. }) => {}
            _ => panic!("Expected SquashCommits variant"),
        }

        // Commit -> Unassigned should be UndoCommit
        match route_operation(&commit, &unassigned) {
            Some(RubOperation::UndoCommit(_)) => {}
            _ => panic!("Expected UndoCommit variant"),
        }

        // Branch -> Stack should be BranchToStack
        match route_operation(&branch, &stack) {
            Some(RubOperation::BranchToStack { .. }) => {}
            _ => panic!("Expected BranchToStack variant"),
        }

        // Stack -> Branch should be StackToBranch
        match route_operation(&stack, &branch) {
            Some(RubOperation::StackToBranch { .. }) => {}
            _ => panic!("Expected StackToBranch variant"),
        }

        // CommittedFile -> Commit should be CommittedFileToCommit
        match route_operation(&committed_file, &commit) {
            Some(RubOperation::CommittedFileToCommit(_, _, _)) => {}
            _ => panic!("Expected CommittedFileToCommit variant"),
        }

        // Unassigned -> Stack should be UnassignedToStack
        match route_operation(&unassigned, &stack) {
            Some(RubOperation::UnassignedToStack(_)) => {}
            _ => panic!("Expected UnassignedToStack variant"),
        }
    }
}
