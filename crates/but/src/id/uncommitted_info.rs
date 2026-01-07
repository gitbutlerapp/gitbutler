use std::collections::{BTreeMap, HashSet, btree_map::Entry};

use bstr::BString;
use but_core::ref_metadata::StackId;
use but_hunk_assignment::HunkAssignment;
use nonempty::NonEmpty;

use crate::id::id_usage::UintId;

/// Information about uncommitted files.
pub(crate) struct UncommittedInfo {
    /// Uncommitted hunks partitioned by stack assignment and filename.
    pub(crate) partitioned_hunks: Vec<NonEmpty<HunkAssignment>>,
    pub(crate) uncommitted_short_filenames: HashSet<BString>,
}

impl UncommittedInfo {
    /// Partitions hunk assignments by stack assignment and filename.
    pub(crate) fn from_hunk_assignments(
        hunk_assignments: Vec<HunkAssignment>,
    ) -> anyhow::Result<Self> {
        let mut uncommitted_hunks: BTreeMap<(Option<StackId>, BString), NonEmpty<HunkAssignment>> =
            BTreeMap::new();
        let mut uncommitted_short_filenames = HashSet::new();
        for assignment in hunk_assignments {
            if assignment.path_bytes.len() <= UintId::LENGTH_LIMIT
                && !uncommitted_short_filenames.contains(&assignment.path_bytes)
            {
                uncommitted_short_filenames.insert(assignment.path_bytes.clone());
            }
            // Rust does not let us borrow a tuple from 2 separate fields, so
            // we have to clone the parts of the key even though we technically
            // might not need it.
            let key = (assignment.stack_id, assignment.path_bytes.clone());
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
