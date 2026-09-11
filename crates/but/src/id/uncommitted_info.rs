use std::collections::HashSet;

use bstr::BString;
use nonempty::NonEmpty;

use crate::{
    id::id_usage::UintId,
    utils::change_source::{ChangeSourceId, SourceChanges},
};

/// Information about uncommitted files.
pub(crate) struct UncommittedInfo {
    /// Uncommitted changes and associated hunks partitioned by the checkout they come from and
    /// their path.
    ///
    /// Ordered by source and then path, so the same working state always yields the same IDs.
    pub(crate) partitioned_changes_and_hunks: Vec<(
        ChangeSourceId,
        but_core::ui::TreeChange,
        NonEmpty<but_core::SingleHunk>,
    )>,
    /// The short filenames of every source, which all compete for the same short
    /// IDs as branches do.
    pub(crate) uncommitted_short_filenames: HashSet<BString>,
}

impl UncommittedInfo {
    /// Creates an [`UncommittedInfo`] from any amount of [`SourceChanges`].
    pub(crate) fn from_sources(sources: impl IntoIterator<Item = SourceChanges>) -> Self {
        let mut uncommitted_short_filenames = HashSet::new();
        let mut partitioned_changes_and_hunks: Vec<(
            ChangeSourceId,
            but_core::ui::TreeChange,
            NonEmpty<but_core::SingleHunk>,
        )> = vec![];

        for SourceChanges {
            source,
            changes_with_hunks,
        } in sources
        {
            for (change, hunks) in changes_with_hunks {
                if change.path.len() <= UintId::LENGTH_LIMIT {
                    uncommitted_short_filenames.insert(change.path.clone());
                }
                partitioned_changes_and_hunks.push((source.clone(), change.into(), hunks));
            }
        }

        partitioned_changes_and_hunks.sort_by(
            |(lhs_source_id, lhs_change, _), (rhs_source_id, rhs_change, _)| {
                lhs_source_id
                    .cmp(rhs_source_id)
                    .then_with(|| lhs_change.path_bytes.cmp(&rhs_change.path_bytes))
            },
        );

        Self {
            partitioned_changes_and_hunks,
            uncommitted_short_filenames,
        }
    }
}
