use std::collections::{BTreeMap, HashSet, btree_map::Entry};

use bstr::BString;
use nonempty::NonEmpty;

use crate::id::id_usage::UintId;

/// Information about uncommitted files.
pub(crate) struct UncommittedInfo {
    /// Uncommitted hunks partitioned by filename.
    pub(crate) partitioned_hunks: Vec<NonEmpty<but_core::SingleHunk>>,
    pub(crate) uncommitted_short_filenames: HashSet<BString>,
}

impl UncommittedInfo {
    /// Partitions hunks by filename.
    pub(crate) fn from_hunks(hunks: Vec<but_core::SingleHunk>) -> anyhow::Result<Self> {
        let mut uncommitted_hunks: BTreeMap<BString, NonEmpty<_>> = BTreeMap::new();
        let mut uncommitted_short_filenames = HashSet::new();
        for hunk in hunks {
            if hunk.path.len() <= UintId::LENGTH_LIMIT
                && !uncommitted_short_filenames.contains(&hunk.path)
            {
                uncommitted_short_filenames.insert(hunk.path.clone());
            }
            match uncommitted_hunks.entry(hunk.path.clone()) {
                Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(NonEmpty::new(hunk));
                }
                Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.get_mut().push(hunk);
                }
            };
        }

        Ok(Self {
            partitioned_hunks: uncommitted_hunks.into_values().collect(),
            uncommitted_short_filenames,
        })
    }
}
