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
use bstr::BString;
use but_core::UnifiedDiff;
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_workspace::{HunkHeader, StackId};
use gitbutler_command_context::CommandContext;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HunkAssignment {
    /// The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
    /// If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
    pub hunk_header: Option<HunkHeader>,
    /// The file path of the hunk.
    pub path_bytes: BString,
    /// The stack to which the hunk is assigned. If None, the hunk is not assigned to any stack.
    pub stack_id: Option<StackId>,
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

/// Returns the current hunk assignments for the workspace.
pub fn assignments(ctx: &CommandContext) -> Result<Vec<HunkAssignment>> {
    let state = state::AssignmentsHandle::new(&ctx.project().gb_dir());
    let assignments = state.assignments()?;
    Ok(assignments)
}

/// Sets the assignment for a hunk. It must be already present in the current assignments, errors out if it isn't.
/// If the stack is not in the list of applied stacks, it errors out.
/// Returns the updated assignments list.
pub fn assign(ctx: &CommandContext, new_assignment: HunkAssignment) -> Result<Vec<HunkAssignment>> {
    let state = state::AssignmentsHandle::new(&ctx.project().gb_dir());
    let previous_assignments = state.assignments()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let applied_stacks = vb_state
        .list_stacks_in_workspace()?
        .iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();
    let new_assignments = set_assignment(&applied_stacks, previous_assignments, new_assignment)?;
    let deps_assignments = hunk_dependency_assignments(ctx)?;
    let assignments_considering_deps = reconcile_assignments(
        new_assignments,
        &deps_assignments,
        &applied_stacks,
        MultiDepsResolution::SetNone, // If there is double locking, move the hunk to the Uncommitted section
    )?;
    state.set_assignments(assignments_considering_deps.clone())?;
    Ok(assignments_considering_deps)
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
/// This needs to be ran only after the worktree has changed.
/// TODO: When listing, we can reffer to a dirty bit to know if we need to run this.
pub fn reconcile(ctx: &CommandContext) -> Result<Vec<HunkAssignment>> {
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
        )?;
        let assignments_considering_deps = reconcile_assignments(
            assignments_considering_previous,
            &deps_assignments,
            &applied_stacks,
            MultiDepsResolution::SetNone, // If there is double locking, move the hunk to the Uncommitted section
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
) -> Result<Vec<HunkAssignment>> {
    let mut new_assignments = vec![];
    for mut worktree_entry in current_assignments {
        let intersecting = previous_assignments
            .iter()
            .filter(|current_entry| current_entry.intersects(worktree_entry.clone()))
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
                        worktree_entry.stack_id = intersecting[0].stack_id;
                    }
                }
            }
            Ordering::Greater => {
                match multi_deps_resolution {
                    MultiDepsResolution::SetNone => {
                        worktree_entry.stack_id = None;
                    }
                    MultiDepsResolution::SetMostLines => {
                        // More than one intersection - pick the one with the most lines
                        worktree_entry.stack_id = intersecting
                            .iter()
                            .max_by_key(|x| x.hunk_header.as_ref().map(|h| h.new_lines))
                            .and_then(|x| x.stack_id);
                    }
                }
            }
        }
        new_assignments.push(worktree_entry);
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
    let mut ass = vec![];
    for (path, hunk, locks) in deps {
        // If there are more than one locks, this means that the hunk depends on more than one stack and should have assignment None
        let stack_id = if locks.len() == 1 {
            Some(locks[0].stack_id)
        } else {
            None
        };
        let x = HunkAssignment {
            hunk_header: Some(hunk.into()),
            path_bytes: path.into(),
            stack_id,
        };
        ass.push(x);
    }
    Ok(ass)
}

fn diff_to_assignments(diff: UnifiedDiff, path: BString) -> Vec<HunkAssignment> {
    match diff {
        but_core::UnifiedDiff::Binary => vec![HunkAssignment {
            hunk_header: None,
            path_bytes: path,
            stack_id: None,
        }],
        but_core::UnifiedDiff::TooLarge { .. } => vec![HunkAssignment {
            hunk_header: None,
            path_bytes: path,
            stack_id: None,
        }],
        but_core::UnifiedDiff::Patch {
            hunks,
            is_result_of_binary_to_text_conversion,
            ..
        } => {
            if is_result_of_binary_to_text_conversion {
                vec![HunkAssignment {
                    hunk_header: None,
                    path_bytes: path,
                    stack_id: None,
                }]
            } else {
                hunks
                    .iter()
                    .map(|hunk| HunkAssignment {
                        hunk_header: Some(hunk.into()),
                        path_bytes: path.clone(),
                        stack_id: None,
                    })
                    .collect()
            }
        }
    }
}

fn set_assignment(
    applied_stacks: &[StackId],
    mut previous_assignments: Vec<HunkAssignment>,
    new_assignment: HunkAssignment,
) -> Result<Vec<HunkAssignment>> {
    if let Some(stack_id) = new_assignment.stack_id {
        if !applied_stacks.contains(&stack_id) {
            return Err(anyhow::anyhow!("No such stack in the workspace"));
        }
    }
    if !previous_assignments.contains(&new_assignment) {
        return Err(anyhow::anyhow!("Hunk not found in current assignments"));
    }
    if let Some(found) = previous_assignments
        .iter_mut()
        .find(|x| **x == new_assignment)
    {
        found.stack_id = new_assignment.stack_id;
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
        )
        .unwrap();
        assert_eq!(result, vec![ass("foo.rs", 5, 18, None)]);
    }

    #[test]
    fn test_set_assignment_success() {
        let applied_stacks = vec![id(1), id(2)];
        let previous_assignments =
            vec![ass("foo.rs", 10, 15, None), ass("bar.rs", 20, 25, Some(1))];

        // Assign foo.rs:10 to stack 2
        let new_assignment = ass("foo.rs", 10, 15, Some(2));
        let updated = set_assignment(
            &applied_stacks,
            previous_assignments.clone(),
            new_assignment.clone(),
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
        let new_assignment = ass("foo.rs", 10, 15, Some(3));
        let result = set_assignment(
            &applied_stacks,
            previous_assignments.clone(),
            new_assignment.clone(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_set_assignment_hunk_not_found() {
        let applied_stacks = vec![id(1), id(2)];
        let previous_assignments =
            vec![ass("foo.rs", 10, 15, None), ass("bar.rs", 20, 25, Some(1))];

        // Assign baz.rs:30 to stack 2 (not found)
        let new_assignment = ass("baz.rs", 30, 35, Some(2));
        let result = set_assignment(
            &applied_stacks,
            previous_assignments.clone(),
            new_assignment.clone(),
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
