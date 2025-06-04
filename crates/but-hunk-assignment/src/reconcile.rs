use std::cmp::Ordering;

use anyhow::Result;
use but_workspace::StackId;
use itertools::Itertools;

use crate::HunkAssignment;

pub enum MultipleOverlapping {
    SetNone,
    SetMostLines,
}

pub(crate) fn assignments(
    new: Vec<HunkAssignment>,
    old: &[HunkAssignment],
    applied_stack_ids: &[StackId],
    multiple_overlapping_resolution: MultipleOverlapping,
    update_unassigned: bool,
) -> Result<Vec<HunkAssignment>> {
    let mut reconciled = vec![];
    for mut new_assignment in new {
        let intersecting = old
            .iter()
            .filter(|current_entry| current_entry.intersects(new_assignment.clone()))
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
                    if applied_stack_ids.contains(&stack_id) {
                        if update_unassigned && new_assignment.stack_id.is_none() {
                            // In the case where there was no assignment but a hunk lock was detected, we dont want to automatically assign the hunk to the lane where it depends
                            new_assignment.stack_id = intersecting[0].stack_id;
                        }
                        new_assignment.hunk_locks = intersecting[0].hunk_locks.clone();
                        if intersecting[0].id.is_some() {
                            new_assignment.id = intersecting[0].id;
                        }
                    }
                }
            }
            Ordering::Greater => {
                match multiple_overlapping_resolution {
                    MultipleOverlapping::SetNone => {
                        new_assignment.stack_id = None;
                    }
                    MultipleOverlapping::SetMostLines => {
                        // More than one intersection - pick the one with the most lines
                        new_assignment.stack_id = intersecting
                            .iter()
                            .max_by_key(|x| x.hunk_header.as_ref().map(|h| h.new_lines))
                            .and_then(|x| x.stack_id);
                    }
                }

                if intersecting[0].id.is_some() {
                    new_assignment.id = intersecting[0].id; // Use the id of the first intersecting assignment as the id for the new assignment
                }

                // Inherit all locks from the intersecting assignments
                let all_locks = intersecting
                    .iter()
                    .flat_map(|i| i.hunk_locks.clone())
                    .unique()
                    .collect::<Vec<_>>();
                new_assignment.hunk_locks = all_locks;
            }
        }
        reconciled.push(new_assignment);
    }
    Ok(reconciled)
}
