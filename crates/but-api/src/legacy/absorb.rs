use std::collections::{BTreeMap, HashMap};

use anyhow::Context as _;
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use but_hunk_assignment::{
    AbsorbCandidate, AbsorptionReason, AbsorptionTarget, CommitAbsorption, CommitMap,
    convert_hunks_to_diff_specs,
};
use but_hunk_dependency::ui::{
    HunkDependencies, HunkLock, HunkLockTarget,
    hunk_dependencies_for_workspace_changes_by_worktree_dir,
};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use but_workspace::{RefInfo, branch::Stack, commit::ChangeSource};
use gitbutler_oplog::OplogExt as _;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use itertools::Itertools;
use tracing::instrument;

use crate::commit::json::ChangesSource;

type GroupedChanges = BTreeMap<
    (StackId, gix::ObjectId, Option<gix::refs::FullName>),
    (Vec<AbsorbCandidate>, AbsorptionReason),
>;

#[derive(Debug, Default)]
pub struct AbsorbExecutionOutcome {
    rejected: Vec<RejectedAbsorption>,
}

#[derive(Debug)]
pub struct AbsorbFinalizationError {
    source: anyhow::Error,
    undo_available: bool,
}

impl std::fmt::Display for AbsorbFinalizationError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.undo_available {
            write!(
                out,
                "Absorb changes were published and an undo checkpoint was created, but finalization \
                 failed. Run `but undo` before retrying: {}",
                self.source
            )
        } else {
            write!(
                out,
                "Absorb changes were published, but finalization failed. Automatic undo is \
                 unavailable; inspect the workspace before retrying: {}",
                self.source
            )
        }
    }
}

impl std::error::Error for AbsorbFinalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

#[derive(Debug)]
pub struct AbsorbCheckpointError(anyhow::Error);

impl std::fmt::Display for AbsorbCheckpointError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            out,
            "Absorb changes were published, but creating the undo checkpoint failed. Automatic undo \
             is unavailable; inspect the workspace before retrying: {}",
            self.0
        )
    }
}

impl std::error::Error for AbsorbCheckpointError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl AbsorbExecutionOutcome {
    pub fn rejected_count(&self) -> usize {
        self.rejected.len()
    }

    pub fn is_success(&self) -> bool {
        self.rejected.is_empty()
    }
}

#[derive(Debug)]
struct RejectedAbsorption {
    commit_id: gix::ObjectId,
    commit_summary: String,
    path: bstr::BString,
    hunk_headers: Vec<but_core::HunkHeader>,
    reason: AbsorptionReason,
}

impl std::fmt::Display for AbsorbExecutionOutcome {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            out,
            "Absorb rejected {} selected file group{}; no changes were published.",
            self.rejected.len(),
            if self.rejected.len() == 1 { "" } else { "s" }
        )?;
        for rejected in &self.rejected {
            let ranges = rejected
                .hunk_headers
                .iter()
                .map(|header| {
                    format!(
                        "-{},{} +{},{}",
                        header.old_start, header.old_lines, header.new_start, header.new_lines
                    )
                })
                .join(", ");
            writeln!(
                out,
                "- {} {} -> {} ({}): {}",
                rejected.path,
                ranges,
                rejected.commit_summary,
                rejected.commit_id,
                rejected.reason.description()
            )?;
        }
        write!(
            out,
            "Refresh the absorb plan and inspect the listed dependencies before retrying."
        )
    }
}

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
    let snapshot = but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
        ctx,
        SnapshotDetails::new(OperationKind::Absorb),
        guard.read_permission(),
    )
    .context("Failed to prepare absorb rollback checkpoint")?;
    let outcome = absorb_with_checkpoint_with_perm(
        ctx,
        absorption_plan,
        guard.write_permission(),
        Some(snapshot),
    )?;
    anyhow::ensure!(outcome.is_success(), outcome);
    Ok(0)
}

