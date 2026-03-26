use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::sync::RepoExclusive;
use but_ctx::Context;
use but_hunk_assignment::{
    AbsorptionReason, AbsorptionTarget, CommitAbsorption, CommitMap, FileAbsorption,
    GroupedChanges, HunkAssignment, convert_assignments_to_diff_specs,
};
use but_hunk_dependency::ui::{
    HunkDependencies, HunkLock, HunkLockTarget,
    hunk_dependencies_for_workspace_changes_by_worktree_dir,
};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_workspace::ui::StackDetails;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_stack::StackId;
use itertools::Itertools;
use tracing::instrument;

use crate::{
    commit::insert_blank::commit_insert_blank_only_impl, diff::changes_in_worktree,
    legacy::workspace::amend_commit_and_count_failures,
};

/// Absorb multiple changes into their target commits as per the provided absorption plan
#[but_api]
#[instrument(err(Debug))]
pub fn absorb(ctx: &mut Context, absorption_plan: Vec<CommitAbsorption>) -> anyhow::Result<usize> {
    let mut guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;
    let data_dir = ctx.project_data_dir();
    // Create a snapshot before performing absorb operations
    // This allows the user to undo if needed
    let _snapshot = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::Absorb),
            guard.write_permission(),
        )
        .ok(); // Ignore errors for snapshot creation

    let total_rejected = absorb_impl(absorption_plan, guard.write_permission(), &repo, &data_dir)?;

    // Refresh the workspace commit so `gitbutler/workspace` HEAD stays in sync
    // with the rewritten branch commits. Without this, tools that inspect HEAD
    // (e.g. pre-push hooks that stash against it) see a stale synthetic commit.
    gitbutler_branch_actions::update_workspace_commit(ctx, false)?;

    Ok(total_rejected)
}

pub fn absorb_impl(
    absorption_plan: Vec<CommitAbsorption>,
    perm: &mut RepoExclusive,
    repo: &gix::Repository,
    data_dir: &Path,
) -> anyhow::Result<usize> {
    // Apply each group to its target commit and track failures
    let mut total_rejected = 0;
    let mut commit_map = CommitMap::default();

    for absorption in absorption_plan {
        let diff_specs = convert_assignments_to_diff_specs(
            &absorption
                .files
                .iter()
                .map(|f| f.assignment.clone())
                .collect::<Vec<_>>(),
        )?;
        let commit_id = commit_map.find_mapped_id(absorption.commit_id);
        let outcome = amend_commit_and_count_failures(
            absorption.stack_id,
            commit_id,
            diff_specs,
            perm,
            repo,
            data_dir,
        )?;
        if let Some(rebase_output) = &outcome.rebase_output {
            for (_base, old, new) in &rebase_output.commit_mapping {
                commit_map.add_mapping(*old, *new);
            }
        }
        total_rejected += outcome.rejected_specs.len();
    }
    Ok(total_rejected)
}

