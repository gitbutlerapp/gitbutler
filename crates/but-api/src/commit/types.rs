use crate::WorkspaceState;
use but_core::{DiffSpec, tree::create_tree::RejectionReason};
use serde::{Deserialize, Serialize};

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

/// A source entry for uncommitting changes from a commit.
///
/// Multiple entries may target the same commit; the backend groups them by
/// commit id before removing the changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UncommitChangesSource {
    /// The commit to remove `changes` from.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub commit_id: gix::ObjectId,
    /// The changes to remove from the commit.
    pub changes: Vec<DiffSpec>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UncommitChangesSource);

/// A grouped source that could not be uncommitted.
pub struct UncommitChangesFailure {
    /// The commit whose changes failed to uncommit.
    pub commit_id: gix::ObjectId,
    /// All changes requested for this commit.
    pub changes: Vec<DiffSpec>,
    /// Human-readable failure reason.
    pub error: String,
}

/// Outcome after uncommitting changes from multiple commits.
pub struct UncommitChangesFromCommitsResult {
    /// Workspace state after uncommitting successful sources.
    pub workspace: WorkspaceState,
    /// Sources that could not be uncommitted.
    pub failures: Vec<UncommitChangesFailure>,
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

/// Outcome of cherry-picking commits.
pub struct CommitCherryPickResult {
    /// The final IDs of the cherry-picked commits in parent-first order.
    pub new_commits: Vec<gix::ObjectId>,
    /// Workspace state after the cherry-pick.
    pub workspace: WorkspaceState,
}

/// Outcome after inserting a blank commit.
pub struct CommitInsertBlankResult {
    /// The ID of the newly inserted blank commit.
    pub new_commit: gix::ObjectId,
    /// Workspace state after inserting the blank commit.
    pub workspace: WorkspaceState,
}

/// Outcome of discarding one or more commits.
pub struct CommitDiscardResult {
    /// The IDs of the commits discarded.
    pub discarded_commits: Vec<gix::ObjectId>,
    /// Workspace state after discarding the commits.
    pub workspace: WorkspaceState,
}

/// Outcome of uncommitting one or more commits.
pub struct UncommitResult {
    /// The IDs of the commits that were uncommitted.
    pub uncommitted_ids: Vec<gix::ObjectId>,
    /// Workspace state after uncommitting.
    pub workspace: WorkspaceState,
}
