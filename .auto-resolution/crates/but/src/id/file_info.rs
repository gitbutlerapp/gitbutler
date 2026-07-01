use std::collections::{BTreeMap, btree_map};

use bstr::BString;
use nonempty::NonEmpty;

/// Information about committed files needed for CLI ID generation.
pub(crate) struct FileInfo {
    // TODO: It was observed in bd5151cf9 (fix(but status --files): Resolves an
    // issue where the ids shown for committed files are incorrect, 2025-12-29)
    // that sometimes, more than one TreeChange is reported for a (commit,
    // filename) pair even though it's not supposed to happen. (This is why
    // there's a NonEmpty in the definition of `changes` below.) Make sure that this
    // does not happen (possibly by tightening the types involved).
    /// Tree changes indexed by filename.
    pub(crate) changes: BTreeMap<BString, NonEmpty<but_core::TreeChange>>,
}

impl FileInfo {
    /// Extracts file information from tree changes.
    pub(crate) fn from_tree_changes(
        tree_changes: Vec<but_core::TreeChange>,
    ) -> anyhow::Result<Self> {
        let mut changes: BTreeMap<BString, NonEmpty<but_core::TreeChange>> = BTreeMap::new();

        for change in tree_changes {
            match changes.entry(change.path.clone()) {
                btree_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(NonEmpty::new(change));
                }
                btree_map::Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.get_mut().push(change);
                }
            };
        }
        Ok(Self { changes })
    }
}
