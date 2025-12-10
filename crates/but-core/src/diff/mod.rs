use bstr::{BStr, ByteSlice};

use crate::{
    ChangeState, IgnoredWorktreeChange, ModeFlags, TreeChange, TreeStatus, TreeStatusKind,
};

pub(crate) mod tree_changes;
pub use tree_changes::{TreeChanges, tree_changes, tree_changes_with_line_stats};

mod worktree;
pub use worktree::{worktree_changes, worktree_changes_no_renames};

mod commit_details;
pub use commit_details::{CommitDetails, LineStats};

/// conversion functions for use in the UI
pub mod ui;

impl TreeStatus {
    /// Learn what kind of status this is, useful if only this information is needed.
    pub fn kind(&self) -> TreeStatusKind {
        match self {
            TreeStatus::Addition { .. } => TreeStatusKind::Addition,
            TreeStatus::Deletion { .. } => TreeStatusKind::Deletion,
            TreeStatus::Modification { .. } => TreeStatusKind::Modification,
            TreeStatus::Rename { .. } => TreeStatusKind::Rename,
        }
    }

    /// Return the state in which the change is currently. May be `None` if there is no current state after a deletion.
    pub fn state(&self) -> Option<ChangeState> {
        match self {
            TreeStatus::Addition { state, .. }
            | TreeStatus::Rename { state, .. }
            | TreeStatus::Modification { state, .. } => Some(*state),
            TreeStatus::Deletion { .. } => None,
        }
    }

    /// Return the previous state that the change originated from. May be `None` if there is no previous state, for instance after an addition.
    /// Also provide the path from which the state was possibly obtained.
    pub fn previous_state_and_path(&self) -> Option<(ChangeState, Option<&BStr>)> {
        match self {
            TreeStatus::Addition { .. } => None,
            TreeStatus::Rename {
                previous_state,
                previous_path,
                ..
            } => Some((*previous_state, Some(previous_path.as_bstr()))),
            TreeStatus::Modification { previous_state, .. }
            | TreeStatus::Deletion { previous_state, .. } => Some((*previous_state, None)),
        }
    }
}

impl TreeChange {
    /// Return the path at which this directory entry was previously located, if it was renamed.
    pub fn previous_path(&self) -> Option<&BStr> {
        match &self.status {
            TreeStatus::Addition { .. }
            | TreeStatus::Deletion { .. }
            | TreeStatus::Modification { .. } => None,
            TreeStatus::Rename { previous_path, .. } => Some(previous_path.as_ref()),
        }
    }
}

impl std::fmt::Debug for IgnoredWorktreeChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IgnoredWorktreeChange")
            .field("path", &self.path)
            .field("status", &self.status)
            .finish()
    }
}

impl ModeFlags {
    fn calculate(old: &ChangeState, new: &ChangeState) -> Option<Self> {
        Self::calculate_inner(old.kind, new.kind)
    }

    fn calculate_inner(
        old: gix::object::tree::EntryKind,
        new: gix::object::tree::EntryKind,
    ) -> Option<Self> {
        use gix::object::tree::EntryKind as E;
        Some(match (old, new) {
            (E::Blob, E::BlobExecutable) => ModeFlags::ExecutableBitAdded,
            (E::BlobExecutable, E::Blob) => ModeFlags::ExecutableBitRemoved,
            (E::Blob | E::BlobExecutable, E::Link) => ModeFlags::TypeChangeFileToLink,
            (E::Link, E::Blob | E::BlobExecutable) => ModeFlags::TypeChangeLinkToFile,
            (a, b) if a != b => ModeFlags::TypeChange,
            _ => return None,
        })
    }
}

impl ModeFlags {
    /// Returns `true` if this instance indicates a type-change.
    /// The only reason this isn't the case is if the executable bit changed.
    pub fn is_typechange(&self) -> bool {
        match self {
            ModeFlags::ExecutableBitAdded | ModeFlags::ExecutableBitRemoved => false,
            ModeFlags::TypeChangeFileToLink
            | ModeFlags::TypeChangeLinkToFile
            | ModeFlags::TypeChange => true,
        }
    }
}

#[cfg(test)]
mod tests {
    mod flags {
        use gix::objs::tree::EntryKind;

        use crate::ModeFlags;

        #[test]
        fn calculate() {
            for ((old, new), expected) in [
                ((EntryKind::Blob, EntryKind::Blob), None),
                (
                    (EntryKind::Blob, EntryKind::BlobExecutable),
                    Some(ModeFlags::ExecutableBitAdded),
                ),
                (
                    (EntryKind::BlobExecutable, EntryKind::Blob),
                    Some(ModeFlags::ExecutableBitRemoved),
                ),
                (
                    (EntryKind::BlobExecutable, EntryKind::Link),
                    Some(ModeFlags::TypeChangeFileToLink),
                ),
                (
                    (EntryKind::Blob, EntryKind::Link),
                    Some(ModeFlags::TypeChangeFileToLink),
                ),
                (
                    (EntryKind::Link, EntryKind::BlobExecutable),
                    Some(ModeFlags::TypeChangeLinkToFile),
                ),
                (
                    (EntryKind::Link, EntryKind::Blob),
                    Some(ModeFlags::TypeChangeLinkToFile),
                ),
                (
                    (EntryKind::Commit, EntryKind::Blob),
                    Some(ModeFlags::TypeChange),
                ),
                (
                    (EntryKind::Blob, EntryKind::Commit),
                    Some(ModeFlags::TypeChange),
                ),
            ] {
                assert_eq!(ModeFlags::calculate_inner(old, new), expected);
            }
        }
    }
}
