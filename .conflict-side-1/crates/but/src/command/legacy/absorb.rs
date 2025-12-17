use std::collections::BTreeMap;

use bstr::{BStr, BString, ByteSlice};
use but_api::{
    json::HexHash,
    legacy::{diff, virtual_branches},
};
use but_core::DiffSpec;
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use but_hunk_dependency::ui::HunkDependencies;
use colored::Colorize;
use gitbutler_project::Project;

use crate::{CliId, IdMap, command::legacy::rub::parse_sources, utils::OutputChannel};

/// Amends changes into the appropriate commits where they belong.
///
/// The semantic for finding "the appropriate commit" is as follows
/// - Changes are amended into the topmost commit of the leftmost (first) lane (branch)
/// - If a change is assigned to a particular lane (branch), it will be amended into a commit there
///     - If there are no commits in this branch, a new commit is created
/// - If a change has a dependency to a particular commit, it will be amended into that particular commit
///
/// Optionally an identifier to an Uncommitted File or a Branch (stack) may be provided.
///
/// If an Uncommitted File id is provided, absorb will be performed for just that file
/// If a Branch (stack) id is provided, absorb will be performed for all changes assigned to that stack
/// If no source is provided, absorb is performed for all uncommitted changes
pub(crate) fn handle(
    project: &Project,
    out: &mut OutputChannel,
    source: Option<&str>,
) -> anyhow::Result<()> {
    let ctx = &mut Context::new_from_legacy_project(project.clone())?;
    let mut id_map = IdMap::new_from_context(ctx)?;
    id_map.add_file_info_from_context(ctx)?;
    let source: Option<CliId> = source
        .and_then(|s| parse_sources(ctx, &id_map, s).ok())
        .and_then(|s| {
            s.into_iter().find(|s| {
                matches!(s, CliId::UncommittedFile { .. }) || matches!(s, CliId::Branch { .. })
            })
        });

    // Get all worktree changes, assignments, and dependencies
    let worktree_changes = diff::changes_in_worktree(project.id)?;
    let assignments = worktree_changes.assignments;
    let dependencies = worktree_changes.dependencies;

    if let Some(source) = source {
        match source {
            CliId::UncommittedFile {
                path, assignment, ..
            } => {
                // Absorb this particular file
                absorb_file(
                    project,
                    path.as_ref(),
                    assignment,
                    &assignments,
                    &dependencies,
                    out,
                )?;
            }
            CliId::Branch { name, .. } => {
                // Absorb everything that is assigned to this lane
                absorb_branch(project, &name, &assignments, &dependencies, out)?;
            }
            _ => {
                anyhow::bail!("Invalid source: expected an uncommitted file or branch");
            }
        }
    } else {
        // Try to absorb everything uncommitted
        absorb_all(project, &assignments, &dependencies, out)?;
    }
    Ok(())
}

/// Absorb a single file into the appropriate commit
fn absorb_file(
    project: &Project,
    path: &BStr,
    _assignment: Option<but_core::ref_metadata::StackId>,
    assignments: &[HunkAssignment],
    dependencies: &Option<HunkDependencies>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Filter assignments to just this file
    let file_assignments: Vec<_> = assignments
        .iter()
        .filter(|a| a.path_bytes == path)
        .cloned()
        .collect();

    if file_assignments.is_empty() {
        anyhow::bail!("No uncommitted changes found for file: {}", path);
    }

    // Group changes by their target commit
    let changes_by_commit =
        group_changes_by_target_commit(project.id, &file_assignments, dependencies)?;

    // Apply each group to its target commit
    for ((stack_id, commit_id), file_hunks) in changes_by_commit {
        let diff_specs = convert_assignments_to_diff_specs(&file_hunks)?;
        amend_commit(project, stack_id, commit_id, diff_specs, out)?;
    }

    Ok(())
}

/// Absorb all files assigned to a specific branch/stack
fn absorb_branch(
    project: &Project,
    branch_name: &str,
    assignments: &[HunkAssignment],
    dependencies: &Option<HunkDependencies>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Get the stack ID for this branch
    let stacks = but_api::legacy::workspace::stacks(project.id, None)?;

    // Find the stack that contains this branch
    let stack = stacks
        .iter()
        .find(|s| {
            s.heads
                .iter()
                .any(|h| h.name.to_str().map(|n| n == branch_name).unwrap_or(false))
        })
        .ok_or_else(|| anyhow::anyhow!("Branch not found: {}", branch_name))?;

    let stack_id = stack.id.ok_or_else(|| anyhow::anyhow!("Stack has no ID"))?;

    // Filter assignments to just this stack
    let stack_assignments: Vec<_> = assignments
        .iter()
        .filter(|a| a.stack_id == Some(stack_id))
        .cloned()
        .collect();

    if stack_assignments.is_empty() {
        anyhow::bail!("No uncommitted changes assigned to branch: {}", branch_name);
    }

    // Group changes by their target commit
    let changes_by_commit =
        group_changes_by_target_commit(project.id, &stack_assignments, dependencies)?;

    // Apply each group to its target commit
    for ((target_stack_id, commit_id), hunks) in changes_by_commit {
        let diff_specs = convert_assignments_to_diff_specs(&hunks)?;
        amend_commit(project, target_stack_id, commit_id, diff_specs, out)?;
    }

    Ok(())
}

