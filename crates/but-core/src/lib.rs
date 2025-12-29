#![deny(missing_docs)]
//! The basic primitives that GitButler is built around.
//!
//! It also is a catch-all for code until it's worth putting it into its own crate, *as long as it doesn't come with more dependencies*.
//! As such, this is very much intended to **be a leaf crate**, i.e. a crate that can be used in many places for basic functionality.
//!
//! ### Data for consumption by UI
//!
//! Data-types for the user-interface are colocated in a `ui` module close to the module where the plumbing-type is located.
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
//!    - don't introduce dependencies towards legacy crates (e.g. prefixed withn "gitbutler-")
//! * **The filesystem is `Sync` but we don't have atomic operations**
//!    - Let's be very careful about changes to the filesystem, must at least be on the level of Git which means `.lock` files instead of direct writes.
//!    - If only one part of the application is supposed to change the worktree, let's protect the Application from itself by using `gitbutler::access` just like we do now.
//! * **Implement `Serialize` on utility types to facilitate transfer to the frontend**
//!     - But don't make bigger types frontend-specific. If that is needed, create a new type in the frontend-crate that uses frontend types.
//!     - `BString` has a `BStringForFrontend` counterpart.
//!     - `gix::ObjectId` has a `with = but_serde::object_id` serialization module.
//! * **Make it work, make it work right, and if time and profiler permits, make it work fast**.
//! * **If this crate gets too fat, spin modules off into their own crates**
//! * **All of the above can and should be scrutinized and is there is no hard rules.**
//!
//! ### Terminology
//!
//! * **Worktree**
//!     - A git worktree, i.e. the checkout of a tree that makes the tree accessible on disk.
//! * **Workspace**
//!     - A GitButler concept of the combination of one or more branches into one worktree. This allows
//!       multiple branches to be perceived in one worktree, by merging multiple branches together.
//! * **TreeChange**
//!     - A change to a path contained in a Git tree.
//!     - The change may have various sources, like an actual Git tree, or the worktree.
//!     - It's tuned to contain only information we are interested in, which includes if an addition is implied by an untracked file.
//! * **UnifiedDiff**
//!     - A list of patches in unified diff format, with easily accessible line number information. It isn't baked into the patch string itself.
//!

