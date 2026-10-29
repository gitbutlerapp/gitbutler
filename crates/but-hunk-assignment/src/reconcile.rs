use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use but_core::ref_metadata::StackId;
use itertools::Itertools;

use crate::HunkAssignment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultipleOverlapping {
    SetNone,
    SetMostLines,
}

impl HunkAssignment {
    fn set_from(
        &mut self,
        other: &Self,
        valid_branch_refs: &HashSet<&gix::refs::FullName>,
        update_unassigned: bool,
    ) {
        // Always set the path from the other assignment
        self.path = other.path.clone();
        // Override the id only if the other assignment has an id
        if other.id.is_some() {
            self.id = other.id;
        }
        // Override the lines added only if the other assignment has them set
        if other.line_nums_added.is_some() {
            self.line_nums_added = other.line_nums_added.clone();
        }
        // Override the lines removed only if the other assignment has them set
        if other.line_nums_removed.is_some() {
            self.line_nums_removed = other.line_nums_removed.clone();
        }

        // Override the branch target only if the current assignment already has one,
        // or if we're allowed to update previously unassigned hunks.
        match &self.branch_ref_bytes {
            Some(_) => {
                self.branch_ref_bytes = other.branch_ref_bytes.clone();
            }
            None => {
                if update_unassigned {
                    self.branch_ref_bytes = other.branch_ref_bytes.clone();
                }
            }
        }

        // Clear stale branch targets that no longer exist in the workspace.
        if let Some(branch_ref) = &self.branch_ref_bytes
            && !valid_branch_refs.contains(branch_ref)
        {
            self.branch_ref_bytes = None;
        }
    }
}

pub(crate) fn assignments(
    new: &[HunkAssignment],
    old: &[HunkAssignment],
    branches_by_stack: &HashMap<StackId, Vec<gix::refs::FullName>>,
    multiple_overlapping_resolution: MultipleOverlapping,
    update_unassigned: bool,
) -> Vec<HunkAssignment> {
    let valid_branch_refs: HashSet<&gix::refs::FullName> =
        branches_by_stack.values().flatten().collect();
    let mut reconciled = vec![];
    for new_assignment in new {
        let mut new_assignment = new_assignment.clone();
        let intersecting = old
            .iter()
            .filter(|current_entry| current_entry.intersects(new_assignment.clone()))
            .collect::<Vec<_>>();

        match intersecting.len().cmp(&1) {
            Ordering::Less => {
                // No intersection - do nothing, the None assignment is kept
            }
            Ordering::Equal => {
                new_assignment.set_from(intersecting[0], &valid_branch_refs, update_unassigned);
            }
            Ordering::Greater => {
                // Pick the hunk with the most lines to adopt the assignment info from.
                let biggest_hunk = intersecting
                    .iter()
                    .max_by_key(|h| h.hunk_header.as_ref().map(|h| h.new_lines));
                if let Some(other) = biggest_hunk {
                    new_assignment.set_from(other, &valid_branch_refs, update_unassigned);
                }

                // If requested, reset to unassigned when multiple conflicting branch
                // targets overlap into one hunk.
                let unique_branch_refs = intersecting
                    .iter()
                    .filter_map(|a| a.branch_ref_bytes.as_ref())
                    .unique();
                if multiple_overlapping_resolution == MultipleOverlapping::SetNone
                    && unique_branch_refs.count() > 1
                {
                    new_assignment.branch_ref_bytes = None;
                }
            }
        }
        reconciled.push(new_assignment);
    }
    reconciled
}
