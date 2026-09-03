use std::collections::{BTreeMap, HashMap};

use anyhow::Context as _;
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use but_hunk_assignment::{
    AbsorbCandidate, AbsorptionReason, AbsorptionTarget, CommitAbsorption, CommitMap,
    GroupedChanges, convert_hunks_to_diff_specs,
};
use but_hunk_dependency::ui::{
    HunkDependencies, HunkLock, HunkLockTarget,
    hunk_dependencies_for_workspace_changes_by_worktree_dir,
};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_workspace::{RefInfo, branch::Stack};
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use itertools::Itertools;
use tracing::instrument;

use crate::{
    commit::amend::commit_amend_only_impl, commit::insert_blank::commit_insert_blank_only_impl,
    commit::json::ChangesSource,
};
use but_core::DryRun;

/// Absorb the changes described by `absorption_plan` using the behavior documented by
/// [`absorb_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// snapshot and rewriting commits.
///
/// Before applying the plan, this records an `Absorb` oplog snapshot.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn absorb(ctx: &mut Context, absorption_plan: Vec<CommitAbsorption>) -> anyhow::Result<usize> {
    let mut guard = ctx.exclusive_worktree_access();
    // Create a snapshot before performing absorb operations
    // This allows the user to undo if needed
    let _snapshot = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::Absorb),
            guard.write_permission(),
        )
        .ok(); // Ignore errors for snapshot creation

    absorb_with_perm(ctx, absorption_plan, guard.write_permission())
}