/// Absorb the changes described by `absorption_plan` using the exclusive repository
/// access granted by `perm`, applying the updates through the modern commit amend API.
///
/// Returns the number of original (commit, path) groups with rejected diff specs.
pub fn absorb_with_perm(
    ctx: &mut Context,
    absorption_plan: Vec<CommitAbsorption>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<AbsorbExecutionOutcome> {
    absorb_with_checkpoint_with_perm(ctx, absorption_plan, perm, None)
}

pub fn absorb_with_checkpoint_with_perm(
    ctx: &mut Context,
    absorption_plan: Vec<CommitAbsorption>,
    perm: &mut RepoExclusive,
    checkpoint: Option<but_oplog::UnmaterializedOplogSnapshot>,
) -> anyhow::Result<AbsorbExecutionOutcome> {
    let context_lines = ctx.settings.context_lines;
    anyhow::ensure!(
        absorption_plan.is_empty()
            || absorption_plan
                .iter()
                .all(|absorption| absorption.source_snapshot_tree.is_some()),
        "Absorb plan is stale: it has no source snapshot; create a new plan"
    );
    let current_source_snapshot = source_snapshot_tree(ctx, perm.read_permission())?;
    for absorption in &absorption_plan {
        if let Some(planned_precondition) = absorption.source_snapshot_tree {
            let current_precondition = absorption_precondition_tree(
                ctx,
                current_source_snapshot,
                absorption,
                context_lines,
            )?;
            let changed_components = changed_source_components(
                &*ctx.repo.get()?,
                planned_precondition,
                current_precondition,
            )?;
            anyhow::ensure!(
                current_precondition == planned_precondition,
                "Absorb plan is stale: its source, routing, target, selection, or settings \
                 changed after planning (expected {planned_precondition}, current \
                 {current_precondition}, changed components: {changed_components:?})"
            );
        }
    }
    let record_checkpoint = checkpoint.is_some();
    let checkpoint = match checkpoint {
        Some(checkpoint) => checkpoint,
        None => but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
            ctx,
            SnapshotDetails::new(OperationKind::Absorb),
            perm.read_permission(),
        )
        .context("Failed to prepare absorb rollback checkpoint")?,
    };
    let mut meta = ctx.meta()?;
    let (repo, mut workspace, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let source_hunks = but_core::worktree_hunks(&repo, context_lines)?;
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;
    let mut rebase = editor.rebase()?;

    // Apply each group to the in-memory rebase and track failures. Nothing is
    // materialized until every planned group has been interpreted.
    let mut rejected_groups = HashMap::new();
    let mut commit_map = CommitMap::default();
    let mut blank_commits = BTreeMap::new();
    let mut accepted_diff_specs = Vec::new();
    let absorption_plan = absorption_steps_for_application(absorption_plan, &source_hunks);

    for absorption in absorption_plan {
        let diff_specs = convert_hunks_to_diff_specs(&absorption.hunks)?;
        accepted_diff_specs.extend(diff_specs.iter().cloned());
        let rewritten_commits = rebase.history.commit_mappings();
        let commit_id = if let Some(blank_commit_ref) = &absorption.blank_commit_ref {
            let blank_commit = if let Some(blank_commit) = blank_commits.get(blank_commit_ref) {
                *blank_commit
            } else {
                let expected_target = rewritten_commits
                    .get(&absorption.commit_id)
                    .copied()
                    .unwrap_or(absorption.commit_id);
                let actual_target = rebase.reference_target(blank_commit_ref.as_ref())?;
                anyhow::ensure!(
                    actual_target == expected_target,
                    "Absorb plan is stale: {blank_commit_ref} moved from {expected_target} to {actual_target}"
                );
                let (next_rebase, selector) = but_workspace::commit::insert_blank_commit(
                    rebase.into_editor(),
                    InsertSide::Below,
                    RelativeTo::Reference(blank_commit_ref.clone()),
                )?;
                rebase = next_rebase;
                let blank_commit = rebase.lookup_pick(selector)?;
                blank_commits.insert(blank_commit_ref.clone(), blank_commit);
                blank_commit
            };
            commit_map.find_mapped_id(blank_commit)
        } else {
            rewritten_commits
                .get(&absorption.commit_id)
                .copied()
                .unwrap_or(absorption.commit_id)
        };
        let but_workspace::commit::CommitAmendOutcome {
            rebase: next_rebase,
            commit_selector,
            rejected_specs,
        } = but_workspace::commit::commit_amend_without_checkout_cancellation(
            rebase.into_editor(),
            commit_id,
            diff_specs,
            context_lines,
            ChangeSource::Head,
        )?;
        rebase = next_rebase;
        if !rejected_specs.is_empty() {
            tracing::warn!(?rejected_specs, "Failed to commit at least one hunk");
        }
        if let Some(commit_selector) = commit_selector {
            let new_commit = rebase.lookup_pick(commit_selector)?;
            commit_map.add_mapping(commit_id, new_commit);
        }
        for (_, spec) in rejected_specs {
            rejected_groups
                .entry((
                    absorption.commit_id,
                    absorption.blank_commit_ref.clone(),
                    spec.path.clone(),
                ))
                .or_insert_with(|| RejectedAbsorption {
                    commit_id: absorption.commit_id,
                    commit_summary: absorption.commit_summary.clone(),
                    path: spec.path,
                    hunk_headers: spec.hunk_headers,
                    reason: absorption.reason.clone(),
                });
        }
    }

    if !rejected_groups.is_empty() {
        return Ok(AbsorbExecutionOutcome {
            rejected: rejected_groups.into_values().collect(),
        });
    }

    let mut editor = rebase.into_editor();
    but_workspace::commit::cancel_consumed_changes(
        &mut editor,
        &ChangeSource::Head,
        accepted_diff_specs,
        &[],
        context_lines,
    )?;
    let rebase = editor.rebase()?;
    let materialization = rebase.materialize(Default::default()).map(drop);
    drop((repo, workspace, db, meta));
    if let Err(error) = materialization {
        return match checkpoint.rollback(ctx, perm) {
            Ok(()) => Err(error
                .context("Absorb materialization failed; the pre-invocation state was restored")),
            Err(rollback_error) => Err(rollback_error.context(format!(
                "Absorb materialization failed ({error:#}) and checkpoint rollback failed. \
                 State may be partially changed; inspect the workspace before retrying"
            ))),
        };
    }
    if record_checkpoint {
        checkpoint
            .commit(ctx, perm)
            .map_err(AbsorbCheckpointError)?;
    }
    crate::diff::changes_in_worktree_with_perm(
        ctx,
        ChangesSource::Head,
        true,
        perm.read_permission(),
    )
    .map_err(|source| AbsorbFinalizationError {
        source,
        undo_available: record_checkpoint,
    })?;
    Ok(AbsorbExecutionOutcome::default())
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
    let planned_source_snapshot = source_snapshot_tree(ctx, perm.read_permission())?;
    let (candidates, dependencies) = match target {
        AbsorptionTarget::Branch { branch_name } => {
            // Get all worktree changes, assignments, and dependencies
            // TODO: Ideally, there's a simpler way of getting the worktree changes without passing the context to it.
            // At this time, the context is passed pretty deep into the function.
            let worktree_changes =
                crate::diff::changes_in_worktree_without_persisting_assignments_with_perm(
                    ctx,
                    ChangesSource::Head,
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
            let worktree_changes =
                crate::diff::changes_in_worktree_without_persisting_assignments_with_perm(
                    ctx,
                    ChangesSource::Head,
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
            let worktree_changes =
                crate::diff::changes_in_worktree_without_persisting_assignments_with_perm(
                    ctx,
                    ChangesSource::Head,
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

    ensure_supported_candidate_paths(ctx, &candidates)?;

    // Group all changes by their target commit
    let changes_by_commit =
        group_changes_by_target_commit(ctx, &candidates, dependencies.as_ref())?;

    // Prepare commit absorptions for display
    let mut commit_absorptions = prepare_commit_absorptions(ctx, changes_by_commit)?;
    anyhow::ensure!(
        source_snapshot_tree(ctx, perm.read_permission())? == planned_source_snapshot,
        "Absorb plan became stale while it was being created; retry planning"
    );
    for absorption in &mut commit_absorptions {
        absorption.source_snapshot_tree = Some(absorption_precondition_tree(
            ctx,
            planned_source_snapshot,
            absorption,
            ctx.settings.context_lines,
        )?);
    }

    Ok(commit_absorptions)
}

fn ensure_supported_candidate_paths(
    ctx: &Context,
    candidates: &[AbsorbCandidate],
) -> anyhow::Result<()> {
    let non_utf8_paths = candidates
        .iter()
        .filter(|candidate| candidate.hunk.path.to_str().is_err())
        .map(|candidate| format!("{:?}", candidate.hunk.path))
        .collect::<Vec<_>>();
    anyhow::ensure!(
        non_utf8_paths.is_empty(),
        "Absorb does not yet support non-UTF-8 paths: {}. Commit or revert those changes before retrying.",
        non_utf8_paths.iter().join(", ")
    );

    let repo = ctx.repo.get()?;
    let renamed_paths = but_core::diff::worktree_changes(&repo)?
        .changes
        .into_iter()
        .filter(|change| {
            matches!(change.status, but_core::TreeStatus::Rename { .. })
                && candidates
                    .iter()
                    .any(|candidate| candidate.hunk.path == change.path)
        })
        .map(|change| change.path.to_str_lossy().into_owned())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        renamed_paths.is_empty(),
        "Absorb does not yet support renamed paths: {}. Commit or revert the rename before retrying.",
        renamed_paths.iter().join(", ")
    );
    Ok(())
}

/// Group changes by their target commit based on dependencies and assignments
fn group_changes_by_target_commit(
    ctx: &mut Context,
    candidates: &[AbsorbCandidate],
    dependencies: Option<&HunkDependencies>,
) -> anyhow::Result<GroupedChanges> {
    let mut changes_by_commit: GroupedChanges = BTreeMap::new();
    let source_hunks = {
        let repo = ctx.repo.get()?;
        but_core::worktree_hunks(&repo, ctx.settings.context_lines)?
    };

    let workspace = crate::legacy::workspace::head_info(ctx)?;

    // Build an index for O(1) lock lookups per candidate
    let lock_index = dependencies.map(build_lock_index);

    let candidate_hunks = candidates
        .iter()
        .map(|candidate| &candidate.hunk)
        .collect::<Vec<_>>();
    for candidate_indices in hunk_groups_by_source(&candidate_hunks, &source_hunks) {
        let grouped_candidates = candidate_indices
            .into_iter()
            .map(|index| &candidates[index])
            .collect::<Vec<_>>();
        let candidate = grouped_candidates[0];
        if grouped_candidates.iter().any(|other| {
            other.stack_id != candidate.stack_id || other.branch_ref != candidate.branch_ref
        }) {
            anyhow::bail!(
                "Coupled hunk selections have conflicting assignments in path: {}",
                candidate.hunk.path
            );
        }
        let routing_candidate = grouped_candidates
            .iter()
            .copied()
            .find(|candidate| {
                candidate
                    .hunk
                    .hunk_header
                    .is_some_and(|header| header.new_lines > 0)
            })
            .unwrap_or(candidate);
        let locks = lock_index
            .as_ref()
            .map(|idx| locks_for_candidate(idx, routing_candidate))
            .filter(|l| !l.is_empty());
        let (stack_id, commit_id, reason, blank_commit_ref) =
            ensure_target_commit(ctx, candidate, locks.as_deref(), &workspace)?;

        let entry = changes_by_commit
            .entry((stack_id, commit_id, blank_commit_ref))
            .or_insert_with(|| (Vec::new(), reason.clone()));

        entry.0.extend(grouped_candidates.into_iter().cloned());
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
                let overlaps = ranges_overlap(
                    dep_hunk.new_start,
                    dep_hunk.new_lines,
                    hunk_header.new_start,
                    hunk_header.new_lines,
                ) || (dep_hunk.new_lines == 0
                    && hunk_header.new_lines < hunk_header.old_lines
                    && dep_hunk.new_start
                        == hunk_header.new_start.saturating_add(hunk_header.new_lines));
                if overlaps {
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

// Find the child-most lock when every lock belongs to the same identified stack.
fn find_top_most_lock<'a>(locks: &'a [HunkLock], workspace: &RefInfo) -> Option<&'a HunkLock> {
    let stack_id = unique_lock_stack_id(locks)?;
    let stack = stack_by_id(workspace, stack_id)?;

    // Dependency entries are grouped by changed ranges, not ordered by the
    // workspace's application order. Walk only this stack so parallel stacks
    // cannot be mistaken for ancestors and descendants of one another.
    for segment in &stack.segments {
        for commit in &segment.commits {
            if let Some(lock) = locks.iter().find(|lock| {
                lock.commit_id == commit.id && lock.target == HunkLockTarget::Stack(stack_id)
            }) {
                return Some(lock);
            }
        }
    }

    None
}

fn unique_lock_stack_id(locks: &[HunkLock]) -> Option<StackId> {
    let mut stack_ids = locks.iter().map(|lock| match lock.target {
        HunkLockTarget::Stack(stack_id) => Some(stack_id),
        HunkLockTarget::Unidentified => None,
    });
    let stack_id = stack_ids.next()??;
    stack_ids
        .all(|candidate| candidate == Some(stack_id))
        .then_some(stack_id)
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
    workspace: &RefInfo,
) -> anyhow::Result<(
    but_core::ref_metadata::StackId,
    gix::ObjectId,
    AbsorptionReason,
    Option<gix::refs::FullName>,
)> {
    // Priority 1: Check if there's a dependency lock for this hunk
    if let Some(locks) = locks {
        if let Some(lock) = find_top_most_lock(locks, workspace) {
            if let HunkLockTarget::Stack(stack_id) = lock.target {
                return Ok((
                    stack_id,
                    lock.commit_id,
                    AbsorptionReason::HunkDependency,
                    None,
                ));
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
            return Ok((stack_id, commit_id, AbsorptionReason::StackAssignment, None));
        }
        let anchor = ctx
            .repo
            .get()?
            .find_reference(reference.as_ref())?
            .peel_to_id()?
            .detach();
        return Ok((
            stack_id,
            anchor,
            AbsorptionReason::StackAssignment,
            Some(reference),
        ));
    }

    // Priority 3: If no assignment, find the topmost commit of the leftmost lane
    if let Some(stack_id) = workspace.stacks.first().and_then(|stack| stack.id) {
        let (reference, commit_id) = target_segment(workspace, stack_id, None)?;
        if let Some(commit_id) = commit_id {
            return Ok((stack_id, commit_id, AbsorptionReason::DefaultStack, None));
        }
        let anchor = ctx
            .repo
            .get()?
            .find_reference(reference.as_ref())?
            .peel_to_id()?
            .detach();
        return Ok((
            stack_id,
            anchor,
            AbsorptionReason::DefaultStack,
            Some(reference),
        ));
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
        .map(|(stack_id, _, _)| *stack_id)
        .unique()
        .collect::<Vec<_>>();

    // Iterate through the stacks' commits in application order (parent to child)
    for stack_id in all_stack_ids {
        let stack = stack_by_id(&workspace, stack_id)
            .with_context(|| format!("Couldn't find {stack_id} in the current workspace"))?;
        for segment in stack.segments.iter().rev() {
            for commit in segment.commits.iter().rev() {
                let key = (stack_id, commit.id, None);
                if let Some((candidates, reason)) = changes_by_commit.get(&key) {
                    let hunks = candidates
                        .iter()
                        .map(|candidate| candidate.hunk.clone())
                        .collect();
                    commit_absorptions.push(CommitAbsorption {
                        stack_id,
                        commit_id: commit.id,
                        blank_commit_ref: None,
                        source_snapshot_tree: None,
                        commit_summary: get_commit_summary(&*ctx.repo.get()?, commit.id)?,
                        hunks,
                        reason: reason.clone(),
                    });
                }
            }
        }
    }

    for ((stack_id, commit_id, blank_commit_ref), (candidates, reason)) in &changes_by_commit {
        let Some(blank_commit_ref) = blank_commit_ref else {
            continue;
        };
        commit_absorptions.push(CommitAbsorption {
            stack_id: *stack_id,
            commit_id: *commit_id,
            blank_commit_ref: Some(blank_commit_ref.clone()),
            source_snapshot_tree: None,
            commit_summary: "New commit".into(),
            hunks: candidates
                .iter()
                .map(|candidate| candidate.hunk.clone())
                .collect(),
            reason: reason.clone(),
        });
    }

    Ok(commit_absorptions)
}

/// Preserve selectors from one source diff hunk as one amendment so they retain
/// their shared coordinate space.
fn absorption_steps_for_application(
    absorptions: Vec<CommitAbsorption>,
    source_hunks: &[but_core::SingleHunk],
) -> Vec<CommitAbsorption> {
    absorptions
        .into_iter()
        .flat_map(|absorption| {
            let selected_hunks = absorption.hunks.iter().collect::<Vec<_>>();
            hunk_groups_by_source(&selected_hunks, source_hunks)
                .into_iter()
                .map(move |indices| CommitAbsorption {
                    stack_id: absorption.stack_id,
                    commit_id: absorption.commit_id,
                    blank_commit_ref: absorption.blank_commit_ref.clone(),
                    source_snapshot_tree: absorption.source_snapshot_tree,
                    commit_summary: absorption.commit_summary.clone(),
                    hunks: indices
                        .into_iter()
                        .map(|index| absorption.hunks[index].clone())
                        .collect(),
                    reason: absorption.reason.clone(),
                })
        })
        .collect()
}

fn source_snapshot_tree(
    ctx: &Context,
    perm: &but_core::sync::RepoShared,
) -> anyhow::Result<gix::ObjectId> {
    let snapshot_tree = ctx.prepare_snapshot(perm)?;
    let mut assignments = ctx
        .db
        .get_cache()?
        .hunk_assignments()
        .list_all()?
        .into_iter()
        .map(|assignment| serde_json::to_vec(&assignment))
        .collect::<Result<Vec<_>, _>>()?;
    assignments.sort();

    let repo = ctx.repo.get()?;
    let assignments_blob = repo.write_blob(serde_json::to_vec(&assignments)?)?;
    let metadata_path = ctx.project_data_dir().join("virtual_branches.toml");
    let metadata = match std::fs::read(metadata_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err.into()),
    };
    let metadata_blob = repo.write_blob(metadata)?;
    let mut source_tree = repo.find_tree(snapshot_tree)?.edit()?;
    source_tree.upsert(
        "virtual_branches.toml",
        gix::object::tree::EntryKind::Blob,
        metadata_blob,
    )?;
    source_tree.upsert(
        "hunk-assignments",
        gix::object::tree::EntryKind::Blob,
        assignments_blob,
    )?;
    Ok(source_tree.write()?.detach())
}

fn absorption_precondition_tree(
    ctx: &Context,
    source_snapshot_tree: gix::ObjectId,
    absorption: &CommitAbsorption,
    context_lines: u32,
) -> anyhow::Result<gix::ObjectId> {
    let repo = ctx.repo.get()?;
    let precondition_blob = repo.write_blob(serde_json::to_vec(&serde_json::json!({
        "stackId": absorption.stack_id.to_string(),
        "commitId": absorption.commit_id.to_string(),
        "blankCommitRef": absorption.blank_commit_ref.as_ref().map(ToString::to_string),
        "commitSummary": absorption.commit_summary,
        "hunks": absorption.hunks,
        "reason": absorption.reason,
        "contextLines": context_lines,
    }))?)?;
    let mut precondition_tree = repo.find_tree(source_snapshot_tree)?.edit()?;
    precondition_tree.upsert(
        "absorb-plan",
        gix::object::tree::EntryKind::Blob,
        precondition_blob,
    )?;
    Ok(precondition_tree.write()?.detach())
}

fn changed_source_components(
    repo: &gix::Repository,
    expected: gix::ObjectId,
    current: gix::ObjectId,
) -> anyhow::Result<Vec<bstr::BString>> {
    let entries = |tree_id| -> anyhow::Result<BTreeMap<bstr::BString, gix::ObjectId>> {
        repo.find_tree(tree_id)?
            .iter()
            .map(|entry| {
                let entry = entry?;
                Ok((entry.filename().to_owned(), entry.id().detach()))
            })
            .collect()
    };
    let expected = entries(expected)?;
    let current = entries(current)?;
    Ok(expected
        .keys()
        .chain(current.keys())
        .unique()
        .filter(|name| expected.get(*name) != current.get(*name))
        .cloned()
        .collect())
}

fn hunk_groups_by_source(
    selected_hunks: &[&but_core::SingleHunk],
    source_hunks: &[but_core::SingleHunk],
) -> Vec<Vec<usize>> {
    let mut groups: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
    for (selected_index, selected) in selected_hunks.iter().enumerate() {
        let source_index = source_hunks
            .iter()
            .position(|source| hunk_is_within_source(selected, source));
        if let Some((_, indices)) = groups
            .iter_mut()
            .find(|(group_source, _)| source_index.is_some() && *group_source == source_index)
        {
            indices.push(selected_index);
        } else {
            groups.push((source_index, vec![selected_index]));
        }
    }
    groups.into_iter().map(|(_, indices)| indices).collect()
}

fn hunk_is_within_source(selected: &but_core::SingleHunk, source: &but_core::SingleHunk) -> bool {
    if selected.path != source.path {
        return false;
    }
    let (Some(selected), Some(source)) = (selected.hunk_header, source.hunk_header) else {
        return selected.hunk_header.is_none() && source.hunk_header.is_none();
    };
    let side_is_within = |start: u32, lines: u32, source_start: u32, source_lines: u32| {
        lines == 0
            || (source_lines > 0
                && source_start <= start
                && start.saturating_add(lines) <= source_start.saturating_add(source_lines))
    };
    side_is_within(
        selected.old_start,
        selected.old_lines,
        source.old_start,
        source.old_lines,
    ) && side_is_within(
        selected.new_start,
        selected.new_lines,
        source.new_start,
        source.new_lines,
    )
}

/// Get the commit summary message
fn get_commit_summary(repo: &gix::Repository, commit_id: gix::ObjectId) -> anyhow::Result<String> {
    let commit = repo.find_commit(commit_id)?;
    // The title still carries the trailing newline of single-paragraph messages.
    let message = commit.message()?.title.trim_end().as_bstr().to_string();
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use but_core::RefMetadata as _;

    fn stamp_plan(ctx: &mut Context, plan: &mut [CommitAbsorption]) -> anyhow::Result<()> {
        let context_lines = ctx.settings.context_lines;
        let guard = ctx.exclusive_worktree_access();
        let source_snapshot_tree = source_snapshot_tree(ctx, guard.read_permission())?;
        for absorption in plan {
            absorption.source_snapshot_tree = Some(absorption_precondition_tree(
                ctx,
                source_snapshot_tree,
                absorption,
                context_lines,
            )?);
        }
        Ok(())
    }

    #[derive(Debug, PartialEq, Eq)]
    struct WorktreeFileState {
        bytes: Vec<u8>,
        is_file: bool,
        is_symlink: bool,
        symlink_target: Option<std::path::PathBuf>,
        #[cfg(unix)]
        mode: u32,
    }

    #[derive(Debug, PartialEq)]
    struct AbsorbInvocationState {
        worktree_files: Vec<(String, Option<WorktreeFileState>)>,
        index: String,
        git_status: String,
        head_name: Option<gix::refs::FullName>,
        refs: Vec<(String, Option<gix::ObjectId>)>,
        commits: Vec<CommitState>,
        project_meta: but_core::ref_metadata::ProjectMeta,
        workspace_meta: but_core::ref_metadata::Workspace,
        assignments: Vec<but_db::HunkAssignment>,
        oplog_head: Option<gix::ObjectId>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct CommitState {
        reference: String,
        parent_ids: Vec<gix::ObjectId>,
        blobs: Vec<(String, Option<Vec<u8>>)>,
    }

    fn reference_id(
        repo: &gix::Repository,
        reference: &str,
    ) -> anyhow::Result<Option<gix::ObjectId>> {
        let Some(mut reference) = repo.try_find_reference(reference)? else {
            return Ok(None);
        };
        Ok(Some(reference.peel_to_id()?.detach()))
    }

    fn worktree_file_state(path: &std::path::Path) -> anyhow::Result<Option<WorktreeFileState>> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        Ok(Some(WorktreeFileState {
            bytes: std::fs::read(path)?,
            is_file: metadata.file_type().is_file(),
            is_symlink: metadata.file_type().is_symlink(),
            symlink_target: metadata
                .file_type()
                .is_symlink()
                .then(|| std::fs::read_link(path))
                .transpose()?,
            #[cfg(unix)]
            mode: {
                use std::os::unix::fs::PermissionsExt;

                metadata.permissions().mode()
            },
        }))
    }

    fn absorb_invocation_state(
        ctx: &mut Context,
        worktree: &std::path::Path,
        worktree_paths: &[&str],
        references: &[&str],
        commit_paths: &[(&str, &[&str])],
    ) -> anyhow::Result<AbsorbInvocationState> {
        let mut worktree_files = worktree_paths
            .iter()
            .map(|path| {
                Ok((
                    (*path).to_owned(),
                    worktree_file_state(&worktree.join(path))?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        worktree_files.sort_by(|left, right| left.0.cmp(&right.0));
        let (index, git_status, head_name, refs, commits) = {
            let repo = ctx.repo.get()?;
            let index = repo.open_index()?;
            (
                but_testsupport::visualize_index_with_content(&repo, &index),
                but_testsupport::git_status(&repo)?,
                repo.head_name()?.map(|name| name.to_owned()),
                references
                    .iter()
                    .map(|reference| Ok(((*reference).to_owned(), reference_id(&repo, reference)?)))
                    .collect::<anyhow::Result<Vec<_>>>()?,
                commit_paths
                    .iter()
                    .map(|(reference, paths)| {
                        let commit = repo.find_commit(repo.rev_parse_single(*reference)?)?;
                        let tree = commit.tree()?;
                        let blobs = paths
                            .iter()
                            .map(|path| {
                                let blob = tree
                                    .lookup_entry_by_path(path)?
                                    .map(|entry| -> anyhow::Result<Vec<u8>> {
                                        Ok(entry.object()?.into_blob().data.to_vec())
                                    })
                                    .transpose()?;
                                Ok(((*path).to_owned(), blob))
                            })
                            .collect::<anyhow::Result<Vec<_>>>()?;
                        Ok(CommitState {
                            reference: (*reference).to_owned(),
                            parent_ids: commit.parent_ids().map(|id| id.detach()).collect(),
                            blobs,
                        })
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )
        };
        let mut assignments = ctx.db.get_cache()?.hunk_assignments().list_all()?;
        assignments.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.hunk_header.cmp(&right.hunk_header))
        });
        let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
        let workspace_meta = {
            let metadata = ctx.meta()?;
            let workspace = metadata.workspace(workspace_ref.as_ref())?;
            (*workspace).clone()
        };

        Ok(AbsorbInvocationState {
            worktree_files,
            index,
            git_status,
            head_name,
            refs,
            commits,
            project_meta: ctx.project_meta()?,
            workspace_meta,
            assignments,
            oplog_head: ctx.oplog_head()?,
        })
    }

    fn state_summary(state: &AbsorbInvocationState) -> String {
        let worktree_files = state
            .worktree_files
            .iter()
            .map(|(path, file)| {
                let Some(file) = file else {
                    return format!("{path}: absent");
                };
                let mode: Option<u32> = {
                    #[cfg(unix)]
                    {
                        Some(file.mode)
                    }
                    #[cfg(not(unix))]
                    {
                        None
                    }
                };
                format!(
                    "{path}: bytes={:?}, file={}, symlink={}, target={:?}, mode={:?}",
                    String::from_utf8_lossy(&file.bytes),
                    file.is_file,
                    file.is_symlink,
                    file.symlink_target,
                    mode,
                )
            })
            .collect::<Vec<_>>();
        let commits = state
            .commits
            .iter()
            .map(|commit| {
                (
                    &commit.reference,
                    &commit.parent_ids,
                    commit
                        .blobs
                        .iter()
                        .map(|(path, bytes)| (path, bytes.as_deref().map(String::from_utf8_lossy)))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        format!(
            "worktree files: {worktree_files:#?}\n\
             index: {:?}\n\
             status: {:?}\n\
             HEAD: {:?}\n\
             refs: {:#?}\n\
             commits: {commits:#?}\n\
             project metadata: {:#?}\n\
             workspace metadata: {:#?}\n\
             assignments: {:#?}\n\
             oplog head: {:?}",
            state.index,
            state.git_status,
            state.head_name,
            state.refs,
            state.project_meta,
            state.workspace_meta,
            state.assignments,
            state.oplog_head,
        )
    }

    #[test]
    fn applicable_hunk_is_absorbed_through_public_planning_and_execution() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-rejected-hunks");
        let expected_worktree_content = (1..=20)
            .map(|line| match line {
                1 => "unselected change\n".to_owned(),
                10 => "selected change\n".to_owned(),
                _ => format!("line {line}\n"),
            })
            .collect::<String>();
        let expected_target_content = (1..=20)
            .map(|line| match line {
                10 => "selected change\n".to_owned(),
                _ => format!("line {line}\n"),
            })
            .collect::<String>();
        let feature_commit = repo.head_id()?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;

        let plan = absorption_plan(
            &mut ctx,
            AbsorptionTarget::Hunks {
                hunks: vec![but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 10,
                        old_lines: 1,
                        new_start: 10,
                        new_lines: 1,
                    }),
                    path: "shared.txt".into(),
                    diff: None,
                }],
            },
        )?;

        assert_eq!(
            plan.len(),
            1,
            "the selected hunk produces one planned target"
        );
        assert_eq!(
            plan[0].commit_id, feature_commit,
            "planning routes the selected line to the feature commit"
        );

        let rejected = absorb(&mut ctx, plan)?;

        assert_eq!(rejected, 0, "the applicable hunk is not rejected");
        let repo = ctx.repo.get()?;
        let tree = repo
            .find_commit(repo.rev_parse_single("refs/heads/feature")?)?
            .tree()?;
        let blob = tree
            .lookup_entry_by_path("shared.txt")?
            .expect("committed file")
            .object()?
            .into_blob();
        assert_eq!(
            blob.data,
            expected_target_content.as_bytes(),
            "the selected line is absorbed into the planned commit"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("shared.txt"))?,
            expected_worktree_content,
            "the worktree retains its exact user-visible bytes"
        );
        Ok(())
    }

    #[cfg(unix)]
    fn absorb_file_types_fixture() -> anyhow::Result<(Context, tempfile::TempDir)> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-file-types");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        Ok((ctx, tmp))
    }

    #[cfg(unix)]
    fn reset_absorb_file_types_fixture(tmp: &tempfile::TempDir) -> anyhow::Result<()> {
        use std::os::unix::{ffi::OsStrExt as _, fs::PermissionsExt as _};

        std::fs::rename(
            tmp.path().join("renamed-new.txt"),
            tmp.path().join("renamed-old.txt"),
        )?;
        std::fs::write(tmp.path().join("binary.dat"), b"\0\x01old\xff")?;
        let mode_path = tmp.path().join("mode.sh");
        let mut permissions = std::fs::metadata(&mode_path)?.permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(mode_path, permissions)?;
        let non_utf8_path = std::path::Path::new(std::ffi::OsStr::from_bytes(b"invalid-\xff.txt"));
        std::fs::write(tmp.path().join(non_utf8_path), b"old non-utf8 path\n")?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn renamed_path_is_rejected_without_changing_the_invocation() -> anyhow::Result<()> {
        use std::os::unix::ffi::OsStrExt as _;

        let (mut ctx, tmp) = absorb_file_types_fixture()?;
        let non_utf8_path = std::path::Path::new(std::ffi::OsStr::from_bytes(b"invalid-\xff.txt"));
        std::fs::write(tmp.path().join(non_utf8_path), b"old non-utf8 path\n")?;
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &[
                "renamed-old.txt",
                "renamed-new.txt",
                "binary.dat",
                "mode.sh",
            ],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[(
                "refs/heads/feature",
                &[
                    "renamed-old.txt",
                    "renamed-new.txt",
                    "binary.dat",
                    "mode.sh",
                ],
            )],
        )?;

        let error = absorption_plan(&mut ctx, AbsorptionTarget::All)
            .expect_err("rename planning must fail before previous_path is discarded");

        assert!(
            error
                .to_string()
                .contains("does not yet support renamed paths: renamed-new.txt"),
            "rename rejection identifies the affected path: {error:#}"
        );
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &[
                "renamed-old.txt",
                "renamed-new.txt",
                "binary.dat",
                "mode.sh",
            ],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[(
                "refs/heads/feature",
                &[
                    "renamed-old.txt",
                    "renamed-new.txt",
                    "binary.dat",
                    "mode.sh",
                ],
            )],
        )?;
        assert_eq!(
            after, before,
            "rename rejection preserves refs, commits, index, worktree, metadata, assignments, and oplog"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_is_rejected_without_changing_the_invocation() -> anyhow::Result<()> {
        let (mut ctx, tmp) = absorb_file_types_fixture()?;
        std::fs::rename(
            tmp.path().join("renamed-new.txt"),
            tmp.path().join("renamed-old.txt"),
        )?;
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &[
                "renamed-old.txt",
                "renamed-new.txt",
                "binary.dat",
                "mode.sh",
            ],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[(
                "refs/heads/feature",
                &[
                    "renamed-old.txt",
                    "renamed-new.txt",
                    "binary.dat",
                    "mode.sh",
                ],
            )],
        )?;

        let error = absorption_plan(&mut ctx, AbsorptionTarget::All)
            .expect_err("non-UTF-8 planning must fail before path bytes are lost");

        assert!(
            error
                .to_string()
                .contains("does not yet support non-UTF-8 paths"),
            "non-UTF-8 rejection identifies the unsupported path class: {error:#}"
        );
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &[
                "renamed-old.txt",
                "renamed-new.txt",
                "binary.dat",
                "mode.sh",
            ],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[(
                "refs/heads/feature",
                &[
                    "renamed-old.txt",
                    "renamed-new.txt",
                    "binary.dat",
                    "mode.sh",
                ],
            )],
        )?;
        assert_eq!(
            after, before,
            "non-UTF-8 rejection preserves refs, commits, index, worktree, metadata, assignments, and oplog"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn binary_and_mode_changes_preserve_bytes_and_modes() -> anyhow::Result<()> {
        use std::os::unix::{ffi::OsStrExt as _, fs::PermissionsExt as _};

        let (mut ctx, tmp) = absorb_file_types_fixture()?;
        std::fs::rename(
            tmp.path().join("renamed-new.txt"),
            tmp.path().join("renamed-old.txt"),
        )?;
        let non_utf8_path = std::path::Path::new(std::ffi::OsStr::from_bytes(b"invalid-\xff.txt"));
        std::fs::write(tmp.path().join(non_utf8_path), b"old non-utf8 path\n")?;

        let plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        assert_eq!(
            plan.len(),
            1,
            "binary and mode changes route to the feature commit"
        );
        assert_eq!(
            absorb(&mut ctx, plan)?,
            0,
            "binary and mode changes are accepted"
        );

        let repo = ctx.repo.get()?;
        let tree = repo
            .find_commit(repo.rev_parse_single("refs/heads/feature")?)?
            .tree()?;
        assert_eq!(
            tree.lookup_entry_by_path("binary.dat")?
                .expect("binary file exists")
                .object()?
                .into_blob()
                .data,
            b"\0\x02new\xfe",
            "binary bytes are absorbed without text conversion"
        );
        assert_eq!(
            tree.lookup_entry_by_path("mode.sh")?
                .expect("mode-only file exists")
                .mode()
                .kind(),
            gix::object::tree::EntryKind::BlobExecutable,
            "the executable bit is absorbed"
        );
        assert_eq!(
            tree.lookup_entry_by_path(non_utf8_path)?
                .expect("unchanged non-UTF-8 path exists")
                .object()?
                .into_blob()
                .data,
            b"old non-utf8 path\n",
            "absorbing other paths preserves the non-UTF-8 tree entry"
        );
        assert_eq!(
            std::fs::read(tmp.path().join("binary.dat"))?,
            b"\0\x02new\xfe",
            "successful absorb preserves binary worktree bytes"
        );
        assert_ne!(
            std::fs::metadata(tmp.path().join("mode.sh"))?
                .permissions()
                .mode()
                & 0o111,
            0,
            "successful absorb preserves the executable worktree mode"
        );
        let non_utf8_worktree_path = tmp.path().join(non_utf8_path);
        assert_eq!(
            std::fs::read(non_utf8_worktree_path)?,
            b"old non-utf8 path\n",
            "successful absorb preserves an unchanged non-UTF-8 worktree path"
        );
        assert!(
            but_core::worktree_hunks(&repo, 0)?.is_empty(),
            "all file-type changes are absent from the residual worktree diff"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn copied_file_is_absorbed_as_addition_without_changing_source() -> anyhow::Result<()> {
        let (mut ctx, tmp) = absorb_file_types_fixture()?;
        reset_absorb_file_types_fixture(&tmp)?;
        std::fs::copy(
            tmp.path().join("renamed-old.txt"),
            tmp.path().join("copied.txt"),
        )?;

        let plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        assert_eq!(plan.len(), 1, "the copied file has one addition target");
        assert_eq!(
            absorb(&mut ctx, plan)?,
            0,
            "the copy-like addition is accepted"
        );

        let repo = ctx.repo.get()?;
        let tree = repo
            .find_commit(repo.rev_parse_single("refs/heads/feature")?)?
            .tree()?;
        for path in ["renamed-old.txt", "copied.txt"] {
            assert_eq!(
                tree.lookup_entry_by_path(path)?
                    .unwrap_or_else(|| panic!("{path} exists"))
                    .object()?
                    .into_blob()
                    .data,
                b"base\n",
                "copy-like addition preserves both source and destination bytes"
            );
        }
        assert!(
            but_core::worktree_hunks(&repo, 0)?.is_empty(),
            "the accepted addition leaves no residual diff"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn large_binary_addition_preserves_exact_bytes() -> anyhow::Result<()> {
        let (mut ctx, tmp) = absorb_file_types_fixture()?;
        reset_absorb_file_types_fixture(&tmp)?;
        let large_bytes = (0..2 * 1024 * 1024)
            .map(|offset| (offset % 251) as u8)
            .collect::<Vec<_>>();
        std::fs::write(tmp.path().join("large.dat"), &large_bytes)?;

        let plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        assert_eq!(plan.len(), 1, "the large binary has one addition target");
        assert_eq!(absorb(&mut ctx, plan)?, 0, "the large binary is accepted");

        let repo = ctx.repo.get()?;
        let tree = repo
            .find_commit(repo.rev_parse_single("refs/heads/feature")?)?
            .tree()?;
        assert_eq!(
            tree.lookup_entry_by_path("large.dat")?
                .expect("large binary exists")
                .object()?
                .into_blob()
                .data,
            large_bytes,
            "the target tree contains every large-file byte"
        );
        assert_eq!(
            std::fs::read(tmp.path().join("large.dat"))?,
            large_bytes,
            "successful absorb preserves every worktree byte"
        );
        assert!(
            but_core::worktree_hunks(&repo, 0)?.is_empty(),
            "the accepted large binary leaves no residual diff"
        );
        Ok(())
    }

    #[test]
    fn merge_descendant_is_rewritten_without_losing_parents() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-merge-history");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let (merge_before_id, parents_before) = {
            let merge_before = repo.head_commit()?;
            (
                merge_before.id,
                merge_before
                    .parent_ids()
                    .map(|parent| parent.detach())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            parents_before.len(),
            2,
            "the fixture head is a merge commit"
        );
        let feature_before = parents_before[0];
        let side_before = parents_before[1];
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 3;

        let plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        assert_eq!(plan.len(), 1, "the feature edit has one dependency target");
        assert_eq!(
            plan[0].commit_id, feature_before,
            "the edit routes to the pre-merge feature commit"
        );
        assert_eq!(absorb(&mut ctx, plan)?, 0, "the merge history is accepted");

        let repo = ctx.repo.get()?;
        let merge_after = repo.find_commit(repo.rev_parse_single("refs/heads/feature")?)?;
        assert_ne!(
            merge_after.id, merge_before_id,
            "the feature ref advances to the rewritten merge descendant"
        );
        let parents_after = merge_after.parent_ids().collect::<Vec<_>>();
        assert_eq!(
            parents_after.len(),
            2,
            "the rewritten descendant remains a merge commit"
        );
        assert_ne!(
            parents_after[0], feature_before,
            "the amended feature parent receives the selected content"
        );
        assert_eq!(
            parents_after[1], side_before,
            "the independent side parent remains unchanged"
        );
        let tree = merge_after.tree()?;
        assert_eq!(
            tree.lookup_entry_by_path("feature.txt")?
                .expect("feature file exists")
                .object()?
                .into_blob()
                .data,
            b"feature new\n",
            "the selected feature bytes reach the rewritten merge"
        );
        assert_eq!(
            tree.lookup_entry_by_path("side.txt")?
                .expect("side file exists")
                .object()?
                .into_blob()
                .data,
            b"side content\n",
            "the merge retains independent side content"
        );
        assert_eq!(
            std::fs::read(tmp.path().join("feature.txt"))?,
            b"feature new\n",
            "successful absorb preserves worktree bytes"
        );
        assert!(
            but_core::worktree_hunks(&repo, 0)?.is_empty(),
            "the accepted merge-history edit leaves no residual diff"
        );
        Ok(())
    }

    #[test]
    fn resolved_merge_descendant_conflict_aborts_unchanged() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-resolved-merge-history");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let feature_before = repo
            .head_commit()?
            .parent_ids()
            .next()
            .expect("first parent")
            .detach();
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        assert_eq!(plan.len(), 1, "the resolved-merge edit has one target");
        assert_eq!(
            plan[0].commit_id, feature_before,
            "the edit routes to the pre-merge feature parent"
        );
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/heads/side",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[
                ("refs/heads/feature", &["shared.txt"]),
                ("refs/heads/side", &["shared.txt"]),
            ],
        )?;

        let error = absorb(&mut ctx, plan)
            .expect_err("a conflicting resolved-merge replay must abort rather than publish");
        assert!(
            error.to_string().contains("conflict"),
            "the graph rewrite identifies the descendant merge conflict: {error:#}"
        );
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/heads/side",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[
                ("refs/heads/feature", &["shared.txt"]),
                ("refs/heads/side", &["shared.txt"]),
            ],
        )?;
        assert_eq!(
            after, before,
            "a descendant rewrite conflict preserves the complete invocation state"
        );
        Ok(())
    }

    fn applicable_hunk_plan(ctx: &mut Context) -> anyhow::Result<Vec<CommitAbsorption>> {
        absorption_plan(
            ctx,
            AbsorptionTarget::Hunks {
                hunks: vec![but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 10,
                        old_lines: 1,
                        new_start: 10,
                        new_lines: 1,
                    }),
                    path: "shared.txt".into(),
                    diff: None,
                }],
            },
        )
    }

    fn absorb_recovery_fixture(
        persist_assignments: bool,
    ) -> anyhow::Result<(
        Context,
        tempfile::TempDir,
        Vec<CommitAbsorption>,
        AbsorbInvocationState,
    )> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-rejected-hunks");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        if persist_assignments {
            let changes = crate::diff::changes_in_worktree(&ctx, ChangesSource::Head, true)?;
            assert!(
                changes.assignments_error.is_none() && changes.assignments.len() == 2,
                "the recovery fixture persists both real worktree assignments before planning"
            );
        }
        let plan = applicable_hunk_plan(&mut ctx)?;
        let mut before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;
        if persist_assignments {
            for assignment in &mut before.assignments {
                assignment.id = None;
            }
        }
        Ok((ctx, tmp, plan, before))
    }

    #[test]
    fn index_lock_failure_before_publication_leaves_invocation_unchanged() -> anyhow::Result<()> {
        let (mut ctx, tmp, plan, before) = absorb_recovery_fixture(false)?;
        let index_path = ctx.repo.get()?.index_path();
        let external_lock = gix::lock::File::acquire_to_update_resource(
            &index_path,
            gix::lock::acquire::Fail::Immediately,
            None,
        )?;

        let error = absorb(&mut ctx, plan).expect_err("the external index lock must fail checkout");

        assert!(
            format!("{error:#}").contains("lock"),
            "the materialization failure identifies the index lock: {error:#}"
        );
        assert!(
            external_lock.lock_path().exists(),
            "absorb must not disturb the external index lock"
        );
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;
        assert_eq!(
            after, before,
            "materialization failure before publication leaves all observed state unchanged"
        );
        Ok(())
    }

    #[test]
    fn checkpoint_preparation_failure_leaves_invocation_unchanged() -> anyhow::Result<()> {
        let (mut ctx, tmp, plan, before) = absorb_recovery_fixture(false)?;
        let metadata_path = ctx.project_data_dir().join("virtual_branches.toml");
        let metadata_backup = ctx
            .project_data_dir()
            .join("virtual_branches.toml.absorb-test-backup");
        let had_metadata = metadata_path.exists();
        if had_metadata {
            std::fs::rename(&metadata_path, &metadata_backup)?;
        }
        std::fs::create_dir(&metadata_path)?;

        let result = absorb(&mut ctx, plan);

        std::fs::remove_dir(&metadata_path)?;
        if had_metadata {
            std::fs::rename(&metadata_backup, &metadata_path)?;
        }
        let error = result.expect_err("unreadable metadata must prevent checkpoint preparation");
        assert!(
            format!("{error:#}").contains("Failed to prepare absorb rollback checkpoint"),
            "checkpoint preparation identifies its operation boundary: {error:#}"
        );
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;
        assert_eq!(
            after, before,
            "failed checkpoint preparation must leave the invocation unchanged"
        );
        Ok(())
    }

    #[test]
    fn ref_lock_failure_during_publication_leaves_invocation_unchanged() -> anyhow::Result<()> {
        for record_checkpoint in [false, true] {
            let (mut ctx, tmp, plan, before) = absorb_recovery_fixture(false)?;
            let reference_path = ctx.repo.get()?.git_dir().join("refs/heads/feature");
            let external_lock = gix::lock::File::acquire_to_update_resource(
                &reference_path,
                gix::lock::acquire::Fail::Immediately,
                None,
            )?;

            let result = if record_checkpoint {
                absorb(&mut ctx, plan).map(|_| ())
            } else {
                let mut guard = ctx.exclusive_worktree_access();
                absorb_with_perm(&mut ctx, plan, guard.write_permission()).map(|_| ())
            };
            let error = result.expect_err("the external ref lock must fail publication");

            assert!(
                format!("{error:#}").contains("lock"),
                "the materialization failure identifies the target ref lock: {error:#}"
            );
            assert!(
                external_lock.lock_path().exists(),
                "absorb must not disturb the external target ref lock"
            );
            let after = absorb_invocation_state(
                &mut ctx,
                tmp.path(),
                &["shared.txt"],
                &[
                    "refs/heads/main",
                    "refs/heads/feature",
                    "refs/remotes/origin/main",
                    "refs/heads/gitbutler/workspace",
                ],
                &[("refs/heads/feature", &["shared.txt"])],
            )?;
            assert_eq!(
                after, before,
                "a returned publication failure must leave refs, index, worktree, and metadata unchanged"
            );
        }
        Ok(())
    }

    #[test]
    fn checkpoint_failure_reports_published_without_automatic_undo() -> anyhow::Result<()> {
        let (mut ctx, tmp, plan, before) = absorb_recovery_fixture(false)?;
        let mut guard = ctx.exclusive_worktree_access();
        let checkpoint = but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
            &ctx,
            SnapshotDetails::new(OperationKind::Absorb),
            guard.read_permission(),
        )?;
        let oplog_path = ctx.project_data_dir().join("operations-log.toml");
        let backup_path = ctx.project_data_dir().join("operations-log.w07-backup");
        let had_oplog = oplog_path.exists();
        if had_oplog {
            std::fs::rename(&oplog_path, &backup_path)?;
        }
        std::fs::create_dir(&oplog_path)?;

        let result = absorb_with_checkpoint_with_perm(
            &mut ctx,
            plan,
            guard.write_permission(),
            Some(checkpoint),
        );

        std::fs::remove_dir(&oplog_path)?;
        if had_oplog {
            std::fs::rename(&backup_path, &oplog_path)?;
        }
        let error = result.expect_err("the obstructed oplog path must fail checkpoint commit");
        assert!(
            error.downcast_ref::<AbsorbCheckpointError>().is_some(),
            "checkpoint failure has its post-publication classification: {error:#}"
        );
        assert!(
            error.to_string().contains("Automatic undo is unavailable"),
            "checkpoint failure gives accurate recovery guidance: {error:#}"
        );
        drop(guard);
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;
        assert_ne!(
            after.refs, before.refs,
            "graph publication precedes checkpoint commit"
        );
        assert_eq!(
            after.worktree_files, before.worktree_files,
            "published absorb preserves user-visible worktree bytes"
        );
        assert_eq!(
            after.oplog_head, before.oplog_head,
            "failed checkpoint commit does not advertise an undo snapshot"
        );
        Ok(())
    }

    #[test]
    fn finalization_failure_can_be_undone_to_pre_absorb_state() -> anyhow::Result<()> {
        let (mut ctx, tmp, plan, before) = absorb_recovery_fixture(true)?;
        let mut guard = ctx.exclusive_worktree_access();
        let checkpoint = but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
            &ctx,
            SnapshotDetails::new(OperationKind::Absorb),
            guard.read_permission(),
        )?;
        let database_path = but_db::DbHandle::db_file_path(ctx.project_data_dir());
        let fault_connection = rusqlite::Connection::open(database_path)?;
        fault_connection.execute_batch(
            "CREATE TRIGGER fail_absorb_assignment_delete
             BEFORE DELETE ON hunk_assignments
             BEGIN
                 SELECT RAISE(ABORT, 'injected assignment reconciliation failure');
             END;",
        )?;

        let error = absorb_with_checkpoint_with_perm(
            &mut ctx,
            plan,
            guard.write_permission(),
            Some(checkpoint),
        )
        .expect_err("the assignment trigger must fail finalization");

        assert!(
            error.downcast_ref::<AbsorbFinalizationError>().is_some(),
            "assignment failure has its post-checkpoint classification: {error:#}"
        );
        assert!(
            error.to_string().contains("Run `but undo` before retrying"),
            "finalization failure identifies the available recovery: {error:#}"
        );
        let checkpoint_id = ctx
            .oplog_head()?
            .expect("checkpoint commit succeeded before assignment finalization");
        assert_ne!(
            Some(checkpoint_id),
            before.oplog_head,
            "finalization failure leaves the absorb checkpoint available"
        );

        fault_connection.execute_batch("DROP TRIGGER fail_absorb_assignment_delete;")?;
        drop(fault_connection);
        crate::legacy::oplog::restore_snapshot_with_kind_with_perm(
            &mut ctx,
            crate::legacy::oplog::RestoreKind::RestoreFromSnapshotViaUndo,
            checkpoint_id,
            guard.write_permission(),
        )?;
        drop(guard);
        let reconciled = crate::diff::changes_in_worktree(&ctx, ChangesSource::Head, true)?;
        assert!(
            reconciled.assignments_error.is_none(),
            "undo assignment reconciliation must succeed"
        );
        assert_eq!(
            reconciled
                .assignments
                .iter()
                .map(|assignment| assignment.path.as_str())
                .collect::<Vec<_>>(),
            ["shared.txt", "shared.txt"],
            "reconciled assignments cover both restored worktree hunks"
        );
        let mut restored = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;
        assert_ne!(
            restored.oplog_head,
            Some(checkpoint_id),
            "undo records a new oplog entry instead of leaving the absorb checkpoint as head"
        );
        restored.oplog_head = before.oplog_head;
        for assignment in &mut restored.assignments {
            assignment.id = None;
        }
        assert_eq!(
            restored, before,
            "undo restores refs, commits, index, worktree, metadata, and reconciled assignments"
        );
        Ok(())
    }

    #[test]
    fn action_only_finalization_failure_does_not_claim_undo() -> anyhow::Result<()> {
        let (mut ctx, tmp, plan, before) = absorb_recovery_fixture(true)?;
        let mut guard = ctx.exclusive_worktree_access();
        let database_path = but_db::DbHandle::db_file_path(ctx.project_data_dir());
        let fault_connection = rusqlite::Connection::open(database_path)?;
        fault_connection.execute_batch(
            "CREATE TRIGGER fail_absorb_assignment_delete
             BEFORE DELETE ON hunk_assignments
             BEGIN
                 SELECT RAISE(ABORT, 'injected assignment reconciliation failure');
             END;",
        )?;

        let error = absorb_with_perm(&mut ctx, plan, guard.write_permission())
            .expect_err("the assignment trigger must fail action-only finalization");

        assert!(
            error.downcast_ref::<AbsorbFinalizationError>().is_some(),
            "assignment failure retains its post-publication classification: {error:#}"
        );
        assert!(
            !error.to_string().contains("Run `but undo`"),
            "action-only finalization must not claim an unpublished checkpoint: {error:#}"
        );
        assert!(
            error.to_string().contains("Automatic undo is unavailable"),
            "action-only finalization gives truthful recovery guidance: {error:#}"
        );
        fault_connection.execute_batch("DROP TRIGGER fail_absorb_assignment_delete;")?;
        drop(fault_connection);
        drop(guard);
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;
        assert_ne!(after.refs, before.refs, "action-only absorb was published");
        assert_eq!(
            after.oplog_head, before.oplog_head,
            "action-only absorb does not publish its rollback checkpoint"
        );
        assert_eq!(
            after.worktree_files, before.worktree_files,
            "published action-only absorb preserves worktree bytes"
        );
        Ok(())
    }

    #[test]
    fn source_change_after_planning_fails_without_publishing() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-rejected-hunks");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let plan = absorption_plan(
            &mut ctx,
            AbsorptionTarget::Hunks {
                hunks: vec![but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 10,
                        old_lines: 1,
                        new_start: 10,
                        new_lines: 1,
                    }),
                    path: "shared.txt".into(),
                    diff: None,
                }],
            },
        )?;
        let path = tmp.path().join("shared.txt");
        let changed_source = std::fs::read_to_string(&path)?
            .replace("selected change\n", "changed after planning\n");
        std::fs::write(&path, changed_source)?;
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &["refs/heads/feature", "refs/remotes/origin/main"],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;

        let err = absorb(&mut ctx, plan).expect_err("a stale source plan must fail");
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &["refs/heads/feature", "refs/remotes/origin/main"],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;

        assert!(
            err.to_string().contains("stale"),
            "the caller receives an actionable stale-plan error: {err:#}"
        );
        assert_eq!(
            after, before,
            "a stale plan leaves invocation state unchanged"
        );
        Ok(())
    }

    #[test]
    fn assignment_change_after_planning_fails_without_publishing() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-rejected-hunks");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;

        {
            let guard = ctx.exclusive_worktree_access();
            crate::diff::changes_in_worktree_with_perm(
                &ctx,
                ChangesSource::Head,
                true,
                guard.read_permission(),
            )?;
        }
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &["refs/heads/feature", "refs/remotes/origin/main"],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;

        let err = absorb(&mut ctx, plan).expect_err("changed routing state must stale the plan");
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &["refs/heads/feature", "refs/remotes/origin/main"],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;

        assert!(
            err.to_string().contains("stale"),
            "the caller receives an actionable stale-plan error: {err:#}"
        );
        assert_eq!(
            after, before,
            "changed assignment state leaves invocation state unchanged"
        );
        Ok(())
    }

    #[test]
    fn changed_or_unstamped_plan_preconditions_fail_without_publishing() -> anyhow::Result<()> {
        for mutation in ["route", "context", "unstamped"] {
            let (repo, tmp) = but_testsupport::writable_scenario("absorb-rejected-hunks");
            but_core::ref_metadata::ProjectMeta {
                target_ref: Some("refs/remotes/origin/main".try_into()?),
                target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
                push_remote: None,
            }
            .persist(&repo)?;
            let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
            ctx.settings.context_lines = 0;
            let mut plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;

            match mutation {
                "route" => {
                    plan[0].commit_id = ctx
                        .repo
                        .get()?
                        .rev_parse_single("refs/remotes/origin/main")?
                        .detach();
                }
                "context" => ctx.settings.context_lines = 3,
                "unstamped" => plan[0].source_snapshot_tree = None,
                _ => unreachable!("all mutation cases are listed above"),
            }

            let before = absorb_invocation_state(
                &mut ctx,
                tmp.path(),
                &["shared.txt"],
                &["refs/heads/feature", "refs/remotes/origin/main"],
                &[("refs/heads/feature", &["shared.txt"])],
            )?;
            let err = absorb(&mut ctx, plan)
                .expect_err("a changed or unstamped plan precondition must fail");
            let after = absorb_invocation_state(
                &mut ctx,
                tmp.path(),
                &["shared.txt"],
                &["refs/heads/feature", "refs/remotes/origin/main"],
                &[("refs/heads/feature", &["shared.txt"])],
            )?;

            assert!(
                err.to_string().contains("stale"),
                "{mutation} returns an actionable stale-plan error: {err:#}"
            );
            assert_eq!(
                after, before,
                "{mutation} leaves invocation state unchanged"
            );
        }
        Ok(())
    }

    #[test]
    fn mixed_valid_and_unrecognized_selectors_leave_public_absorb_unchanged() -> anyhow::Result<()>
    {
        for (include_valid_selection, delete_source) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let (repo, tmp) = but_testsupport::writable_scenario("absorb-paired-selection");
            if delete_source {
                std::fs::remove_file(tmp.path().join("selected.txt"))?;
            }
            let commit_id = repo.head_id()?.detach();
            but_core::ref_metadata::ProjectMeta {
                target_ref: Some("refs/remotes/origin/main".try_into()?),
                target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
                push_remote: None,
            }
            .persist(&repo)?;
            let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
            ctx.settings.context_lines = 0;
            let mut headers = Vec::new();
            if include_valid_selection {
                headers.push(but_core::HunkHeader {
                    old_start: 2,
                    old_lines: 1,
                    new_start: 0,
                    new_lines: 0,
                });
                if !delete_source {
                    headers.push(but_core::HunkHeader {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 2,
                        new_lines: 1,
                    });
                }
            }
            headers.push(but_core::HunkHeader {
                old_start: 4,
                old_lines: 1,
                new_start: 4,
                new_lines: 1,
            });
            let mut plan = vec![CommitAbsorption {
                stack_id: StackId::generate(),
                commit_id,
                blank_commit_ref: None,
                source_snapshot_tree: None,
                commit_summary: "add selected lines".into(),
                hunks: headers
                    .into_iter()
                    .map(|header| but_core::SingleHunk {
                        hunk_header: Some(header),
                        path: "selected.txt".into(),
                        diff: None,
                    })
                    .collect(),
                reason: AbsorptionReason::HunkDependency,
            }];
            stamp_plan(&mut ctx, &mut plan)?;
            let refs = [
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
            ];
            let before = absorb_invocation_state(
                &mut ctx,
                tmp.path(),
                &["selected.txt"],
                &refs,
                &[("refs/heads/feature", &["selected.txt"])],
            )?;

            let rejected = absorb(&mut ctx, plan)
                .expect_err("an unrecognized selector must reject the entire invocation");

            let after = absorb_invocation_state(
                &mut ctx,
                tmp.path(),
                &["selected.txt"],
                &refs,
                &[("refs/heads/feature", &["selected.txt"])],
            )?;
            assert!(
                rejected.to_string().contains("selected.txt")
                    && rejected.to_string().contains("-4,1 +4,1")
                    && rejected.to_string().contains("no changes were published"),
                "the invalid header must be reported even with a valid selection: {rejected}"
            );
            assert_eq!(
                after, before,
                "rejection preserves invocation state with include_valid_selection={include_valid_selection}, delete_source={delete_source}"
            );
        }
        Ok(())
    }

    #[test]
    fn rejected_hunk_after_applicable_hunk_leaves_public_absorb_invocation_unchanged()
    -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-rejected-hunks");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;

        let plan = absorption_plan(
            &mut ctx,
            AbsorptionTarget::Hunks {
                hunks: [10, 5]
                    .into_iter()
                    .map(|line| but_core::SingleHunk {
                        hunk_header: Some(but_core::HunkHeader {
                            old_start: line,
                            old_lines: 1,
                            new_start: line,
                            new_lines: 1,
                        }),
                        path: "shared.txt".into(),
                        diff: None,
                    })
                    .collect(),
            },
        )?;
        assert_eq!(
            plan.iter()
                .map(|absorption| absorption.hunks.len())
                .sum::<usize>(),
            2,
            "planning retains both selected hunk candidates for execution"
        );

        let rejected = absorb(&mut ctx, plan).expect_err("atomic rejection must be an error");
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["shared.txt"],
            &[
                "refs/heads/main",
                "refs/heads/feature",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[("refs/heads/feature", &["shared.txt"])],
        )?;

        assert!(
            rejected.to_string().contains("shared.txt")
                && rejected.to_string().contains("no changes were published")
                && after == before,
            "a rejected invocation must not publish an earlier applicable amendment; \
             rejected groups: {rejected};\nstate before:\n{}\nstate after:\n{}",
            state_summary(&before),
            state_summary(&after),
        );
        Ok(())
    }

    #[test]
    fn rejection_on_one_independent_branch_leaves_all_planned_branches_unchanged()
    -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-independent-branches");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let a_commit = repo.rev_parse_single("refs/heads/A")?.detach();
        let b_commit = repo.rev_parse_single("refs/heads/B")?.detach();
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        {
            let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
            let mut metadata = ctx.meta()?;
            let mut workspace = metadata.workspace(workspace_ref.as_ref())?;
            for branch in ["A", "B"] {
                let branch_ref: gix::refs::FullName = format!("refs/heads/{branch}").try_into()?;
                workspace.add_or_insert_new_stack_if_not_present(
                    branch_ref.as_ref(),
                    None,
                    but_core::ref_metadata::WorkspaceCommitRelation::Merged,
                    |_| StackId::generate(),
                );
            }
            metadata.set_workspace(&workspace)?;
        }
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["a.txt", "b.txt"],
            &[
                "refs/heads/main",
                "refs/heads/A",
                "refs/heads/B",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[
                ("refs/heads/A", &["a.txt"]),
                ("refs/heads/B", &["b.txt"]),
                ("refs/heads/gitbutler/workspace", &["a.txt", "b.txt"]),
            ],
        )?;

        let mut plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        assert_eq!(
            plan.len(),
            2,
            "the independent fixture produces one planned target on each branch"
        );
        let planned_targets = plan
            .iter()
            .map(|absorption| {
                (
                    absorption.commit_id,
                    absorption
                        .hunks
                        .iter()
                        .map(|hunk| hunk.path.to_string())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            planned_targets.contains(&(a_commit, vec!["a.txt".to_owned()])),
            "a.txt is locked to A rather than routed by workspace order: {planned_targets:#?}"
        );
        assert!(
            planned_targets.contains(&(b_commit, vec!["b.txt".to_owned()])),
            "b.txt is locked to B rather than routed by workspace order: {planned_targets:#?}"
        );

        let b_absorption = plan
            .iter_mut()
            .find(|absorption| absorption.commit_id == b_commit)
            .expect("planning includes B's selected hunk");
        assert_eq!(
            b_absorption.hunks.len(),
            1,
            "the fixture has one selected hunk on B"
        );
        b_absorption.hunks[0].hunk_header = Some(but_core::HunkHeader {
            old_start: 5,
            old_lines: 1,
            new_start: 5,
            new_lines: 1,
        });

        stamp_plan(&mut ctx, &mut plan)?;
        let rejected = absorb(&mut ctx, plan).expect_err("atomic rejection must be an error");
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["a.txt", "b.txt"],
            &[
                "refs/heads/main",
                "refs/heads/A",
                "refs/heads/B",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[
                ("refs/heads/A", &["a.txt"]),
                ("refs/heads/B", &["b.txt"]),
                ("refs/heads/gitbutler/workspace", &["a.txt", "b.txt"]),
            ],
        )?;

        assert!(
            rejected.to_string().contains("b.txt") && after == before,
            "a rejection on B must not publish the independently planned A amendment; \
             rejected groups: {rejected};\nstate before:\n{}\nstate after:\n{}",
            state_summary(&before),
            state_summary(&after),
        );
        Ok(())
    }

    #[test]
    fn planning_blank_commit_before_rejection_leaves_invocation_unchanged() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-independent-branches");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        {
            let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
            let mut metadata = ctx.meta()?;
            let mut workspace = metadata.workspace(workspace_ref.as_ref())?;
            for branch in ["A", "B"] {
                let branch_ref: gix::refs::FullName = format!("refs/heads/{branch}").try_into()?;
                workspace.add_or_insert_new_stack_if_not_present(
                    branch_ref.as_ref(),
                    None,
                    but_core::ref_metadata::WorkspaceCommitRelation::Merged,
                    |_| StackId::generate(),
                );
            }
            metadata.set_workspace(&workspace)?;
        }
        let empty_branch: gix::refs::FullName = "refs/heads/empty".try_into()?;
        let a_branch: gix::refs::FullName = "refs/heads/A".try_into()?;
        crate::branch::branch_create(
            &mut ctx,
            Some(empty_branch.clone()),
            crate::branch::json::BranchCreatePlacement::Dependent {
                relative_to: crate::commit::json::RelativeTo::Reference(a_branch.clone()),
                side: InsertSide::Above,
            },
        )?;
        std::fs::write(tmp.path().join("empty.txt"), "new empty-branch content\n")?;

        crate::diff::assign_hunk_only(
            &ctx,
            vec![but_hunk_assignment::HunkAssignmentRequest {
                hunk_header: Some(but_core::HunkHeader {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 1,
                }),
                path_bytes: bstr::BString::from("empty.txt"),
                target: Some(but_hunk_assignment::HunkAssignmentTarget::Branch {
                    branch_ref_bytes: bstr::BString::from(empty_branch.to_string().as_bytes()),
                }),
            }],
        )?;
        let before = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["a.txt", "b.txt", "empty.txt"],
            &[
                "refs/heads/main",
                "refs/heads/A",
                "refs/heads/B",
                "refs/heads/empty",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[
                ("refs/heads/A", &["a.txt"]),
                ("refs/heads/B", &["b.txt"]),
                ("refs/heads/empty", &["empty.txt"]),
                ("refs/heads/gitbutler/workspace", &["a.txt", "b.txt"]),
            ],
        )?;

        let mut plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        let b_commit = ctx.repo.get()?.rev_parse_single("refs/heads/B")?.detach();
        let b_absorption = plan
            .iter_mut()
            .find(|absorption| absorption.commit_id == b_commit)
            .expect("planning includes B's selected hunk");
        b_absorption.hunks[0].hunk_header = Some(but_core::HunkHeader {
            old_start: 5,
            old_lines: 1,
            new_start: 5,
            new_lines: 1,
        });

        stamp_plan(&mut ctx, &mut plan)?;
        let empty_absorption = plan
            .iter()
            .find(|absorption| absorption.blank_commit_ref.as_ref() == Some(&empty_branch))
            .expect("planning defers a target for the empty branch");
        assert_eq!(
            empty_absorption.commit_id,
            ctx.repo
                .get()?
                .rev_parse_single("refs/heads/empty")?
                .detach(),
            "the deferred plan anchors the empty branch's current target"
        );

        let rejected = absorb(&mut ctx, plan).expect_err("atomic rejection must be an error");
        let after = absorb_invocation_state(
            &mut ctx,
            tmp.path(),
            &["a.txt", "b.txt", "empty.txt"],
            &[
                "refs/heads/main",
                "refs/heads/A",
                "refs/heads/B",
                "refs/heads/empty",
                "refs/remotes/origin/main",
                "refs/heads/gitbutler/workspace",
            ],
            &[
                ("refs/heads/A", &["a.txt"]),
                ("refs/heads/B", &["b.txt"]),
                ("refs/heads/empty", &["empty.txt"]),
                ("refs/heads/gitbutler/workspace", &["a.txt", "b.txt"]),
            ],
        )?;
        assert!(
            rejected.to_string().contains("b.txt") && after == before,
            "a later rejection must roll back planner-created commits and assignments; \
             rejected groups: {rejected};\nstate before:\n{}\nstate after:\n{}",
            state_summary(&before),
            state_summary(&after),
        );

        let empty_before = ctx
            .repo
            .get()?
            .rev_parse_single("refs/heads/empty")?
            .detach();
        let empty_plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?
            .into_iter()
            .filter(|absorption| absorption.blank_commit_ref.as_ref() == Some(&empty_branch))
            .collect::<Vec<_>>();
        assert_eq!(
            empty_plan.len(),
            1,
            "the successful control has one deferred empty-branch target"
        );
        assert_eq!(
            absorb(&mut ctx, empty_plan)?,
            0,
            "the deferred empty-branch absorption succeeds"
        );
        let repo = ctx.repo.get()?;
        let empty_after = repo.rev_parse_single("refs/heads/empty")?.detach();
        assert_ne!(
            empty_after, empty_before,
            "successful execution publishes the staged blank commit"
        );
        let blob = repo
            .find_commit(empty_after)?
            .tree()?
            .lookup_entry_by_path("empty.txt")?
            .expect("empty branch contains the absorbed file")
            .object()?
            .into_blob();
        assert_eq!(
            blob.data, b"new empty-branch content\n",
            "the staged blank commit receives the selected content"
        );
        Ok(())
    }

    #[test]
    fn independent_branch_absorption_succeeds_for_all_planned_targets() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-independent-branches");
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        {
            let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
            let mut metadata = ctx.meta()?;
            let mut workspace = metadata.workspace(workspace_ref.as_ref())?;
            for branch in ["A", "B"] {
                let branch_ref: gix::refs::FullName = format!("refs/heads/{branch}").try_into()?;
                workspace.add_or_insert_new_stack_if_not_present(
                    branch_ref.as_ref(),
                    None,
                    but_core::ref_metadata::WorkspaceCommitRelation::Merged,
                    |_| StackId::generate(),
                );
            }
            metadata.set_workspace(&workspace)?;
        }
        let a_before = ctx.repo.get()?.rev_parse_single("refs/heads/A")?.detach();
        let b_before = ctx.repo.get()?.rev_parse_single("refs/heads/B")?.detach();
        let before_files = ["a.txt", "b.txt"]
            .map(|path| std::fs::read_to_string(tmp.path().join(path)))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        {
            let guard = ctx.exclusive_worktree_access();
            crate::diff::changes_in_worktree_with_perm(
                &ctx,
                ChangesSource::Head,
                true,
                guard.read_permission(),
            )?;
        }
        assert!(
            !ctx.db
                .get_cache()?
                .hunk_assignments()
                .list_all()?
                .is_empty(),
            "the control starts with persisted worktree assignments"
        );

        let plan = absorption_plan(&mut ctx, AbsorptionTarget::All)?;
        assert_eq!(plan.len(), 2, "both independent changes are planned");
        let rejected = absorb(&mut ctx, plan)?;
        assert_eq!(rejected, 0, "both eligible independent targets succeed");
        assert_ne!(
            ctx.repo.get()?.rev_parse_single("refs/heads/A")?.detach(),
            a_before,
            "A is amended exactly once"
        );
        assert_ne!(
            ctx.repo.get()?.rev_parse_single("refs/heads/B")?.detach(),
            b_before,
            "B is amended exactly once"
        );
        let after_files = ["a.txt", "b.txt"]
            .map(|path| std::fs::read_to_string(tmp.path().join(path)))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            after_files, before_files,
            "successful absorb preserves worktree bytes"
        );
        assert!(
            but_core::worktree_hunks(&*ctx.repo.get()?, 0)?.is_empty(),
            "all accepted groups are absent from the residual worktree diff"
        );
        assert!(
            ctx.db
                .get_cache()?
                .hunk_assignments()
                .list_all()?
                .is_empty(),
            "consumed worktree hunks leave no stale assignment rows"
        );
        Ok(())
    }

    #[test]
    fn rejected_hunks_are_counted_once_per_original_commit_and_path() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-rejected-hunks");
        let content = (1..=20)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        let worktree_content = content
            .replace("line 1\n", "unselected change\n")
            .replace("line 10\n", "selected change\n");

        let commit_id = repo.head_id()?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let mut plan = vec![CommitAbsorption {
            stack_id: StackId::generate(),
            commit_id,
            blank_commit_ref: None,
            source_snapshot_tree: None,
            commit_summary: "add shared file".into(),
            hunks: [5, 10, 18]
                .into_iter()
                .map(|line| but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: line,
                        old_lines: 1,
                        new_start: line,
                        new_lines: 1,
                    }),
                    path: "shared.txt".into(),
                    diff: None,
                })
                .collect(),
            reason: AbsorptionReason::HunkDependency,
        }];
        stamp_plan(&mut ctx, &mut plan)?;
        let mut guard = ctx.exclusive_worktree_access();
        let rejected = absorb_with_perm(&mut ctx, plan, guard.write_permission())?;

        assert_eq!(
            rejected.rejected_count(),
            1,
            "two stale hunks from one file count as one rejection"
        );
        let diagnostic = rejected.to_string();
        assert!(
            diagnostic.contains("shared.txt -5,1 +5,1")
                && diagnostic.contains("add shared file")
                && diagnostic.contains("files locked to commit")
                && diagnostic.contains("no changes were published"),
            "rejection diagnostics identify the selection, target, reason, and atomic outcome: \
             {diagnostic}"
        );
        let repo = ctx.repo.get()?;
        let tree = repo.head_commit()?.tree()?;
        let blob = tree
            .lookup_entry_by_path("shared.txt")?
            .expect("committed file")
            .object()?
            .into_blob();
        assert_eq!(
            blob.data,
            content.as_bytes(),
            "a rejection must leave the target commit unchanged"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("shared.txt"))?,
            worktree_content,
            "partial absorption must preserve the worktree"
        );
        Ok(())
    }

    #[test]
    fn paired_old_and_new_hunk_selections_preserve_their_shared_replacement() -> anyhow::Result<()>
    {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-paired-selection");
        let worktree_content = (1..=10)
            .map(|line| format!("new-{line:02}\n"))
            .collect::<String>();
        let expected_target_content = (1..=10)
            .map(|line| {
                if line == 5 {
                    "new-05\n".to_owned()
                } else {
                    format!("old-{line:02}\n")
                }
            })
            .collect::<String>();
        let initial_hunks = but_core::worktree_hunks(&repo, 0)?;
        assert_eq!(
            initial_hunks.len(),
            1,
            "the fixture begins as one zero-context replacement hunk"
        );
        assert_eq!(
            initial_hunks[0].hunk_header,
            Some(but_core::HunkHeader {
                old_start: 1,
                old_lines: 10,
                new_start: 1,
                new_lines: 10,
            }),
            "the fixture's only hunk spans both ten-line images"
        );

        let commit_id = repo.head_id()?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let mut plan = vec![CommitAbsorption {
            stack_id: StackId::generate(),
            commit_id,
            blank_commit_ref: None,
            source_snapshot_tree: None,
            commit_summary: "add selected lines".into(),
            hunks: vec![
                but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 5,
                        old_lines: 1,
                        new_start: 0,
                        new_lines: 0,
                    }),
                    path: "selected.txt".into(),
                    diff: None,
                },
                but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 5,
                        new_lines: 1,
                    }),
                    path: "selected.txt".into(),
                    diff: None,
                },
            ],
            reason: AbsorptionReason::HunkDependency,
        }];
        stamp_plan(&mut ctx, &mut plan)?;

        let mut guard = ctx.exclusive_worktree_access();
        let rejected = absorb_with_perm(&mut ctx, plan, guard.write_permission())?;

        assert_eq!(
            rejected.rejected_count(),
            0,
            "paired selections must both be accepted"
        );
        let repo = ctx.repo.get()?;
        let tree = repo.head_commit()?.tree()?;
        let blob = tree
            .lookup_entry_by_path("selected.txt")?
            .expect("committed file")
            .object()?
            .into_blob();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("selected.txt"))?,
            worktree_content,
            "a successful absorb preserves the worktree bytes"
        );
        assert_eq!(
            but_core::worktree_hunks(&repo, 0)?
                .into_iter()
                .map(|hunk| hunk.hunk_header.expect("text hunk"))
                .collect::<Vec<_>>(),
            [
                but_core::HunkHeader {
                    old_start: 1,
                    old_lines: 4,
                    new_start: 1,
                    new_lines: 4,
                },
                but_core::HunkHeader {
                    old_start: 6,
                    old_lines: 5,
                    new_start: 6,
                    new_lines: 5,
                },
            ],
            "the residual diff excludes only the paired replacement"
        );
        assert_eq!(
            blob.data,
            expected_target_content.as_bytes(),
            "only the selected replacement line belongs in the target commit"
        );
        Ok(())
    }

    #[test]
    fn multiple_old_side_hunk_selections_do_not_use_shifted_coordinates() -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-paired-selection");
        let worktree_content = (1..=10)
            .map(|line| format!("new-{line:02}\n"))
            .collect::<String>();
        let expected_target_content = [1, 3, 4, 5, 6, 8, 9, 10]
            .into_iter()
            .map(|line| format!("old-{line:02}\n"))
            .collect::<String>();
        assert_eq!(
            but_core::worktree_hunks(&repo, 0)?.len(),
            1,
            "the fixture begins as one zero-context replacement hunk"
        );

        let commit_id = repo.head_id()?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let mut plan = vec![CommitAbsorption {
            stack_id: StackId::generate(),
            commit_id,
            blank_commit_ref: None,
            source_snapshot_tree: None,
            commit_summary: "add selected lines".into(),
            hunks: [2, 7]
                .into_iter()
                .map(|old_start| but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start,
                        old_lines: 1,
                        new_start: 0,
                        new_lines: 0,
                    }),
                    path: "selected.txt".into(),
                    diff: None,
                })
                .collect(),
            reason: AbsorptionReason::HunkDependency,
        }];
        stamp_plan(&mut ctx, &mut plan)?;

        let mut guard = ctx.exclusive_worktree_access();
        let rejected = absorb_with_perm(&mut ctx, plan, guard.write_permission())?;

        assert_eq!(
            rejected.rejected_count(),
            0,
            "both old-side selections must be accepted"
        );
        let repo = ctx.repo.get()?;
        let tree = repo.head_commit()?.tree()?;
        let blob = tree
            .lookup_entry_by_path("selected.txt")?
            .expect("committed file")
            .object()?
            .into_blob();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("selected.txt"))?,
            worktree_content,
            "a successful absorb preserves the worktree bytes"
        );
        assert_eq!(
            but_core::worktree_hunks(&repo, 0)?
                .into_iter()
                .map(|hunk| hunk.hunk_header.expect("text hunk"))
                .collect::<Vec<_>>(),
            [but_core::HunkHeader {
                old_start: 1,
                old_lines: 8,
                new_start: 1,
                new_lines: 10,
            }],
            "the residual diff contains every new line against the reduced target"
        );
        assert_eq!(
            blob.data,
            expected_target_content.as_bytes(),
            "only the selected original lines are removed from the target"
        );
        Ok(())
    }

    #[test]
    fn mixed_full_and_paired_hunk_selections_preserve_only_selected_content() -> anyhow::Result<()>
    {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-mixed-selection");
        let worktree_content = (1..=22)
            .map(|line| match line {
                1..=4 | 13..=22 => format!("new-{line:02}\n"),
                5..=12 => format!("stable-{line:02}\n"),
                _ => unreachable!("fixture line is within its documented range"),
            })
            .collect::<String>();
        let expected_target_content = (1..=22)
            .map(|line| match line {
                1..=4 | 15 => format!("new-{line:02}\n"),
                5..=12 => format!("stable-{line:02}\n"),
                13..=14 | 16..=22 => format!("old-{line:02}\n"),
                _ => unreachable!("fixture line is within its documented range"),
            })
            .collect::<String>();
        assert_eq!(
            but_core::worktree_hunks(&repo, 0)?
                .into_iter()
                .map(|hunk| hunk.hunk_header.expect("text hunk"))
                .collect::<Vec<_>>(),
            [
                but_core::HunkHeader {
                    old_start: 1,
                    old_lines: 4,
                    new_start: 1,
                    new_lines: 4,
                },
                but_core::HunkHeader {
                    old_start: 13,
                    old_lines: 10,
                    new_start: 13,
                    new_lines: 10,
                },
            ],
            "the fixture begins with two independent zero-context replacement hunks"
        );

        let commit_id = repo.head_id()?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let mut plan = vec![CommitAbsorption {
            stack_id: StackId::generate(),
            commit_id,
            blank_commit_ref: None,
            source_snapshot_tree: None,
            commit_summary: "add selected regions".into(),
            hunks: vec![
                but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 1,
                        old_lines: 4,
                        new_start: 1,
                        new_lines: 4,
                    }),
                    path: "selected.txt".into(),
                    diff: None,
                },
                but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 15,
                        old_lines: 1,
                        new_start: 0,
                        new_lines: 0,
                    }),
                    path: "selected.txt".into(),
                    diff: None,
                },
                but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 15,
                        new_lines: 1,
                    }),
                    path: "selected.txt".into(),
                    diff: None,
                },
            ],
            reason: AbsorptionReason::HunkDependency,
        }];
        stamp_plan(&mut ctx, &mut plan)?;

        let mut guard = ctx.exclusive_worktree_access();
        let rejected = absorb_with_perm(&mut ctx, plan, guard.write_permission())?;

        assert_eq!(
            rejected.rejected_count(),
            0,
            "all mixed selections must be accepted"
        );
        let repo = ctx.repo.get()?;
        let tree = repo.head_commit()?.tree()?;
        let blob = tree
            .lookup_entry_by_path("selected.txt")?
            .expect("committed file")
            .object()?
            .into_blob();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("selected.txt"))?,
            worktree_content,
            "a successful absorb preserves the worktree bytes"
        );
        assert_eq!(
            but_core::worktree_hunks(&repo, 0)?
                .into_iter()
                .map(|hunk| hunk.hunk_header.expect("text hunk"))
                .collect::<Vec<_>>(),
            [
                but_core::HunkHeader {
                    old_start: 13,
                    old_lines: 2,
                    new_start: 13,
                    new_lines: 2,
                },
                but_core::HunkHeader {
                    old_start: 16,
                    old_lines: 7,
                    new_start: 16,
                    new_lines: 7,
                },
            ],
            "the residual diff excludes the full hunk and the selected paired line"
        );
        assert_eq!(
            blob.data,
            expected_target_content.as_bytes(),
            "the target contains the full first hunk and only the paired second-region line"
        );
        Ok(())
    }

    #[test]
    fn partial_selections_in_two_blocks_preserve_worktree() -> anyhow::Result<()> {
        partial_first_selection_preserves_worktree(false)
    }

    #[test]
    fn partial_first_full_last_selection_preserves_worktree() -> anyhow::Result<()> {
        partial_first_selection_preserves_worktree(true)
    }

    fn partial_first_selection_preserves_worktree(
        absorb_last_block_in_full: bool,
    ) -> anyhow::Result<()> {
        let (repo, tmp) = but_testsupport::writable_scenario("absorb-mixed-selection");
        let worktree_content = std::fs::read(tmp.path().join("selected.txt"))?;
        let commit_id = repo.head_id()?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let mut headers = vec![
            but_core::HunkHeader {
                old_start: 2,
                old_lines: 1,
                new_start: 0,
                new_lines: 0,
            },
            but_core::HunkHeader {
                old_start: 0,
                old_lines: 0,
                new_start: 2,
                new_lines: 1,
            },
        ];
        if absorb_last_block_in_full {
            headers.push(but_core::HunkHeader {
                old_start: 13,
                old_lines: 10,
                new_start: 13,
                new_lines: 10,
            });
        } else {
            headers.extend([
                but_core::HunkHeader {
                    old_start: 15,
                    old_lines: 1,
                    new_start: 0,
                    new_lines: 0,
                },
                but_core::HunkHeader {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 15,
                    new_lines: 1,
                },
            ]);
            headers.rotate_left(2);
        }
        let mut plan = vec![CommitAbsorption {
            stack_id: StackId::generate(),
            commit_id,
            blank_commit_ref: None,
            source_snapshot_tree: None,
            commit_summary: "add selected regions".into(),
            hunks: headers
                .into_iter()
                .map(|header| but_core::SingleHunk {
                    hunk_header: Some(header),
                    path: "selected.txt".into(),
                    diff: None,
                })
                .collect(),
            reason: AbsorptionReason::HunkDependency,
        }];
        stamp_plan(&mut ctx, &mut plan)?;
        let mut guard = ctx.exclusive_worktree_access();
        let outcome = absorb_with_perm(&mut ctx, plan, guard.write_permission())?;
        assert_eq!(
            outcome.rejected_count(),
            0,
            "independent source-hunk groups must all be accepted"
        );
        assert_eq!(
            std::fs::read(tmp.path().join("selected.txt"))?,
            worktree_content,
            "checkout cancellation must preserve unselected worktree content without conflict markers"
        );
        let expected_target_content = (1..=22)
            .map(|line| {
                if (5..=12).contains(&line) {
                    format!("stable-{line:02}\n")
                } else if line == 2
                    || (absorb_last_block_in_full && line >= 13)
                    || (!absorb_last_block_in_full && line == 15)
                {
                    format!("new-{line:02}\n")
                } else {
                    format!("old-{line:02}\n")
                }
            })
            .collect::<String>();
        let repo = ctx.repo.get()?;
        let blob = repo
            .head_commit()?
            .tree()?
            .lookup_entry_by_path("selected.txt")?
            .expect("committed file")
            .object()?
            .into_blob();
        assert_eq!(
            blob.data,
            expected_target_content.as_bytes(),
            "all and only selected replacements must appear in the amended target"
        );
        Ok(())
    }

    #[test]
    fn planner_and_executor_preserve_paired_hunk_selection_content() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("absorb-paired-selection");
        let expected_target_content = (1..=10)
            .map(|line| {
                if line == 5 {
                    "new-05\n".to_owned()
                } else {
                    format!("old-{line:02}\n")
                }
            })
            .collect::<String>();
        let commit_id = repo.head_id()?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let selected_hunks = vec![
            but_core::SingleHunk {
                hunk_header: Some(but_core::HunkHeader {
                    old_start: 5,
                    old_lines: 1,
                    new_start: 0,
                    new_lines: 0,
                }),
                path: "selected.txt".into(),
                diff: None,
            },
            but_core::SingleHunk {
                hunk_header: Some(but_core::HunkHeader {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 5,
                    new_lines: 1,
                }),
                path: "selected.txt".into(),
                diff: None,
            },
        ];

        let mut guard = ctx.exclusive_worktree_access();
        let plan = absorption_plan_with_perm(
            &mut ctx,
            AbsorptionTarget::Hunks {
                hunks: selected_hunks,
            },
            guard.write_permission(),
        )?;

        assert_eq!(
            plan.len(),
            1,
            "the single-target fixture produces one absorption destination"
        );
        assert_eq!(
            plan[0].commit_id, commit_id,
            "both selections are routed to the only mutable target commit"
        );
        assert_eq!(
            plan[0]
                .hunks
                .iter()
                .map(|hunk| hunk.hunk_header.expect("text hunk"))
                .collect::<Vec<_>>(),
            [
                but_core::HunkHeader {
                    old_start: 5,
                    old_lines: 1,
                    new_start: 0,
                    new_lines: 0,
                },
                but_core::HunkHeader {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 5,
                    new_lines: 1,
                },
            ],
            "planning retains the caller's paired-selector order"
        );

        let rejected = absorb_with_perm(&mut ctx, plan, guard.write_permission())?;

        assert_eq!(
            rejected.rejected_count(),
            0,
            "the routed paired selections must be accepted"
        );
        let repo = ctx.repo.get()?;
        let tree = repo.head_commit()?.tree()?;
        let blob = tree
            .lookup_entry_by_path("selected.txt")?
            .expect("committed file")
            .object()?
            .into_blob();
        assert_eq!(
            blob.data,
            expected_target_content.as_bytes(),
            "routing must not lose the pair's selection meaning before execution"
        );
        Ok(())
    }

    #[test]
    fn generated_paired_replacements_match_independent_line_oracle() -> anyhow::Result<()> {
        const SEEDS: [u16; 6] = [
            0b0000000001,
            0b1000000000,
            0b0000000011,
            0b1100000000,
            0b0101010101,
            0b1000001100,
        ];

        for seed in SEEDS {
            let (repo, tmp) = but_testsupport::writable_scenario("absorb-paired-selection");
            but_core::ref_metadata::ProjectMeta {
                target_ref: Some("refs/remotes/origin/main".try_into()?),
                target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
                push_remote: None,
            }
            .persist(&repo)?;
            let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
            ctx.settings.context_lines = 0;
            let selected_hunks = (1..=10)
                .filter(|line| seed & (1 << (line - 1)) != 0)
                .flat_map(|line| {
                    [
                        but_core::SingleHunk {
                            hunk_header: Some(but_core::HunkHeader {
                                old_start: line,
                                old_lines: 1,
                                new_start: 0,
                                new_lines: 0,
                            }),
                            path: "selected.txt".into(),
                            diff: None,
                        },
                        but_core::SingleHunk {
                            hunk_header: Some(but_core::HunkHeader {
                                old_start: 0,
                                old_lines: 0,
                                new_start: line,
                                new_lines: 1,
                            }),
                            path: "selected.txt".into(),
                            diff: None,
                        },
                    ]
                })
                .collect();

            let plan = absorption_plan(
                &mut ctx,
                AbsorptionTarget::Hunks {
                    hunks: selected_hunks,
                },
            )?;
            let rejected = absorb(&mut ctx, plan)?;

            assert_eq!(rejected, 0, "seed={seed:#05x} must be fully accepted");
            let expected_target_content = (1..=10)
                .map(|line| {
                    if seed & (1 << (line - 1)) != 0 {
                        format!("new-{line:02}\n")
                    } else {
                        format!("old-{line:02}\n")
                    }
                })
                .collect::<String>();
            let expected_worktree_content = (1..=10)
                .map(|line| format!("new-{line:02}\n"))
                .collect::<String>();
            let mut expected_residual_hunks = Vec::new();
            let mut line = 1;
            while line <= 10 {
                if seed & (1 << (line - 1)) != 0 {
                    line += 1;
                    continue;
                }
                let start = line;
                while line <= 10 && seed & (1 << (line - 1)) == 0 {
                    line += 1;
                }
                expected_residual_hunks.push(but_core::HunkHeader {
                    old_start: start,
                    old_lines: line - start,
                    new_start: start,
                    new_lines: line - start,
                });
            }

            let repo = ctx.repo.get()?;
            let blob = repo
                .head_commit()?
                .tree()?
                .lookup_entry_by_path("selected.txt")?
                .expect("committed file")
                .object()?
                .into_blob();
            assert_eq!(
                blob.data,
                expected_target_content.as_bytes(),
                "seed={seed:#05x} target content must match the line oracle"
            );
            assert_eq!(
                std::fs::read_to_string(tmp.path().join("selected.txt"))?,
                expected_worktree_content,
                "seed={seed:#05x} must preserve worktree bytes"
            );
            assert_eq!(
                but_core::worktree_hunks(&repo, 0)?
                    .into_iter()
                    .map(|hunk| hunk.hunk_header.expect("text hunk"))
                    .collect::<Vec<_>>(),
                expected_residual_hunks,
                "seed={seed:#05x} residual hunks must cover exactly unselected runs"
            );
        }
        Ok(())
    }

    #[test]
    fn planner_does_not_split_a_paired_selection_across_dependency_targets() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("absorb-paired-routing");
        let first_region_commit = repo.rev_parse_single("HEAD~1")?.detach();
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.context_lines = 0;
        let paired_hunks = vec![
            but_core::SingleHunk {
                hunk_header: Some(but_core::HunkHeader {
                    old_start: 5,
                    old_lines: 1,
                    new_start: 0,
                    new_lines: 0,
                }),
                path: "routed.txt".into(),
                diff: None,
            },
            but_core::SingleHunk {
                hunk_header: Some(but_core::HunkHeader {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 5,
                    new_lines: 1,
                }),
                path: "routed.txt".into(),
                diff: None,
            },
        ];

        let mut guard = ctx.exclusive_worktree_access();
        let plan = absorption_plan_with_perm(
            &mut ctx,
            AbsorptionTarget::Hunks {
                hunks: paired_hunks,
            },
            guard.write_permission(),
        )?;

        assert_eq!(
            plan.len(),
            1,
            "a coupled pair must not be split across candidate destinations"
        );
        assert_eq!(
            plan[0].commit_id, first_region_commit,
            "the pair belongs to the commit that last changed its first-region lines"
        );
        assert_eq!(
            plan[0]
                .hunks
                .iter()
                .map(|hunk| hunk.hunk_header.expect("text hunk"))
                .collect::<Vec<_>>(),
            [
                but_core::HunkHeader {
                    old_start: 5,
                    old_lines: 1,
                    new_start: 0,
                    new_lines: 0,
                },
                but_core::HunkHeader {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 5,
                    new_lines: 1,
                },
            ],
            "the routed group retains both sides in caller order"
        );
        Ok(())
    }

    #[test]
    fn absorption_steps_preserve_source_hunk_groups() {
        let stack_id = StackId::generate();
        let commit_id = gix::ObjectId::from_hex(b"0000000000000000000000000000000000000000")
            .expect("valid object ID");
        let hunk_at = |path: &str, line| but_core::SingleHunk {
            hunk_header: Some(but_core::HunkHeader {
                old_start: line,
                old_lines: 1,
                new_start: line,
                new_lines: 1,
            }),
            path: path.into(),
            diff: None,
        };
        let absorption = |summary: &str, hunks| CommitAbsorption {
            stack_id,
            commit_id,
            blank_commit_ref: None,
            source_snapshot_tree: None,
            commit_summary: summary.to_owned(),
            hunks,
            reason: AbsorptionReason::HunkDependency,
        };
        let steps = absorption_steps_for_application(
            vec![
                absorption(
                    "A",
                    vec![
                        hunk_at("shared.txt", 10),
                        hunk_at("other.txt", 1),
                        hunk_at("shared.txt", 100),
                    ],
                ),
                absorption("B", vec![hunk_at("shared.txt", 50)]),
            ],
            &[
                but_core::SingleHunk {
                    hunk_header: Some(but_core::HunkHeader {
                        old_start: 1,
                        old_lines: 100,
                        new_start: 1,
                        new_lines: 100,
                    }),
                    path: "shared.txt".into(),
                    diff: None,
                },
                hunk_at("other.txt", 1),
            ],
        );

        assert_eq!(
            steps.len(),
            3,
            "selectors from independent source hunks become separate steps"
        );
        assert_eq!(steps[0].commit_summary, "A");
        assert_eq!(
            steps[0].hunks.len(),
            2,
            "selectors from one source hunk retain their shared coordinates"
        );
        assert_eq!(steps[1].commit_summary, "A");
        assert_eq!(
            steps[1].hunks.len(),
            1,
            "an independent file remains a separate step"
        );
        assert_eq!(steps[2].commit_summary, "B");
    }

    #[test]
    fn ambiguous_lock_targets_do_not_select_a_stack_by_workspace_order() {
        let a = StackId::generate();
        let b = StackId::generate();
        let lock = |target| HunkLock {
            target,
            commit_id: gix::ObjectId::from_hex(b"0000000000000000000000000000000000000000")
                .expect("valid object ID"),
        };

        assert_eq!(
            unique_lock_stack_id(&[
                lock(HunkLockTarget::Stack(a)),
                lock(HunkLockTarget::Stack(b))
            ]),
            None,
            "locks from parallel stacks do not have a child-most relationship"
        );
        assert_eq!(
            unique_lock_stack_id(&[
                lock(HunkLockTarget::Stack(a)),
                lock(HunkLockTarget::Unidentified)
            ]),
            None,
            "an unidentified lock cannot be ordered safely"
        );
        assert_eq!(
            unique_lock_stack_id(&[
                lock(HunkLockTarget::Stack(a)),
                lock(HunkLockTarget::Stack(a))
            ]),
            Some(a),
            "multiple locks in one stack remain eligible for child-most selection"
        );
    }

    #[test]
    fn candidate_ending_at_a_deletion_point_includes_its_lock() {
        let lock = HunkLock {
            target: HunkLockTarget::Stack(StackId::generate()),
            commit_id: gix::ObjectId::from_hex(b"0000000000000000000000000000000000000000")
                .expect("valid object ID"),
        };
        let dependency = but_core::unified_diff::DiffHunk {
            old_start: 27,
            old_lines: 1,
            new_start: 12,
            new_lines: 0,
            diff: "@@ -27,1 +12,0 @@\n-removed line\n".into(),
        };
        let index = HashMap::from([("file.txt".to_string(), vec![(dependency, vec![lock])])]);
        let candidate = AbsorbCandidate::from(but_core::SingleHunk {
            hunk_header: Some(but_core::HunkHeader {
                old_start: 3,
                old_lines: 24,
                new_start: 3,
                new_lines: 9,
            }),
            path: "file.txt".into(),
            diff: None,
        });

        assert_eq!(
            locks_for_candidate(&index, &candidate)
                .iter()
                .map(|lock| lock.commit_id)
                .collect::<Vec<_>>(),
            [lock.commit_id],
            "a shrinking candidate ending at a zero-line dependency includes the adjacent lock"
        );
    }
}