use std::{
    any::Any,
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use bstr::BString;
use gix::{
    object::tree::EntryKind, refs::FullNameRef,
    status::plumbing::index_as_worktree::ConflictIndexEntry,
};
use serde::Serialize;

/// Functions to obtain changes between various items.
pub mod diff;
/// Fundamental data types for reuse
mod diff_types;
pub use diff_types::{DiffSpec, HunkHeader, ModeFlags};

mod hunks;
pub use hunks::{HunkRange, apply_hunks};

/// Commit related utility types.
pub mod commit;

/// Utilities related to branches
pub mod branch;

/// Types for use in the user interface.
pub mod ui;

mod id;
pub use id::Id;

/// utility types
pub mod unified_diff;

/// utilities for command-invocation.
pub mod cmd;

/// Various settings
pub mod settings;
pub use settings::git::types::GitConfigSettings;

pub mod snapshot;

/// Utilities to deal with git worktrees.
pub mod worktree;

/// Utilities to create Git trees.
pub mod tree;

/// Various types
pub mod ref_metadata;
use crate::ref_metadata::ValueInfo;

/// Utilities to sync project access.
pub mod sync;

mod ext;
pub use ext::ObjectStorageExt;

mod repo_ext;
pub use repo_ext::RepositoryExt;

/// Return `true` if `ref_name` looks like the standard GitButler workspace.
///
/// Note that in the future, ideally we won't rely on the name at all, but instead
/// check for the presence of workspace ref-metadata.
///
/// TODO: no special handling by branch-name should be needed, it's all in the ref-metadata.
pub fn is_workspace_ref_name(ref_name: &FullNameRef) -> bool {
    ref_name.as_bstr() == "refs/heads/gitbutler/workspace"
        || ref_name.as_bstr() == "refs/heads/gitbutler/integration"
}

/// A utility to extra the name of the remote from a remote tracking ref with `ref_name`.
/// If it's not a remote tracking ref, or no remote in `remote_names` (like `origin`) matches,
/// `None` is returned.
pub fn extract_remote_name(
    ref_name: &gix::refs::FullNameRef,
    remote_names: &gix::remote::Names<'_>,
) -> Option<String> {
    let (category, shorthand_name) = ref_name.category_and_short_name()?;
    if !matches!(category, gix::refs::Category::RemoteBranch) {
        return None;
    }

    let longest_remote = remote_names
        .iter()
        .rfind(|reference_name| shorthand_name.starts_with(reference_name))
        .ok_or(anyhow::anyhow!(
            "Failed to find remote branch's corresponding remote"
        ))
        .ok()?;
    Some(longest_remote.to_string())
}

/// A trait to associate arbitrary metadata with any *Git reference name*.
/// Note that a single reference name can have multiple distinct pieces of metadata associated with it.
pub trait RefMetadata {
    /// An implementation-defined wrapper for all data to keep additional information that it might need
    /// to more easily store the data.
    type Handle<T>: Deref<Target = T> + DerefMut + ref_metadata::ValueInfo + AsRef<FullNameRef>;

    /// Traverse all available metadata entries and see if their names still exist in the Git ref database.
    ///
    /// If not, they are dangling, and can then be downcast to their actual type to deal with them in some way,
    /// either by [removing](Self::remove) them, or by re-associating them with an existing reference.
    fn iter(
        &self,
    ) -> impl Iterator<Item = anyhow::Result<(gix::refs::FullName, Box<dyn Any>)>> + '_;

    /// Retrieve workspace metadata for `ref_name` or create it if it wasn't present yet.
    fn workspace(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Self::Handle<ref_metadata::Workspace>>;

    /// Retrieve branch metadata for `ref_name` or create it if it wasn't present yet.
    fn branch(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Self::Handle<ref_metadata::Branch>>;

    /// Like [`branch()`](Self::branch()), but instead of possibly returning default values, return an
    /// optional branch instead.
    ///
    /// This means the returned branch data is never the default value.
    fn branch_opt(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<Self::Handle<ref_metadata::Branch>>> {
        let branch = self.branch(ref_name)?;
        Ok(if branch.is_default() {
            None
        } else {
            Some(branch)
        })
    }

    /// Like [`workspace()`](Self::workspace()), but instead of possibly returning default values, return an
    /// optional workspace instead.
    ///
    /// This means the returned workspace data is never the default value.
    fn workspace_opt(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<Self::Handle<ref_metadata::Workspace>>> {
        let ws = self.workspace(ref_name)?;
        Ok(if ws.is_default() { None } else { Some(ws) })
    }

    /// Set workspace metadata to match `value`.
    fn set_workspace(
        &mut self,
        value: &Self::Handle<ref_metadata::Workspace>,
    ) -> anyhow::Result<()>;

    /// Set branch metadata to match `value`.
    fn set_branch(&mut self, value: &Self::Handle<ref_metadata::Branch>) -> anyhow::Result<()>;

    /// Delete the metadata associated with the given `ref_name` and return `true` if it existed, or `false` otherwise.
    ///
    /// It is OK to delete something that doesn't exist.
    fn remove(&mut self, ref_name: &gix::refs::FullNameRef) -> anyhow::Result<bool>;
}

/// A decoded commit object with easy access to additional GitButler information.
#[derive(Debug, Clone)]
pub struct Commit<'repo> {
    /// The id of the commit itself.
    pub id: gix::Id<'repo>,
    /// The decoded commit for direct access.
    pub inner: gix::objs::Commit,
}

/// A decoded commit object with easy access to additional GitButler information, without repo reference.
#[derive(Debug, Clone)]
pub struct CommitOwned {
    /// The id of the commit itself.
    pub id: gix::ObjectId,
    /// The decoded commit for direct access.
    pub inner: gix::objs::Commit,
}

/// A patch in unified diff format to show how a resource changed or now looks like (in case it was newly added),
/// or how it previously looked like in case of a deletion.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject")]
pub enum UnifiedPatch {
    /// The resource was a binary and couldn't be diffed.
    Binary,
    /// The file was too large and couldn't be diffed.
    TooLarge {
        /// The size of the file on disk that made it too large.
        #[serde(rename = "sizeInBytes")]
        size_in_bytes: u64,
    },
    /// A patch that if applied to the previous state of the resource would yield the current state.
    #[serde(rename_all = "camelCase")]
    Patch {
        /// All non-overlapping hunks, including their context lines.
        hunks: Vec<unified_diff::DiffHunk>,
        /// If `true`, a binary to text filter (`textconv` in Git config) was used to obtain the `hunks` in the diff.
        /// This means hunk-based operations must be disabled.
        is_result_of_binary_to_text_conversion: bool,
        /// The total amount of lines added.
        lines_added: u32,
        /// The total amount of lines removed.
        lines_removed: u32,
    },
}

/// Either git reference or a virtual reference (i.e. a reference not visible in Git).
#[derive(Debug, Clone, PartialEq)]
pub enum Reference {
    /// A git reference or lightweight tag.
    Git(gix::refs::FullName),
    /// A reference not visible in Git, managed by GitButler.
    // TODO: ideally this isn't needed anymore in the final version as all refs are 'real'.
    Virtual(String),
}

impl std::fmt::Display for Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reference::Git(r) => {
                let s = r.to_string();
                s.strip_prefix("refs/heads/").unwrap_or_default().fmt(f)
            }
            Reference::Virtual(r) => r.fmt(f),
        }
    }
}

/// Open a repository in such a way that the object cache is set to accelerate merge operations.
///
/// As it depends on the size of the tree, the index will be loaded for that.
pub fn open_repo_for_merging(path: impl Into<PathBuf>) -> anyhow::Result<gix::Repository> {
    let mut repo = gix::open(path)?;
    let bytes = repo.compute_object_cache_size_for_tree_diffs(&***repo.index_or_empty()?);
    repo.object_cache_size_if_unset(bytes);
    Ok(repo)
}

/// An entry in the worktree that changed and thus is eligible to being committed.
///
/// It either lives (or lived) in the in `.git/index`, or in the `worktree`.
///
/// ### Note
///
/// For simplicity, copy-tracking is not representable right now, but `copy: bool` could be added
/// if needed. Copy-tracking is deactivated as well.
#[derive(Debug, Clone)]
pub struct TreeChange {
    /// The *relative* path in the worktree where the entry can be found.
    pub path: BString,
    /// The specific information about this change.
    pub status: TreeStatus,
}

/// Specifically defines a [`TreeChange`].
#[derive(Debug, Clone)]
pub enum TreeStatus {
    /// Something was added or scheduled to be added.
    Addition {
        /// The current state of what was added or will be added
        state: ChangeState,
        /// If `true`, this is a future addition from an untracked file, a file that wasn't yet added to the index (`.git/index`).
        is_untracked: bool,
    },
    /// Something was deleted.
    Deletion {
        /// The that Git stored before the deletion.
        previous_state: ChangeState,
    },
    /// A tracked entry was modified, which might mean:
    ///
    /// * the content change, i.e. a file was changed
    /// * the type changed, a file is now a symlink or something else
    /// * the executable bit changed, so a file is now executable, or isn't anymore.
    Modification {
        /// The that Git stored before the modification.
        previous_state: ChangeState,
        /// The current state, i.e. the modification itself.
        state: ChangeState,
        /// Derived information based on the mode of both states.
        flags: Option<ModeFlags>,
    },
    /// An entry was renamed from `previous_path` to its current location.
    ///
    /// Note that this may include any change already documented in [`Modification`](TreeStatus::Modification)
    Rename {
        /// The path relative to the repository at which the entry was previously located.
        previous_path: BString,
        /// The that Git stored before the modification.
        previous_state: ChangeState,
        /// The current state, i.e. the modification itself.
        state: ChangeState,
        /// Derived information based on the mode of both states.
        flags: Option<ModeFlags>,
    },
}

/// Like [`TreeStatus`], but distilled down to its variant.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TreeStatusKind {
    /// Something was added or scheduled to be added.
    Addition,
    /// Something was deleted.
    Deletion,
    /// A tracked entry was modified, which might mean:
    ///
    /// * the content change, i.e. a file was changed
    /// * the type changed, a file is now a symlink or something else
    /// * the executable bit changed, so a file is now executable, or isn't anymore.
    Modification,
    /// An entry was renamed from `previous_path` to its current location.
    ///
    /// Note that this may include any change already documented in [`Modification`](TreeStatusKind::Modification)
    Rename,
}

