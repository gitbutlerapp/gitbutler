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
    // TODO: Add a ref-name with which to associate the snapshot commit for safekeeping, but needs UI support.
    KeepConflictingInSnapshotAndOverwrite,
}

/// Options for use in [super::safe_checkout()].
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How to deal with uncommitted changes.
    pub uncommitted_changes: UncommitedWorktreeChanges,
    /// If `true`, do not change `HEAD` to the new commit.
    ///
    /// This is typically to be avoided, but may be used if you want to change the HEAD location yourself.
    pub skip_head_update: bool,
    /// If set, use this tree instead of `HEAD^{tree}` as the merge base when
    /// resolving the worktree snapshot against the new HEAD.
    ///
    /// Set this to `HEAD^{tree}` + consumed changes (additive-only) after a
    /// commit/amend so the consumed hunks cancel in the 3-way merge and don't
    /// reappear as uncommitted changes.
    pub merge_base_override: Option<gix::ObjectId>,
    /// Allow checking out GitButler-managed conflicted commits.
    ///
    /// Most callers should keep the default refusal and surface a higher-level
    /// conflict workflow instead. Rebase materialization may opt in when it
    /// intentionally created the conflicted commit it is about to materialize.
    pub allow_conflicted_commit_checkout: bool,
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