/// Generate an absorption plan based on the provided target, based on hunk dependencies, assignments and other heuristics
#[but_api]
#[instrument(err(Debug))]
pub fn absorption_plan(
    ctx: &mut Context,
    target: AbsorptionTarget,
) -> anyhow::Result<Vec<CommitAbsorption>> {
    let (assignments, dependencies) = match target {
        AbsorptionTarget::Branch { branch_name } => {
            // Get all worktree changes, assignments, and dependencies
            // TODO: Ideally, there's a simpler way of getting the worktree changes without passing the context to it.
            // At this time, the context is passed pretty deep into the function.
            let worktree_changes = changes_in_worktree(ctx)?;
            let all_assignments = worktree_changes.assignments;
            let dependencies = worktree_changes.dependencies;

            // Get the stack ID for this branch
            let stacks = crate::legacy::workspace::stacks(ctx, None)?;

            // Find the stack that contains this branch
            let stack = stacks
                .iter()
                .find(|s| {
                    s.heads
                        .iter()
                        .any(|h| h.name.to_str().map(|n| n == branch_name).unwrap_or(false))
                })
                .ok_or_else(|| anyhow::anyhow!("Branch not found: {branch_name}"))?;

            let stack_id = stack.id.ok_or_else(|| anyhow::anyhow!("Stack has no ID"))?;

            // Filter assignments to just this stack
            let stack_assignments: Vec<_> = all_assignments
                .iter()
                .filter(|a| a.stack_id == Some(stack_id))
                .cloned()
                .collect();

            if stack_assignments.is_empty() {
                anyhow::bail!("No uncommitted changes assigned to branch: {branch_name}");
            }

            (stack_assignments, dependencies)
        }
        AbsorptionTarget::TreeChanges {
            changes,
            assigned_stack_id,
        } => {
            // Get all worktree changes, assignments, and dependencies
            let worktree_changes = changes_in_worktree(ctx)?;
            let all_assignments = worktree_changes.assignments;
            let dependencies = worktree_changes.dependencies;

            // Filter assignments to just this stack
            let stack_assignments: Vec<_> = all_assignments
                .iter()
                .filter(|a| {
                    a.stack_id == assigned_stack_id
                        && changes.iter().any(|c| c.path_bytes == a.path_bytes)
                })
                .cloned()
                .collect();

            if stack_assignments.is_empty() {
                anyhow::bail!("No uncommitted changes assigned to stack: {assigned_stack_id:?}");
            }

            (stack_assignments, dependencies)
        }
        AbsorptionTarget::HunkAssignments { assignments } => {
            // Compute hunk dependencies only for this target since changes_in_worktree isn't called
            let (_read_guard, repo, ws, _db) = ctx.workspace_and_db()?;
            let dependencies =
                hunk_dependencies_for_workspace_changes_by_worktree_dir(&repo, &ws, None).ok();
            drop((_read_guard, repo, ws, _db));
            (assignments, dependencies)
        }
        AbsorptionTarget::All => {
            // Get all worktree changes, assignments, and dependencies
            // TODO: Ideally, there's a simpler way of getting the worktree changes without passing the context to it.
            // At this time, the context is passed pretty deep into the function.
            let worktree_changes = changes_in_worktree(ctx)?;
            (worktree_changes.assignments, worktree_changes.dependencies)
        }
    };

    let mut guard = ctx.exclusive_worktree_access();

    // Group all changes by their target commit
    let changes_by_commit = group_changes_by_target_commit(
        ctx,
        &assignments,
        dependencies.as_ref(),
        guard.write_permission(),
    )?;

    // Prepare commit absorptions for display
    let commit_absorptions = prepare_commit_absorptions(ctx, changes_by_commit)?;

    Ok(commit_absorptions)
}