/// Absorb the changes described by `absorption_plan` using the exclusive repository
/// access granted by `perm`, applying the updates through the modern commit amend API.
///
/// Returns the total amount of rejected diff specs.
pub fn absorb_with_perm(
    ctx: &mut Context,
    absorption_plan: Vec<CommitAbsorption>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<usize> {
    // Apply each group to its target commit and track failures
    let mut total_rejected = 0;
    let mut commit_map = CommitMap::default();
    let context_lines = ctx.settings.context_lines;

    for absorption in absorption_plan {
        let diff_specs = convert_hunks_to_diff_specs(&absorption.hunks)?;
        let commit_id = commit_map.find_mapped_id(absorption.commit_id);
        let outcome = commit_amend_only_impl(
            ctx,
            commit_id,
            diff_specs,
            &ChangesSource::Head,
            DryRun::No,
            context_lines,
            perm,
        )?;
        if !outcome.rejected_specs.is_empty() {
            tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
        }
        for (old, new) in &outcome.workspace.replaced_commits {
            commit_map.add_mapping(*old, *new);
        }
        total_rejected += outcome.rejected_specs.len();
    }
    Ok(total_rejected)
}

/// Build an absorption plan for `target` using the behavior documented by
/// [`absorption_plan_with_perm()`].
#[but_api(napi, provides = [AbsorptionPlan])]
#[instrument(err(Debug))]
pub fn absorption_plan(
    ctx: &mut Context,
    target: AbsorptionTarget,
) -> anyhow::Result<Vec<CommitAbsorption>> {
    let mut guard = ctx.exclusive_worktree_access();
    absorption_plan_with_perm(ctx, target, guard.write_permission())
}

/// Build an absorption plan for `target` while reusing the exclusive repository access
/// in `perm`.
///
/// Depending on `target`, this reads assigned worktree changes, stack state, and hunk
/// dependencies under the same locked view, then groups the selected hunks by destination
/// commit for display and later absorption.
///
/// The worktree inspection is driven by [`crate::diff::changes_in_worktree_with_perm()`].
pub fn absorption_plan_with_perm(
    ctx: &mut Context,
    target: AbsorptionTarget,
    perm: &mut RepoExclusive,
) -> anyhow::Result<Vec<CommitAbsorption>> {
    let (candidates, dependencies) = match target {
        AbsorptionTarget::Branch { branch_name } => {
            // Get all worktree changes, assignments, and dependencies
            // TODO: Ideally, there's a simpler way of getting the worktree changes without passing the context to it.
            // At this time, the context is passed pretty deep into the function.
            let worktree_changes = crate::diff::changes_in_worktree_with_perm(
                ctx,
                ChangesSource::Head,
                true,
                perm.read_permission(),
            )?;
            let all_assignments = worktree_changes.assignments;
            let dependencies = worktree_changes.dependencies;

            // Get the stack ID for this branch
            let workspace = crate::legacy::workspace::head_info(ctx)?;

            // Find the stack that contains this branch
            let stack = workspace
                .stacks
                .iter()
                .find(|stack| {
                    stack.segments.iter().any(|segment| {
                        segment
                            .ref_info
                            .as_ref()
                            .is_some_and(|ri| ri.ref_name.shorten() == branch_name.as_bytes())
                    })
                })
                .ok_or_else(|| anyhow::anyhow!("Branch not found: {branch_name}"))?;

            let stack_id = stack.id.ok_or_else(|| anyhow::anyhow!("Stack has no ID"))?;

            // Filter assignments to just this stack
            let candidates: Vec<AbsorbCandidate> = all_assignments
                .into_iter()
                .filter(|a| a.stack_id == Some(stack_id))
                .map(Into::into)
                .collect();

            if candidates.is_empty() {
                anyhow::bail!("No uncommitted changes assigned to branch: {branch_name}");
            }

            (candidates, dependencies)
        }
        AbsorptionTarget::TreeChanges {
            changes,
            assigned_stack_id,
        } => {
            // Get all worktree changes, assignments, and dependencies
            let worktree_changes = crate::diff::changes_in_worktree_with_perm(
                ctx,
                ChangesSource::Head,
                true,
                perm.read_permission(),
            )?;
            let all_assignments = worktree_changes.assignments;
            let dependencies = worktree_changes.dependencies;

            // Include hunks that are unassigned or assigned to the acting stack,
            // so that dependency locks can route unassigned hunks correctly.
            let candidates: Vec<AbsorbCandidate> = all_assignments
                .into_iter()
                .filter(|a| {
                    changes.iter().any(|c| c.path_bytes == a.path_bytes)
                        && (a.stack_id.is_none() || a.stack_id == assigned_stack_id)
                })
                .map(Into::into)
                .collect();

            if candidates.is_empty() {
                anyhow::bail!("No uncommitted changes found for the selected files");
            }

            (candidates, dependencies)
        }
        AbsorptionTarget::Hunks { hunks } => {
            // Compute hunk dependencies only for this target since changes_in_worktree isn't called
            let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
            let dependencies =
                hunk_dependencies_for_workspace_changes_by_worktree_dir(&repo, &ws, None).ok();
            drop((repo, ws, _db));
            (hunks.into_iter().map(Into::into).collect(), dependencies)
        }
        AbsorptionTarget::All => {
            // Get all worktree changes, assignments, and dependencies
            // TODO: Ideally, there's a simpler way of getting the worktree changes without passing the context to it.
            // At this time, the context is passed pretty deep into the function.
            let worktree_changes = crate::diff::changes_in_worktree_with_perm(
                ctx,
                ChangesSource::Head,
                true,
                perm.read_permission(),
            )?;
            (
                worktree_changes
                    .assignments
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                worktree_changes.dependencies,
            )
        }
    };

    // Group all changes by their target commit
    let changes_by_commit =
        group_changes_by_target_commit(ctx, &candidates, dependencies.as_ref(), perm)?;

    // Prepare commit absorptions for display
    let commit_absorptions = prepare_commit_absorptions(ctx, changes_by_commit)?;

    Ok(commit_absorptions)
}

/// Group changes by their target commit based on dependencies and assignments
fn group_changes_by_target_commit(
    ctx: &mut Context,
    candidates: &[AbsorbCandidate],
    dependencies: Option<&HunkDependencies>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<GroupedChanges> {
    let mut changes_by_commit: GroupedChanges = BTreeMap::new();

    // One projection of the workspace serves every candidate; it is re-read whenever
    // `ensure_target_commit()` inserts a blank commit.
    let mut workspace = crate::legacy::workspace::head_info(ctx)?;

    // Build an index for O(1) lock lookups per candidate
    let lock_index = dependencies.map(build_lock_index);

    // Process each candidate
    for candidate in candidates {
        // Determine the target commit for this candidate
        let locks = lock_index
            .as_ref()
            .map(|idx| locks_for_candidate(idx, candidate))
            .filter(|l| !l.is_empty());
        let (stack_id, commit_id, reason) =
            ensure_target_commit(ctx, candidate, locks.as_deref(), &mut workspace, perm)?;

        let entry = changes_by_commit
            .entry((stack_id, commit_id))
            .or_insert_with(|| (Vec::new(), reason.clone()));

        entry.0.push(candidate.clone());
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

/// Look up the dependency locks for a candidate by finding dependency hunks
/// whose ranges overlap with the candidate's hunk header.
///
/// When the candidate has no hunk header (binary/too-large diffs), all locks
/// for the file are returned as a fallback.
fn locks_for_candidate(index: &LockIndex, candidate: &AbsorbCandidate) -> Vec<HunkLock> {
    // `HunkDependencies` keys its diffs by a lossily-decoded path, so match on the same
    // lossy form. This borrows for the UTF-8 paths that make up practically all lookups.
    let Some(file_entries) = index.get(candidate.hunk.path.to_str_lossy().as_ref()) else {
        return Vec::new();
    };

    match candidate.hunk.hunk_header {
        Some(hunk_header) => {
            let mut locks = Vec::new();
            for (dep_hunk, dep_locks) in file_entries {
                // Match on the new-file side: candidate hunks describe worktree
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
fn find_top_most_lock<'a>(locks: &'a [HunkLock], workspace: &RefInfo) -> Option<&'a HunkLock> {
    // These are all the stack IDs that the hunk is dependent on.
    // If there are multiple, then the absorb will fail.
    let all_stack_ids = locks
        .iter()
        .map(|lock| lock.target)
        .unique()
        .collect::<Vec<_>>();
    for stack_id in &all_stack_ids {
        if let HunkLockTarget::Stack(stack_id) = stack_id {
            let stack = stack_by_id(workspace, *stack_id)?;
            for segment in stack.segments.iter() {
                for commit in segment.commits.iter() {
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

/// Find the stack identified by `stack_id` in the workspace projection.
fn stack_by_id(workspace: &RefInfo, stack_id: StackId) -> Option<&Stack> {
    workspace
        .stacks
        .iter()
        .find(|stack| stack.id == Some(stack_id))
}

/// Resolve the segment that an absorption targets in `stack_id`: the one named by `branch_ref` if
/// set, otherwise the topmost one. Returns its reference and the commit to absorb into, if any.
fn target_segment(
    workspace: &RefInfo,
    stack_id: StackId,
    branch_ref: Option<&gix::refs::FullName>,
) -> anyhow::Result<(gix::refs::FullName, Option<gix::ObjectId>)> {
    let stack = stack_by_id(workspace, stack_id)
        .with_context(|| format!("Couldn't find {stack_id} in the current workspace"))?;
    let segment = branch_ref
        .and_then(|branch_ref| {
            stack.segments.iter().find(|segment| {
                segment
                    .ref_info
                    .as_ref()
                    .is_some_and(|ri| &ri.ref_name == branch_ref)
            })
        })
        .or_else(|| stack.segments.first())
        .context("Stack has no branches")?;
    let ref_name = segment
        .ref_info
        .as_ref()
        .context("Can't absorb into a stack segment that isn't pointed to by a reference")?
        .ref_name
        .clone();
    Ok((ref_name, segment.commits.first().map(|commit| commit.id)))
}

/// Determine the target commit for a candidate based on dependencies and assignments
/// Create a blank one if needed.
fn ensure_target_commit(
    ctx: &mut Context,
    candidate: &AbsorbCandidate,
    locks: Option<&[HunkLock]>,
    workspace: &mut RefInfo,
    perm: &mut RepoExclusive,
) -> anyhow::Result<(
    but_core::ref_metadata::StackId,
    gix::ObjectId,
    AbsorptionReason,
)> {
    // Priority 1: Check if there's a dependency lock for this hunk
    if let Some(locks) = locks {
        if let Some(lock) = find_top_most_lock(locks, workspace) {
            if let HunkLockTarget::Stack(stack_id) = lock.target {
                return Ok((stack_id, lock.commit_id, AbsorptionReason::HunkDependency));
            }
        } else {
            anyhow::bail!(
                "Failed to determine target commit for hunk absorption due to ambiguous dependencies in path: {}",
                candidate.hunk.path
            );
        }
    }

    // Priority 2: Use the candidate's stack ID if available
    if let Some(stack_id) = candidate.stack_id {
        let branch_ref = candidate.branch_ref.as_ref();

        let (reference, commit_id) = target_segment(workspace, stack_id, branch_ref)?;
        if let Some(commit_id) = commit_id {
            return Ok((stack_id, commit_id, AbsorptionReason::StackAssignment));
        }

        // If there are no commits in the target branch, create a blank commit first
        commit_insert_blank_only_impl(
            ctx,
            RelativeTo::Reference(reference),
            InsertSide::Below,
            DryRun::No,
            perm,
        )?;

        // Project the workspace again to see the newly created commit
        *workspace = crate::legacy::workspace::head_info(ctx)?;
        if let (_, Some(commit_id)) = target_segment(workspace, stack_id, branch_ref)? {
            return Ok((stack_id, commit_id, AbsorptionReason::StackAssignment));
        }

        anyhow::bail!("Failed to create blank commit in stack: {stack_id:?}");
    }

    // Priority 3: If no assignment, find the topmost commit of the leftmost lane
    if let Some(stack_id) = workspace.stacks.first().and_then(|stack| stack.id) {
        let (reference, commit_id) = target_segment(workspace, stack_id, None)?;
        if let Some(commit_id) = commit_id {
            return Ok((stack_id, commit_id, AbsorptionReason::DefaultStack));
        }

        // If the first stack has no commits, create a blank commit first
        commit_insert_blank_only_impl(
            ctx,
            RelativeTo::Reference(reference),
            InsertSide::Below,
            DryRun::No,
            perm,
        )?;

        // Now project the workspace again to see the newly created commit
        *workspace = crate::legacy::workspace::head_info(ctx)?;
        if let (_, Some(commit_id)) = target_segment(workspace, stack_id, None)? {
            return Ok((stack_id, commit_id, AbsorptionReason::DefaultStack));
        }

        anyhow::bail!("Failed to create blank commit in leftmost stack");
    }

    anyhow::bail!(
        "Unable to determine target commit for unassigned change: {}",
        candidate.hunk.path
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

    // The workspace projection carries every stack's segments and commits in order
    let workspace = crate::legacy::workspace::head_info(ctx)?;
    let all_stack_ids = changes_by_commit
        .keys()
        .map(|(stack_id, _)| *stack_id)
        .unique()
        .collect::<Vec<_>>();

    // Iterate through the stacks' commits in application order (parent to child)
    for stack_id in all_stack_ids {
        let stack = stack_by_id(&workspace, stack_id)
            .with_context(|| format!("Couldn't find {stack_id} in the current workspace"))?;
        for segment in stack.segments.iter().rev() {
            for commit in segment.commits.iter().rev() {
                let key = (stack_id, commit.id);
                if let Some((candidates, reason)) = changes_by_commit.get(&key) {
                    let hunks = candidates
                        .iter()
                        .map(|candidate| candidate.hunk.clone())
                        .collect();
                    commit_absorptions.push(CommitAbsorption {
                        stack_id,
                        commit_id: commit.id,
                        commit_summary: get_commit_summary(&*ctx.repo.get()?, commit.id)?,
                        hunks,
                        reason: reason.clone(),
                    });
                }
            }
        }
    }

    Ok(commit_absorptions)
}

/// Get the commit summary message
fn get_commit_summary(repo: &gix::Repository, commit_id: gix::ObjectId) -> anyhow::Result<String> {
    let commit = repo.find_commit(commit_id)?;
    // The title still carries the trailing newline of single-paragraph messages.
    let message = commit.message()?.title.trim_end().as_bstr().to_string();
    Ok(message)
}
