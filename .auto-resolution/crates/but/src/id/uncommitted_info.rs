use std::collections::{BTreeMap, HashSet, btree_map::Entry};

use bstr::BString;
use but_hunk_assignment::HunkAssignment;
use nonempty::NonEmpty;

use crate::id::id_usage::UintId;

/// Information about uncommitted files.
pub(crate) struct UncommittedInfo {
    /// Uncommitted hunks partitioned by branch assignment and filename.
    pub(crate) partitioned_hunks: Vec<NonEmpty<HunkAssignment>>,
    pub(crate) uncommitted_short_filenames: HashSet<BString>,
}

/// A key that groups hunks by their assignment target and file path.
/// Uses `branch_ref_bytes` as the primary discriminator, falling back
/// to `stack_id` (as string bytes) when no branch ref is available.
/// This ensures hunks assigned to different stacks but with no branch
/// ref don't collapse into the same bucket.
type PartitionKey = (Option<BString>, BString);

impl UncommittedInfo {
    /// Partitions hunk assignments by branch assignment and filename.
    pub(crate) fn from_hunk_assignments(
        hunk_assignments: Vec<HunkAssignment>,
    ) -> anyhow::Result<Self> {
        let mut uncommitted_hunks: BTreeMap<PartitionKey, NonEmpty<HunkAssignment>> =
            BTreeMap::new();
        let mut uncommitted_short_filenames = HashSet::new();
        for assignment in hunk_assignments {
            if assignment.path_bytes.len() <= UintId::LENGTH_LIMIT
                && !uncommitted_short_filenames.contains(&assignment.path_bytes)
            {
                uncommitted_short_filenames.insert(assignment.path_bytes.clone());
            }
            // Use branch_ref_bytes as the primary grouping key, falling back to
            // stack_id when branch_ref_bytes is None to prevent cross-stack collapse.
            let assignment_key = assignment
                .branch_ref_bytes
                .as_ref()
                .map(|r| BString::from(r.as_bstr()))
                .or_else(|| assignment.stack_id.map(|id| BString::from(id.to_string())));
            let key = (assignment_key, assignment.path_bytes.clone());
            match uncommitted_hunks.entry(key) {
                Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(NonEmpty::new(assignment));
                }
                Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.get_mut().push(assignment);
                }
            };
        }

        Ok(Self {
            partitioned_hunks: uncommitted_hunks.into_values().collect(),
            uncommitted_short_filenames,
        })
    }
}
