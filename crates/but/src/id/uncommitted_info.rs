use std::collections::{BTreeMap, HashSet, btree_map::Entry};

use bstr::BString;
use nonempty::NonEmpty;

use crate::{
    id::id_usage::UintId,
    utils::change_source::{ChangeSourceId, SourceChanges},
};

/// Information about uncommitted files.
pub(crate) struct UncommittedInfo {
    /// Uncommitted hunks partitioned by the checkout they come from and their filename.
    ///
    /// Ordered by source and then path, so the same working state always yields
    /// the same IDs.
    pub(crate) partitioned_hunks: Vec<(ChangeSourceId, NonEmpty<but_core::SingleHunk>)>,
    /// The short filenames of every source, which all compete for the same short
    /// IDs as branches do.
    pub(crate) uncommitted_short_filenames: HashSet<BString>,
}

impl UncommittedInfo {
    /// Partitions the hunks of every source by source and filename.
    pub(crate) fn from_sources(sources: Vec<SourceChanges>) -> anyhow::Result<Self> {
        let mut uncommitted_hunks: BTreeMap<(ChangeSourceId, BString), NonEmpty<_>> =
            BTreeMap::new();
        let mut uncommitted_short_filenames = HashSet::new();
        for SourceChanges { source, hunks, .. } in sources {
            for hunk in hunks {
                if hunk.path.len() <= UintId::LENGTH_LIMIT
                    && !uncommitted_short_filenames.contains(&hunk.path)
                {
                    uncommitted_short_filenames.insert(hunk.path.clone());
                }
                match uncommitted_hunks.entry((source.clone(), hunk.path.clone())) {
                    Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(NonEmpty::new(hunk));
                    }
                    Entry::Occupied(mut occupied_entry) => {
                        occupied_entry.get_mut().push(hunk);
                    }
                };
            }
        }

        Ok(Self {
            partitioned_hunks: uncommitted_hunks
                .into_iter()
                .map(|((source, _path), hunks)| (source, hunks))
                .collect(),
            uncommitted_short_filenames,
        })
    }
}