/// Group changes by their target commit based on dependencies and assignments
fn group_changes_by_target_commit(
    ctx: &mut Context,
    assignments: &[HunkAssignment],
    dependencies: Option<&HunkDependencies>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<GroupedChanges> {
    let mut changes_by_commit: GroupedChanges = BTreeMap::new();

    let mut stack_details_cache = HashMap::<StackId, StackDetails>::new();

    // Build an index for O(1) lock lookups per assignment
    let lock_index = dependencies.map(build_lock_index);

    // Process each assignment
    for assignment in assignments {
        // Determine the target commit for this assignment
        let locks = lock_index
            .as_ref()
            .map(|idx| locks_for_assignment(idx, assignment))
            .filter(|l| !l.is_empty());
        let (stack_id, commit_id, reason) = determine_target_commit(
            ctx,
            assignment,
            locks.as_deref(),
            &mut stack_details_cache,
            perm,
        )?;

        let entry = changes_by_commit
            .entry((stack_id, commit_id))
            .or_insert_with(|| (Vec::new(), reason.clone()));

        entry.0.push(assignment.clone());
        // If we have any hunk dependencies, that takes precedence as the reason for this commit group
        if reason == AbsorptionReason::HunkDependency {
            entry.1 = reason;
        }
    }

    Ok(changes_by_commit)
}

/// Per-file entries of `(DiffHunk, locks)` for range-based lock matching.
type LockIndex = HashMap<String, Vec<(but_core::unified_diff::DiffHunk, Vec<HunkLock>)>>;

/// Build a lookup index from hunk dependencies, grouped by file path.
///
/// Each entry retains the original `DiffHunk` range so that lookups can match
/// by range overlap rather than exact header equality. This is necessary because
/// dependency hunks are computed with 0 context lines while assignment hunks use
/// the user's `context_lines` setting, so their headers differ.
fn build_lock_index(dependencies: &HunkDependencies) -> LockIndex {
    let mut index = LockIndex::new();
    for (path, diff_hunk, locks) in &dependencies.diffs {
        index
            .entry(path.clone())
            .or_default()
            .push((diff_hunk.clone(), locks.clone()));
    }
    index
}

/// Check whether two line ranges overlap.
/// Ranges are `[start, start + lines)` (1-based start, length in lines).
/// A range with 0 lines (pure insertion/deletion) is treated as a point at `start`.
fn ranges_overlap(start_a: u32, lines_a: u32, start_b: u32, lines_b: u32) -> bool {
    let end_a = start_a + lines_a.max(1);
    let end_b = start_b + lines_b.max(1);
    start_a < end_b && start_b < end_a
}

/// Look up the dependency locks for an assignment by finding dependency hunks
/// whose ranges overlap with the assignment's hunk header.
///
/// When the assignment has no hunk header (binary/too-large diffs), all locks
/// for the file are returned as a fallback.
fn locks_for_assignment(index: &LockIndex, assignment: &HunkAssignment) -> Vec<HunkLock> {
    let Some(file_entries) = index.get(&assignment.path) else {
        return Vec::new();
    };

    match assignment.hunk_header {
        Some(hunk_header) => {
            let mut locks = Vec::new();
            for (dep_hunk, dep_locks) in file_entries {
                // Match on the new-file side: assignment hunks describe worktree
                // state (new), and dependency hunks record which committed ranges
                // they depend on.
                if ranges_overlap(
                    dep_hunk.new_start,
                    dep_hunk.new_lines,
                    hunk_header.new_start,
                    hunk_header.new_lines,
                ) {
                    locks.extend(dep_locks.iter().cloned());
                }
            }
            locks
        }
        // No hunk header (binary/too-large) — we can't do range matching,
        // and returning all file locks would be ambiguous if they span multiple
        // stacks/commits. Fall back to default assignment behavior instead.
        None => Vec::new(),
    }
}

// Find the lock that is highest in the application order (child-most commit)
fn find_top_most_lock<'a>(
    locks: &'a [HunkLock],
    ctx: &mut Context,
    stack_details_cache: &'a mut HashMap<StackId, StackDetails>,
) -> Option<&'a HunkLock> {
    // These are all the stack IDs that the hunk is dependent on.
    // If there are multiple, then the absorb will fail.
    let all_stack_ids = locks
        .iter()
        .map(|lock| lock.target)
        .unique()
        .collect::<Vec<_>>();
    for stack_id in &all_stack_ids {
        if let HunkLockTarget::Stack(stack_id) = stack_id {
            let stack_details = if let Some(details) = stack_details_cache.get(stack_id) {
                details.clone()
            } else {
                let details = crate::legacy::workspace::stack_details(ctx, Some(*stack_id)).ok()?;
                stack_details_cache.insert(*stack_id, details.clone());
                details
            };
            for branch in stack_details.branch_details.iter() {
                for commit in branch.commits.iter() {
                    if let Some(lock) = locks.iter().find(|l| {
                        l.commit_id == commit.id && l.target == HunkLockTarget::Stack(*stack_id)
                    }) {
                        return Some(lock);
                    }
                }
            }
        } else {
            // We've got locks to unknown stacks, just return the first one.
            return locks.first();
        }
    }
    None
}