/// Something that fully identifies the state of a [`TreeChange`].
#[derive(Debug, Clone, Copy)]
pub struct ChangeState {
    /// The content of the committable.
    ///
    /// If [`null`](gix::ObjectId::is_null), the current state isn't known which can happen
    /// if this state is living in the worktree and has never been hashed.
    pub id: gix::ObjectId,
    /// The kind of the committable.
    pub kind: EntryKind,
}

/// The status we can't handle, which always originated in the worktree.
#[derive(Debug, Clone, Serialize)]
pub enum IgnoredWorktreeTreeChangeStatus {
    /// A conflicting entry in the index. The worktree state of the entry is unclear.
    Conflict,
    /// A change in the `.git/index` that was overruled by a change to the same path in the *worktree*.
    TreeIndex,
    /// A tree-index change was effectively undone by an index-worktree change. Thus, the version in the worktree
    /// is the same as what Git is currently tracking.
    TreeIndexWorktreeChangeIneffective,
}

/// A way to indicate that a path in the index isn't suitable for committing and needs to be dealt with.
#[derive(Clone, Serialize)]
pub struct IgnoredWorktreeChange {
    /// The worktree-relative path to the change.
    #[serde(serialize_with = "but_serde::bstring_lossy::serialize")]
    pub path: BString,
    /// The status that caused this change to be ignored.
    pub status: IgnoredWorktreeTreeChangeStatus,
}

/// The type returned by [`worktree_changes()`](diff::worktree_changes).
#[derive(Clone)]
pub struct WorktreeChanges {
    /// Changes that could be committed.
    pub changes: Vec<TreeChange>,
    /// Changes that were in the index that we can't handle. The user can see them and interact with them to clear them out before a commit can be made.
    pub ignored_changes: Vec<IgnoredWorktreeChange>,
    /// All unprocessed changes to the index.
    pub index_changes: Vec<gix::diff::index::Change>,
    /// The conflicting index entries, along with their relative path `(rela_path, [Entries(base, ours, theirs)])`.
    pub index_conflicts: Vec<(BString, Box<[Option<ConflictIndexEntry>; 3]>)>,
}
