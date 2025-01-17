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

/// Functions related to a Git worktree, i.e. the files checked out from a repository.
pub mod worktree;

///
pub mod unified_diff;

/// A patch in unified diff format to show how a resource changed or now looks like (in case it was newly added),
/// or how it previously looked like in case of a deletion.
pub enum UnifiedDiff {
    /// The resource was a binary and couldn't be diffed.
    Binary,
    /// The file was too large and couldn't be diffed.
    TooLarge {
        /// The size of the file on disk that made it too large.
        size_in_bytes: u64,
    },
    /// A patch that if applied to the previous state of the resource would yield the current state.
    Patch {
        /// All non-overlapping hunks, including their context lines.
        hunks: Vec<unified_diff::DiffHunk>,
    },
}

/// The patch that turns the previous version of a resource into the current one.
pub struct BlobDiff {
    /// The worktree-relative path at which the diffed blob lives, in the working tree and/or in the repository.
    pub path: BString,
    /// All patches along with their context lines, or `None` if the patch could not be created as the content
    pub hunks: Option<Vec<()>>,
}

/// An entry in the worktree that changed and thus is eligible to being committed.
///
/// It either lives (or lived) in the in `.git/index`, or in the `worktree`.
///
/// ### Note
///
/// For simplicity, copy-tracking is not representable right now, but `copy: bool` could be added
/// if needed.
#[derive(Debug, Clone)]
pub struct TreeChange {
    /// The *relative* path in the worktree where the entry can be found.
    pub path: BString,
    /// The specific information about this change.
    pub status: worktree::Status,
}