/// Determine the target commit for an assignment based on dependencies and assignments
fn determine_target_commit(
    ctx: &mut Context,
    assignment: &HunkAssignment,
    locks: Option<&[HunkLock]>,
    stack_details_cache: &mut HashMap<StackId, StackDetails>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<(
    but_core::ref_metadata::StackId,
    gix::ObjectId,
    AbsorptionReason,
)> {
    // Priority 1: Check if there's a dependency lock for this hunk
    if let Some(locks) = locks {
        if let Some(lock) = find_top_most_lock(locks, ctx, stack_details_cache) {
            if let HunkLockTarget::Stack(stack_id) = lock.target {
                return Ok((stack_id, lock.commit_id, AbsorptionReason::HunkDependency));
            }
        } else {
            anyhow::bail!(
                "Failed to determine target commit for hunk absorption due to ambiguous dependencies in path: {}",
                assignment.path
            );
        }
    }

    // Priority 2: Use the assignment's stack ID if available
    if let Some(stack_id) = assignment.stack_id {
        // We need to find the topmost commit in this stack
        let stack_details = crate::legacy::workspace::stack_details(ctx, Some(stack_id))?;

        // Find the topmost commit in the first branch
        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id, AbsorptionReason::StackAssignment));
        }

        // If there are no commits in the stack, create a blank commit first
        let branch = stack_details
            .branch_details
            .first()
            .ok_or_else(|| anyhow::anyhow!("Stack has no branches"))?;
        commit_insert_blank_only_impl(
            ctx,
            RelativeTo::Reference(branch.reference.clone()),
            InsertSide::Below,
            perm,
        )?;

        // Now fetch the stack details again to get the newly created commit
        let stack_details = crate::legacy::workspace::stack_details(ctx, Some(stack_id))?;
        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id, AbsorptionReason::StackAssignment));
        }

        anyhow::bail!("Failed to create blank commit in stack: {stack_id:?}");
    }

    // Priority 3: If no assignment, find the topmost commit of the leftmost lane
    let stacks = crate::legacy::workspace::stacks(ctx, None)?;
    if let Some(stack) = stacks.first()
        && let Some(stack_id) = stack.id
    {
        let stack_details = crate::legacy::workspace::stack_details(ctx, Some(stack_id))?;
        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id, AbsorptionReason::DefaultStack));
        }

        // If the first stack has no commits, create a blank commit first
        let branch = stack_details
            .branch_details
            .first()
            .ok_or_else(|| anyhow::anyhow!("Stack has no branches"))?;
        commit_insert_blank_only_impl(
            ctx,
            RelativeTo::Reference(branch.reference.clone()),
            InsertSide::Below,
            perm,
        )?;

        // Now fetch the stack details again to get the newly created commit
        let stack_details = crate::legacy::workspace::stack_details(ctx, Some(stack_id))?;
        if let Some(branch) = stack_details.branch_details.first()
            && let Some(commit) = branch.commits.first()
        {
            return Ok((stack_id, commit.id, AbsorptionReason::DefaultStack));
        }

        anyhow::bail!("Failed to create blank commit in leftmost stack");
    }

    anyhow::bail!(
        "Unable to determine target commit for unassigned change: {}",
        assignment.path
    );
}

/// Prepare commit absorptions with commit summaries
///
/// This returns a vector of absorption information, sorted and ready for processing.
fn prepare_commit_absorptions(
    ctx: &Context,
    changes_by_commit: GroupedChanges,
) -> anyhow::Result<Vec<CommitAbsorption>> {
    let mut commit_absorptions = Vec::new();

    // Cache the stack details to determine the commit order
    let mut stack_details_map = HashMap::<StackId, StackDetails>::new();
    let all_stack_ids = changes_by_commit
        .keys()
        .map(|(stack_id, _)| *stack_id)
        .unique()
        .collect::<Vec<_>>();

    for stack_id in &all_stack_ids {
        if let std::collections::hash_map::Entry::Vacant(e) = stack_details_map.entry(*stack_id) {
            let details = crate::legacy::workspace::stack_details(ctx, Some(*stack_id))?;
            e.insert(details);
        }
    }
    // Iterate through the stacks' commits in application order (parent to child)
    for stack_id in all_stack_ids {
        if let Some(stack_details) = stack_details_map.get(&stack_id) {
            for branch in stack_details.branch_details.iter().rev() {
                for commit in branch.commits.iter().rev() {
                    let key = (stack_id, commit.id);
                    if let Some((assignments, reason)) = changes_by_commit.get(&key) {
                        let mut files = Vec::new();
                        for assignment in assignments {
                            files.push(FileAbsorption {
                                path: assignment.path.clone(),
                                assignment: assignment.clone(),
                            });
                        }
                        commit_absorptions.push(CommitAbsorption {
                            stack_id,
                            commit_id: commit.id,
                            commit_summary: get_commit_summary(&*ctx.repo.get()?, commit.id)?,
                            files,
                            reason: reason.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(commit_absorptions)
}

/// Get the commit summary message
fn get_commit_summary(repo: &gix::Repository, commit_id: gix::ObjectId) -> anyhow::Result<String> {
    let commit = repo.find_commit(commit_id)?;
    let message = commit.message()?.title.to_string();
    Ok(message)
}
