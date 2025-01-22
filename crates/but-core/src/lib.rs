#![deny(missing_docs, rust_2018_idioms)]
//! The basic primitives that GitButler is built around.
//!
//! It also is a catch-all for code until it's worth putting it into its own crate.
//!
//! ### House-~~Rules~~ Guidance
//!
//! * **Try hard to do write all the 'right' tests**
//!    - Tests should challenge the implementation, try hard to break it.
//!    - capture *all* business requirements
//!    - Try to avoid doing read-only filesystem fixtures with `tempdir`, instead use `gitbutler-testtools::readonly`.
//! * **minimal dependencies**
//!    - both for the *crate* and for *parameters* of functions as well.
//!         - i.e. try to avoid 'God' structures so the function only has access to what it needs to.
//! * **The filesystem is `Sync` but we don't have atomic operations**
//!    - Let's be very careful about changes to the filesystem, must at least be on the level of Git which means `.lock` files instead of direct writes.
//!    - If only one part of the application is supposed to change the worktree, let's protect the Application from itself by using `gitbutler::access` just like we do now.
//! * **Implement `Serialize` on utility types to facilitate transfer to the frontend**
//!     - But don't make bigger types frontend-specific. If that is needed, create a new type in the frontend-crate that uses frontend types.
//!     - `BString` has a `BStringForFrontend` counterpart.
//!     - `gix::ObjectId` has a `with = gitbutler_serde::object_id` serialization module.
//! * **Make it work, make it work right, and if time and profiler permits, make it work fast**.
//! * **All of the above can and should be scrutinized and is there is no hard rules.**
//!
//! ### Terminology
//!
//! * **Worktree**
//!     - A git worktree, i.e. the checkout of a tree that makes the tree accessible on disk.
//! * **Workspace**
//!     - A GitButler concept of the combination of one or more branches into one worktree. This allows
//!       multiple branches to be perceived in one worktree, by merging multiple branches together.
//!

use bstr::BString;
use serde::{Deserialize, Serialize};

/// Functions related to a Git worktree, i.e. the files checked out from a repository.
pub mod worktree;

/// utility types
pub mod unified_diff;

/// A patch in unified diff format to show how a resource changed or now looks like (in case it was newly added),
/// or how it previously looked like in case of a deletion.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject")]
pub enum UnifiedDiff {
    /// The resource was a binary and couldn't be diffed.
    Binary,
    /// The file was too large and couldn't be diffed.
    TooLarge {
        /// The size of the file on disk that made it too large.
        #[serde(rename = "sizeInBytes")]
        size_in_bytes: u64,
    },
    /// A patch that if applied to the previous state of the resource would yield the current state.
    Patch {
        /// All non-overlapping hunks, including their context lines.
        hunks: Vec<unified_diff::DiffHunk>,
    },
}

/// An entry in the worktree that changed and thus is eligible to being committed.
///
/// It either lives (or lived) in the in `.git/index`, or in the `worktree`.
///
/// ### Note
///
/// For simplicity, copy-tracking is not representable right now, but `copy: bool` could be added
/// if needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeChange {
    /// The *relative* path in the worktree where the entry can be found.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub path: BString,
    /// The specific information about this change.
    pub status: worktree::TreeStatus,
}

/// The status we can't handle, which always originated in the worktree.
#[derive(Debug, Clone, Serialize)]
pub enum IgnoredWorktreeTreeChangeStatus {
    /// A conflicting entry in the index. The worktree state of the entry is unclear.
    Conflict,
    /// A change in the `.git/index` that was overruled by a change to the same path in the *worktree*.
    TreeIndex,
}

/// A way to indicate that a path in the index isn't suitable for committing and needs to be dealt with.
#[derive(Debug, Clone, Serialize)]
pub struct IgnoredWorktreeChange {
    /// The worktree-relative path to the change.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    path: BString,
    /// The status that caused this change to be ignored.
    status: IgnoredWorktreeTreeChangeStatus,
}

/// Keeps simplified [`TreeChange`]s along with changes that were applied to the index that we chose to ignore.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeChanges {
    /// Changes that could be committed.
    pub changes: Vec<TreeChange>,
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    pub ignored_changes: Vec<IgnoredWorktreeChange>,
}

/// Computed using the file kinds/modes of two [`worktree::ChangeState`] instances to represent
/// the *dominant* change to display. Note that it can stack with a content change,
/// but *should not only in case of a `TypeChange*`*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum ModeFlags {
    ExecutableBitAdded,
    ExecutableBitRemoved,
    TypeChangeFileToLink,
    TypeChangeLinkToFile,
    TypeChange,
}

impl ModeFlags {
    fn calculate(old: &worktree::ChangeState, new: &worktree::ChangeState) -> Option<Self> {
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

#[cfg(test)]
mod tests {
    mod flags {
        use crate::ModeFlags;
        use gix::objs::tree::EntryKind;

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
