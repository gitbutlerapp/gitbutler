//! Tools for manipulating trees

/// Provides data that helps describe the effect of the move changes operation.
pub struct MoveChangesResult {
    /// A list of commits that were replaced as part of any rebases that were
    /// performed. Provided as a list of tuples where the first item in the
    /// tuple is the "before" and the second item in the tuple is the "after"
    /// id.
    ///
    /// If a commit was unaffected then it will not be included in this list.
    pub replaced_commits: Vec<(gix::ObjectId, gix::ObjectId)>,
}

impl MoveChangesResult {
    /// Merges the changes from another `MoveChangesResult` into this one.
    pub fn merge(&mut self, other: MoveChangesResult) {
        let mut new_replaced_commits = self.replaced_commits.clone();

        for (before, after) in other.replaced_commits {
            let matching_commit_mapping =
                new_replaced_commits.iter_mut().find(|(_, a)| *a == before);
            if let Some(found_matching_mapping) = matching_commit_mapping {
                // If we found a matching commit mapping, we update the "after" id
                // to the new "after" id.
                found_matching_mapping.1 = after;
            } else {
                // Otherwise, we add the new mapping.
                new_replaced_commits.push((before, after));
            }
        }

        self.replaced_commits = new_replaced_commits;
    }
}

pub(super) mod move_between_commits;
pub(super) mod remove_changes_from_commit_in_stack;
pub(super) mod split_branch;
pub(super) mod split_commit;
mod utils;
