/// What to do when uncommitted changes are in the way of files that will be affected by the checkout, and that
/// don't re-apply cleanly on top of the new worktree commit.
#[derive(Default, Debug, Copy, Clone)]
pub enum UncommitedWorktreeChanges {
    /// Do not alter anything if local worktree changes conflict with the incoming one, but abort the operation instead.
    #[default]
    KeepAndAbortOnConflict,
    /// Place the files that would be altered, AND at least one conflicts when brought back, into a snapshot based
    /// on the current `HEAD`, and overwrite them.
    /// Note that uncommitted changes that aren't affected will just be left as is.
    KeepConflictingInSnapshotAndOverwrite,
}

/// Options for use in [super::safe_checkout()].
#[derive(Default, Debug, Copy, Clone)]
pub struct Options {
    /// How to deal with uncommitted changes.
    pub uncommitted_changes: UncommitedWorktreeChanges,
}

/// The successful outcome of [super::safe_checkout()] operation.
#[derive(Clone)]
pub struct Outcome {
    /// The tree of the snapshot which stores the worktree changes that have been overwritten as part of the checkout,
    /// based on the `current_head_tree_id` from which it was created.
    pub snapshot_tree: Option<gix::ObjectId>,
    /// If `new_head_id` was a commit, these are the ref-edits returned after performing the transaction.
    pub head_update: Option<Vec<gix::refs::transaction::RefEdit>>,
    /// The number of files that were deleted turn the current worktree into the desired one.
    /// Note that this only counts files, not directories.
    pub num_deleted_files: usize,
    /// The number of files that were added or modified turn the current worktree into the desired one.
    /// Note that this only counts files, not directories.
    pub num_added_or_updated_files: usize,
}

pub(crate) mod function;
mod tree;
mod utils;
