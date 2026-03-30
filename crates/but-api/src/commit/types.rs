use std::collections::BTreeMap;

use but_core::{DiffSpec, tree::create_tree::RejectionReason};

/// Outcome after creating a commit.
pub struct CommitCreateResult {
    /// If the commit was successfully created. This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// Any specs that failed to be committed.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome after moving changes between commits.
pub struct MoveChangesResult {
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome after rewording a commit.
pub struct CommitRewordResult {
    /// The ID of the newly created commit with the updated message.
    pub new_commit: gix::ObjectId,
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome of moving a commit.
pub struct CommitMoveResult {
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome after inserting a blank commit.
pub struct CommitInsertBlankResult {
    /// The ID of the newly inserted blank commit.
    pub new_commit: gix::ObjectId,
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome of discarding a commit.
pub struct CommitDiscardResult {
    /// The ID of the commit discarded.
    pub discarded_commit: gix::ObjectId,
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}
