//!
//!
//! Hunk - File, range
//!
//! HunkAssignment - None or Some(Stack in workspace)
//!
//! reconcile_assignments - takes worktree changes (Vec<TreeChange>) + current assignments (Vec<HunkAssignment>)
//! returns updated assignments (Vec<HunkAssignment>)
//!
//! set_assignments

mod state;
use std::cmp::Ordering;

use anyhow::Result;
use bstr::{BString, ByteSlice};
use but_core::UnifiedDiff;
use but_hunk_dependency::ui::{HunkLock, hunk_dependencies_for_workspace_changes_by_worktree_dir};
use but_workspace::{HunkHeader, StackId};
use gitbutler_command_context::CommandContext;
use gitbutler_stack::VirtualBranchesHandle;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HunkAssignment {
    /// The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
    /// If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
    pub hunk_header: Option<HunkHeader>,
    /// The file path of the hunk.
    pub path: String,
    /// The file path of the hunk in bytes.
    pub path_bytes: BString,
    /// The stack to which the hunk is assigned. If None, the hunk is not assigned to any stack.
    pub stack_id: Option<StackId>,
    /// The dependencies(locks) that this hunk has. This determines where the hunk can be assigned.
    /// This field is ignored when HunkAssignment is passed by the UI to create a new assignment.
    pub hunk_locks: Vec<HunkLock>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// Indicates that the assignment request was rejected due to locking - the hunk depends on a commit in the stack it is currently in.
pub struct AssignmentRejection {
    /// The request that was rejected.
    request: HunkAssignmentRequest,
    /// The locks that caused the rejection.
    locks: Vec<HunkLock>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// A request to update a hunk assignment.
/// If a a file has multiple hunks, the UI client should send a list of assignment requests with the appropriate hunk headers.
pub struct HunkAssignmentRequest {
    /// The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
    /// If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
    /// If the file has hunk headers, then header info MUST be provided.
    pub hunk_header: Option<HunkHeader>,
    /// The file path of the hunk in bytes.
    pub path_bytes: BString,
    /// The stack to which the hunk is assigned. If set to None, the hunk is set as "unassigned".
    /// If a stack id is set, it must be one of the applied stacks.
    pub stack_id: Option<StackId>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Same as `but_core::ui::WorktreeChanges`, but with the addition of hunk assignments.
pub struct WorktreeChanges {
    #[serde(flatten)]
    pub worktree_changes: but_core::ui::WorktreeChanges,
    pub assignments: Result<Vec<HunkAssignment>, serde_error::Error>,
}

impl From<but_core::ui::WorktreeChanges> for WorktreeChanges {
    fn from(worktree_changes: but_core::ui::WorktreeChanges) -> Self {
        WorktreeChanges {
            worktree_changes,
            assignments: Ok(vec![]),
        }
    }
}

impl From<but_core::WorktreeChanges> for WorktreeChanges {
    fn from(worktree_changes: but_core::WorktreeChanges) -> Self {
        let ui_changes: but_core::ui::WorktreeChanges = worktree_changes.into();
        ui_changes.into()
    }
}

impl HunkAssignmentRequest {
    pub fn matches_assignment(&self, assignment: &HunkAssignment) -> bool {
        self.path_bytes == assignment.path_bytes && self.hunk_header == assignment.hunk_header
    }
}

impl PartialEq for HunkAssignment {
    fn eq(&self, other: &Self) -> bool {
        self.hunk_header == other.hunk_header && self.path_bytes == other.path_bytes
    }
}

impl HunkAssignment {
    /// Whether there is overlap between the two hunks.
    fn intersects(&self, other: HunkAssignment) -> bool {
        if self == &other {
            return true;
        }
        if self.path_bytes != other.path_bytes {
            return false;
        }
        if self.hunk_header == other.hunk_header {
            return true;
        }
        if let (Some(header), Some(other_header)) = (self.hunk_header, other.hunk_header) {
            if header.new_start >= other_header.new_start
                && header.new_start < other_header.new_start + other_header.new_lines
            {
                return true;
            }
            if other_header.new_start >= header.new_start
                && other_header.new_start < header.new_start + header.new_lines
            {
                return true;
            }
        }
        false
    }
}

#[instrument(skip(ctx), err(Debug))]
/// Returns the current hunk assignments for the workspace.
pub fn assignments(ctx: &CommandContext) -> Result<Vec<HunkAssignment>> {
    // TODO: Use a dirty bit set in the file watcher to indicate when reconcilation is needed.
    if true {
        reconcile(ctx)
    } else {
        let state = state::AssignmentsHandle::new(&ctx.project().gb_dir());
        let assignments = state.assignments()?;
        Ok(assignments)
    }
}

/// Sets the assignment for a hunk. It must be already present in the current assignments, errors out if it isn't.
/// If the stack is not in the list of applied stacks, it errors out.
/// Returns the updated assignments list.
pub fn assign(
    ctx: &CommandContext,
    requests: Vec<HunkAssignmentRequest>,
) -> Result<Vec<AssignmentRejection>> {
    let state = state::AssignmentsHandle::new(&ctx.project().gb_dir());
    let previous_assignments = state.assignments()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let applied_stacks = vb_state
        .list_stacks_in_workspace()?
        .iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();
    let new_assignments = set_assignment(
        &applied_stacks,
        previous_assignments.clone(),
        requests.clone(),
    )?;
    let deps_assignments = hunk_dependency_assignments(ctx)?;
    let assignments_considering_deps = reconcile_assignments(
        new_assignments,
        &deps_assignments,
        &applied_stacks,
        MultiDepsResolution::SetNone, // If there is double locking, move the hunk to the Uncommitted section
        true,
    )?;
    state.set_assignments(assignments_considering_deps.clone())?;

    // Request where the stack_id is different from the outcome are considered rejections - this is due to locking
    // Collect all the rejected requests together with the locks that caused the rejection
    let mut rejections = vec![];
    for req in requests {
        let locks = assignments_considering_deps
            .iter()
            .filter(|assignment| {
                req.matches_assignment(assignment) && req.stack_id != assignment.stack_id
            })
            .flat_map(|assignment| assignment.hunk_locks.clone())
            .collect_vec();
        if !locks.is_empty() {
            rejections.push(AssignmentRejection {
                request: req.clone(),
                locks,
            });
        }
    }
    Ok(rejections)
}

/// Reconciles the current hunk assignments with the current worktree changes.
/// It takes  the current hunk assignments as well as the current worktree changes, producing a new set of hunk assignments.
///
/// Inside, fuzzy matching is performed, such that hunks that were previously assigned
/// and now in they have been modified in the worktree are still assigned to the same stack.
///
/// If a hunk was deleted in the worktree, it is removed from the assignments list.
/// If a completely new hunk is added it is accounted for with a HunkAssignment with stack_id None.
///
/// If a stack is no longer present in the workspace (either unapplied or deleted), any assignments to it are removed.
///
/// If a hunk has a dependency on a particular stack and it has been previously assigned to another stack, the assignment is updated to reflect that dependency.
/// If a hunk has a dependency but it has not been previously assigned to any stack, it is left unassigned (stack_id is None). This is so that the hunk assignment workflow can remain optional.
///
/// This needs to be ran only after the worktree has changed.
fn reconcile(ctx: &CommandContext) -> Result<Vec<HunkAssignment>> {
    let state = state::AssignmentsHandle::new(&ctx.project().gb_dir());
    let previous_assignments = state.assignments()?;
    let repo = &ctx.gix_repo()?;
    let context_lines = ctx.app_settings().context_lines;
    let worktree_changes = but_core::diff::worktree_changes(repo)?.changes;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let applied_stacks = vb_state
        .list_stacks_in_workspace()?
        .iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();

    let deps_assignments = hunk_dependency_assignments(ctx)?;

    let mut new_assignments = vec![];
    for change in worktree_changes {
        let diff = change.unified_diff(repo, context_lines)?;
        let assignments_from_worktree = diff_to_assignments(diff, change.path);
        let assignments_considering_previous = reconcile_assignments(
            assignments_from_worktree,
            &previous_assignments,
            &applied_stacks,
            MultiDepsResolution::SetMostLines,
            true,
        )?;
        let assignments_considering_deps = reconcile_assignments(
            assignments_considering_previous,
            &deps_assignments,
            &applied_stacks,
            MultiDepsResolution::SetNone, // If there is double locking, move the hunk to the Uncommitted section
            false,
        )?;
        new_assignments.extend(assignments_considering_deps);
    }
    state.set_assignments(new_assignments.clone())?;
    Ok(new_assignments)
}

enum MultiDepsResolution {
    SetNone,
    SetMostLines,
}

fn reconcile_assignments(
    current_assignments: Vec<HunkAssignment>,
    previous_assignments: &[HunkAssignment],
    applied_stacks: &[StackId],
    multi_deps_resolution: MultiDepsResolution,
    update_unassigned: bool,
) -> Result<Vec<HunkAssignment>> {
    let mut new_assignments = vec![];
    for mut current_assignment in current_assignments {
        let intersecting = previous_assignments
            .iter()
            .filter(|current_entry| current_entry.intersects(current_assignment.clone()))
            .collect::<Vec<_>>();

        // If the worktree hunk intersects with exactly one previous assignment, then it inherits the stack_id assignment from it, but only if the stack is still applied.
        // If the worktree hunk does not interesect with any previous assingment then it remains, "unassigned", i.e. with stack_id None
        // If the worktree hunk intersects with more that one one previous assignment there is special handling
        match intersecting.len().cmp(&1) {
            Ordering::Less => {
                // No intersection - do nothing, the None assignment is kept
            }
            Ordering::Equal => {
                // One intersection - assign the stack id if the stack is still in the applied list
                if let Some(stack_id) = intersecting[0].stack_id {
                    if applied_stacks.contains(&stack_id) {
                        if update_unassigned && current_assignment.stack_id.is_none() {
                            // In the case where there was no assignment but a hunk lock was detected, we dont want to automatically assign the hunk to the lane where it depends
                            current_assignment.stack_id = intersecting[0].stack_id;
                        }
                        current_assignment.hunk_locks = intersecting[0].hunk_locks.clone();
                    }
                }
            }
            Ordering::Greater => {
                match multi_deps_resolution {
                    MultiDepsResolution::SetNone => {
                        current_assignment.stack_id = None;
                    }
                    MultiDepsResolution::SetMostLines => {
                        // More than one intersection - pick the one with the most lines
                        current_assignment.stack_id = intersecting
                            .iter()
                            .max_by_key(|x| x.hunk_header.as_ref().map(|h| h.new_lines))
                            .and_then(|x| x.stack_id);
                    }
                }

                // Inherit all locks from the intersecting assignments
                let all_locks = intersecting
                    .iter()
                    .flat_map(|i| i.hunk_locks.clone())
                    .unique()
                    .collect::<Vec<_>>();
                current_assignment.hunk_locks = all_locks;
            }
        }
        new_assignments.push(current_assignment);
    }
    Ok(new_assignments)
}

fn hunk_dependency_assignments(ctx: &CommandContext) -> Result<Vec<HunkAssignment>> {
    // NB(Performance): This will do some extra work - in particular, worktree_changes will be fetched again, but that is a fast operation.
    // Furthermore, this call will compute unified_diff for each change. While this is a slower operation, it is invoked with zero context lines,
    // and that seems appropriate for limiting locking to only real overlaps.
    let deps = hunk_dependencies_for_workspace_changes_by_worktree_dir(
        ctx,
        &ctx.project().path,
        &ctx.project().gb_dir(),
    )?
    .diffs;
    let mut assignments = vec![];
    for (path, hunk, locks) in deps {
        // If there are locks towards more than one stack, this means double locking and the assignment None - the user can resolve this by partial committing.
        let locked_to_stack_ids_count = locks
            .iter()
            .map(|lock| lock.stack_id)
            .collect::<std::collections::HashSet<_>>()
            .len();
        let stack_id = if locked_to_stack_ids_count == 1 {
            Some(locks[0].stack_id)
        } else {
            None
        };
        let assignment = HunkAssignment {
            hunk_header: Some(hunk.into()),
            path: path.clone(),
            path_bytes: path.into(),
            stack_id,
            hunk_locks: locks,
        };
        assignments.push(assignment);
    }
    Ok(assignments)
}

fn diff_to_assignments(diff: UnifiedDiff, path: BString) -> Vec<HunkAssignment> {
    let path_str = path.to_str_lossy();
    match diff {
        but_core::UnifiedDiff::Binary => vec![HunkAssignment {
            hunk_header: None,
            path: path_str.into(),
            path_bytes: path,
            stack_id: None,
            hunk_locks: vec![],
        }],
        but_core::UnifiedDiff::TooLarge { .. } => vec![HunkAssignment {
            hunk_header: None,
            path: path_str.into(),
            path_bytes: path,
            stack_id: None,
            hunk_locks: vec![],
        }],
        but_core::UnifiedDiff::Patch {
            hunks,
            is_result_of_binary_to_text_conversion,
            ..
        } => {
            // If there are no hunks, then the assignment is for the whole file
            if is_result_of_binary_to_text_conversion || hunks.is_empty() {
                vec![HunkAssignment {
                    hunk_header: None,
                    path: path_str.into(),
                    path_bytes: path,
                    stack_id: None,
                    hunk_locks: vec![],
                }]
            } else {
                hunks
                    .iter()
                    .map(|hunk| HunkAssignment {
                        hunk_header: Some(hunk.into()),
                        path: path_str.clone().into(),
                        path_bytes: path.clone(),
                        stack_id: None,
                        hunk_locks: vec![],
                    })
                    .collect()
            }
        }
    }
}

fn set_assignment(
    applied_stacks: &[StackId],
    mut previous_assignments: Vec<HunkAssignment>,
    new_assignments: Vec<HunkAssignmentRequest>,
) -> Result<Vec<HunkAssignment>> {
    for new_assignment in new_assignments {
        if let Some(stack_id) = new_assignment.stack_id {
            if !applied_stacks.contains(&stack_id) {
                return Err(anyhow::anyhow!("No such stack in the workspace"));
            }
        }
        if let Some(found) = previous_assignments
            .iter_mut()
            .find(|previous| new_assignment.matches_assignment(previous))
        {
            found.stack_id = new_assignment.stack_id;
        } else {
            return Err(anyhow::anyhow!("Hunk not found in current assignments"));
        }
    }
    Ok(previous_assignments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::BString;
    use but_workspace::{HunkHeader, StackId};

    fn ass(path: &str, start: u32, end: u32, stack_id: Option<usize>) -> HunkAssignment {
        HunkAssignment {
            hunk_header: Some(HunkHeader {
                old_start: 0,
                old_lines: 0,
                new_start: start,
                new_lines: end,
            }),
            path: path.to_string(),
            path_bytes: BString::from(path),
            stack_id: stack_id.map(id),
            hunk_locks: vec![],
        }
    }
    fn ass_req(path: &str, start: u32, end: u32, stack_id: Option<usize>) -> HunkAssignmentRequest {
        HunkAssignmentRequest {
            hunk_header: Some(HunkHeader {
                old_start: 0,
                old_lines: 0,
                new_start: start,
                new_lines: end,
            }),
            path_bytes: BString::from(path),
            stack_id: stack_id.map(id),
        }
    }
    fn id(num: usize) -> StackId {
        StackId::from(
            uuid::Uuid::parse_str(&format!("00000000-0000-0000-0000-00000000000{}", num % 10))
                .unwrap(),
        )
    }

    #[test]
    fn test_reconcile_exact_match_and_no_intersection() {
        let previous_assignments = vec![ass("foo.rs", 10, 15, Some(1))];
        let worktree_assignments = vec![ass("foo.rs", 10, 15, None), ass("foo.rs", 16, 20, None)];
        let applied_stacks = vec![id(1), id(2)];
        let result = reconcile_assignments(
            worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultiDepsResolution::SetMostLines,
            true,
        )
        .unwrap();
        assert_eq!(
            result,
            vec![ass("foo.rs", 10, 15, Some(1)), ass("foo.rs", 16, 20, None)]
        );
    }

    #[test]
    fn test_reconcile_exact_match_unapplied_branch_unassigns() {
        let previous_assignments = vec![ass("foo.rs", 10, 15, Some(1))];
        let worktree_assignments = vec![ass("foo.rs", 10, 15, None)];
        let applied_stacks = vec![id(2)];
        let result = reconcile_assignments(
            worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultiDepsResolution::SetMostLines,
            true,
        )
        .unwrap();
        assert_eq!(result, vec![ass("foo.rs", 10, 15, None)]);
    }

    #[test]
    fn test_reconcile_with_overlap_preserves_assignment() {
        let previous_assignments = vec![ass("foo.rs", 10, 15, Some(1))];
        let worktree_assignments = vec![ass("foo.rs", 12, 17, None)];
        let applied_stacks = vec![id(1)];
        let result = reconcile_assignments(
            worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultiDepsResolution::SetMostLines,
            true,
        )
        .unwrap();
        assert_eq!(result, vec![ass("foo.rs", 12, 17, Some(1))]);
    }

    #[test]
    fn test_double_overlap_picks_the_bigger_previous_assignment() {
        let previous_assignments = vec![
            ass("foo.rs", 5, 15, Some(1)),
            ass("foo.rs", 17, 25, Some(2)),
        ];
        let applied_stacks = vec![id(1), id(2)];
        let worktree_assignments = vec![ass("foo.rs", 5, 18, None)];
        let result = reconcile_assignments(
            worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultiDepsResolution::SetMostLines,
            true,
        )
        .unwrap();
        assert_eq!(result, vec![ass("foo.rs", 5, 18, Some(1))]);
    }

    #[test]
    fn test_double_overlap_unassigns() {
        let previous_assignments = vec![
            ass("foo.rs", 5, 15, Some(1)),
            ass("foo.rs", 17, 25, Some(2)),
        ];
        let applied_stacks = vec![id(1), id(2)];
        let worktree_assignments = vec![ass("foo.rs", 5, 18, None)];
        let result = reconcile_assignments(
            worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultiDepsResolution::SetNone,
            true,
        )
        .unwrap();
        assert_eq!(result, vec![ass("foo.rs", 5, 18, None)]);
    }

    #[test]
    fn test_reconcile_not_updating_unassigned() {
        let previous_assignments = vec![ass("foo.rs", 10, 15, Some(1))];
        let worktree_assignments = vec![ass("foo.rs", 12, 17, None)];
        let applied_stacks = vec![id(1)];
        let result = reconcile_assignments(
            worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultiDepsResolution::SetMostLines,
            true,
        )
        .unwrap();
        assert_eq!(result, vec![ass("foo.rs", 12, 17, None)]);
    }

    #[test]
    fn test_set_assignment_success() {
        let applied_stacks = vec![id(1), id(2)];
        let previous_assignments =
            vec![ass("foo.rs", 10, 15, None), ass("bar.rs", 20, 25, Some(1))];

        // Assign foo.rs:10 to stack 2
        let new_assignment_req = ass_req("foo.rs", 10, 15, Some(2));
        let new_assignment = ass("foo.rs", 10, 15, Some(2));
        let updated = set_assignment(
            &applied_stacks,
            previous_assignments.clone(),
            vec![new_assignment_req],
        )
        .unwrap();

        // Should update the stack_id for foo.rs:10
        let found = updated.iter().find(|h| **h == new_assignment).unwrap();
        assert_eq!(found.stack_id, Some(id(2)));

        // Other assignments should remain unchanged
        let other = updated
            .iter()
            .find(|h| **h == ass("bar.rs", 20, 25, Some(1)))
            .unwrap();
        assert_eq!(other.stack_id, Some(id(1)));
    }

    #[test]
    fn test_set_assignment_stack_not_applied() {
        let applied_stacks = vec![id(1), id(2)];
        let previous_assignments =
            vec![ass("foo.rs", 10, 15, None), ass("bar.rs", 20, 25, Some(1))];

        // Assign foo.rs:10 to stack 3 (not applied)
        let new_assignment = ass_req("foo.rs", 10, 15, Some(3));
        let result = set_assignment(
            &applied_stacks,
            previous_assignments.clone(),
            vec![new_assignment.clone()],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_set_assignment_hunk_not_found() {
        let applied_stacks = vec![id(1), id(2)];
        let previous_assignments =
            vec![ass("foo.rs", 10, 15, None), ass("bar.rs", 20, 25, Some(1))];

        // Assign baz.rs:30 to stack 2 (not found)
        let new_assignment = ass_req("baz.rs", 30, 35, Some(2));
        let result = set_assignment(
            &applied_stacks,
            previous_assignments.clone(),
            vec![new_assignment.clone()],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_hunk_assignment_partial_eq() {
        let hunk1 = ass("foo.rs", 10, 15, Some(1));
        let hunk2 = ass("foo.rs", 10, 15, Some(2));
        assert_eq!(hunk1, hunk2);
    }

    #[test]
    fn test_hunk_assignment_partial_eq_different_path() {
        let hunk1 = ass("foo.rs", 10, 15, Some(1));
        let hunk2 = ass("bar.rs", 10, 15, Some(2));
        assert_ne!(hunk1, hunk2);
    }
}