/// Absorb all uncommitted changes
fn absorb_all(
    project: &Project,
    assignments: &[HunkAssignment],
    dependencies: &Option<HunkDependencies>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    if assignments.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No uncommitted changes to absorb")?;
        }
        return Ok(());
    }

    // Group all changes by their target commit
    let changes_by_commit = group_changes_by_target_commit(project.id, assignments, dependencies)?;

    // Apply each group to its target commit
    for ((stack_id, commit_id), hunks) in changes_by_commit {
        let diff_specs = convert_assignments_to_diff_specs(&hunks)?;
        amend_commit(project, stack_id, commit_id, diff_specs, out)?;
    }

    Ok(())
}

/// Group changes by their target commit based on dependencies and assignments
fn group_changes_by_target_commit(
    project_id: gitbutler_project::ProjectId,
    assignments: &[HunkAssignment],
    dependencies: &Option<HunkDependencies>,
) -> anyhow::Result<BTreeMap<(but_core::ref_metadata::StackId, gix::ObjectId), Vec<HunkAssignment>>>
{
    let mut changes_by_commit: BTreeMap<
        (but_core::ref_metadata::StackId, gix::ObjectId),
        Vec<HunkAssignment>,
    > = BTreeMap::new();

    // Process each assignment
    for assignment in assignments {
        // Determine the target commit for this assignment
        let (stack_id, commit_id) = determine_target_commit(project_id, assignment, dependencies)?;

        changes_by_commit
            .entry((stack_id, commit_id))
            .or_default()
            .push(assignment.clone());
    }

    Ok(changes_by_commit)
}

/// Determine the target commit for an assignment based on dependencies and assignments
fn determine_target_commit(
    project_id: gitbutler_project::ProjectId,
    assignment: &HunkAssignment,
    dependencies: &Option<HunkDependencies>,
) -> anyhow::Result<(but_core::ref_metadata::StackId, gix::ObjectId)> {
    // Priority 1: Check if there's a dependency lock for this hunk
    if let Some(deps) = dependencies
        && let Some(_hunk_id) = assignment.id
    {
        // Find the dependency for this hunk
        for (path, _hunk, locks) in &deps.diffs {
            // Match by path and hunk content
            if path == &assignment.path {
                // If there's a lock (dependency), use the topmost commit
                if let Some(lock) = locks.first() {
                    return Ok((lock.stack_id, lock.commit_id));
                }
            }
        }
    }

    // Priority 2: Use the assignment's stack ID if available
    if let Some(stack_id) = assignment.stack_id {
        // We need to find the topmost commit in this stack
        let stack_details = but_api::legacy::workspace::stack_details(project_id, Some(stack_id))?;

        // Find the topmost commit in the first branch
        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id));
        }

        // If there are no commits in the stack, create a blank commit first
        virtual_branches::insert_blank_commit(project_id, stack_id, None, -1)?;

        // Now fetch the stack details again to get the newly created commit
        let stack_details = but_api::legacy::workspace::stack_details(project_id, Some(stack_id))?;
        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id));
        }

        anyhow::bail!("Failed to create blank commit in stack: {:?}", stack_id);
    }

    // Priority 3: If no assignment, find the topmost commit of the leftmost lane
    let stacks = but_api::legacy::workspace::stacks(project_id, None)?;
    if let Some(stack) = stacks.first()
        && let Some(stack_id) = stack.id
    {
        let stack_details = but_api::legacy::workspace::stack_details(project_id, Some(stack_id))?;

        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id));
        }

        // If the first stack has no commits, create a blank commit first
        virtual_branches::insert_blank_commit(project_id, stack_id, None, -1)?;

        // Now fetch the stack details again to get the newly created commit
        let stack_details = but_api::legacy::workspace::stack_details(project_id, Some(stack_id))?;
        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id));
        }

        anyhow::bail!("Failed to create blank commit in leftmost stack");
    }

    anyhow::bail!(
        "Unable to determine target commit for unassigned change: {}",
        assignment.path
    );
}

/// Convert HunkAssignments to DiffSpecs
fn convert_assignments_to_diff_specs(
    assignments: &[HunkAssignment],
) -> anyhow::Result<Vec<DiffSpec>> {
    let mut specs_by_path: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();

    // Group assignments by file path
    for assignment in assignments {
        specs_by_path
            .entry(assignment.path_bytes.clone())
            .or_default()
            .push(assignment.clone());
    }

    // Convert to DiffSpecs
    let mut diff_specs = Vec::new();
    for (path, hunks) in specs_by_path {
        let mut hunk_headers = Vec::new();
        for hunk in hunks {
            if let Some(header) = hunk.hunk_header {
                hunk_headers.push(header);
            }
        }

        diff_specs.push(DiffSpec {
            previous_path: None, // TODO: Handle renames
            path: path.clone(),
            hunk_headers,
        });
    }

    Ok(diff_specs)
}

/// Amend a commit with the given changes
fn amend_commit(
    project: &Project,
    stack_id: but_core::ref_metadata::StackId,
    commit_id: gix::ObjectId,
    diff_specs: Vec<DiffSpec>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Convert commit_id to HexHash
    let hex_hash = HexHash::from(commit_id);

    let outcome = but_api::legacy::workspace::amend_commit_from_worktree_changes(
        project.id, stack_id, hex_hash, diff_specs,
    )?;

    if let Some(out) = out.for_human() {
        if !outcome.paths_to_rejected_changes.is_empty() {
            writeln!(
                out,
                "{warning}: Failed to absorb {} file(s)",
                outcome.paths_to_rejected_changes.len(),
                warning = "warning".yellow(),
            )?;
        }

        writeln!(
            out,
            "Absorbed changes into commit {}",
            &commit_id.to_hex().to_string()[..7]
        )?;
    }

    Ok(())
}
