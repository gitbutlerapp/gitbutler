use crate::WorkspaceState;
use but_core::{DiffSpec, tree::create_tree::RejectionReason};

/// Outcome after creating a commit.
pub struct CommitCreateResult {
    /// If the commit was successfully created. This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// Any specs that failed to be committed.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    /// Workspace state after the create or amend.
    pub workspace: WorkspaceState,
}

/// Outcome after moving changes between commits.
pub struct MoveChangesResult {
    /// Workspace state after moving changes.
    pub workspace: WorkspaceState,
}

/// Outcome after rewording a commit.
pub struct CommitRewordResult {
    /// The ID of the newly created commit with the updated message.
    pub new_commit: gix::ObjectId,
    /// Workspace state after the reword.
    pub workspace: WorkspaceState,
}

/// Outcome of squashing one commit into another.
pub struct CommitSquashResult {
    /// The ID of the newly created squashed commit.
    pub new_commit: gix::ObjectId,
    /// Workspace state after the squash.
    pub workspace: WorkspaceState,
}

/// Outcome of moving a commit.
pub struct CommitMoveResult {
    /// Workspace state after the move.
    pub workspace: WorkspaceState,
}

/// Outcome after inserting a blank commit.
pub struct CommitInsertBlankResult {
    /// The ID of the newly inserted blank commit.
    pub new_commit: gix::ObjectId,
    /// Workspace state after inserting the blank commit.
    pub workspace: WorkspaceState,
}

/// Outcome of discarding a commit.
pub struct CommitDiscardResult {
    /// The ID of the commit discarded.
    pub discarded_commit: gix::ObjectId,
    /// Workspace state after discarding the commit.
    pub workspace: WorkspaceState,
}

/// Outcome of uncommitting one or more commits.
pub struct UncommitResult {
    /// The IDs of the commits that were uncommitted.
    pub uncommitted_ids: Vec<gix::ObjectId>,
    /// Workspace state after uncommitting.
    pub workspace: WorkspaceState,
}
