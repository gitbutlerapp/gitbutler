/// Options for use in [super::safe_checkout_from_head()].
#[derive(Default, Debug, Clone)]
pub struct Options {
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
    /// If `true`, proceed when uncommitted worktree changes conflict with the checkout
    /// destination, writing Git-style conflict markers into the affected files.
    ///
    /// The markers become part of the uncommitted changes, without conflict entries in the index.
    /// By default such a checkout is refused so no conflict markers ever appear on disk.
    pub allow_worktree_conflicts: bool,
}

/// The successful outcome of [super::safe_checkout_from_head()] operation.
#[derive(Clone)]
pub struct Outcome {
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
