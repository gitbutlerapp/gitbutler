use crate::WorkspaceState;
use but_core::{DiffSpec, tree::create_tree::RejectionReason};
use serde::{Deserialize, Serialize};

pub struct CommitCreateResult {
    /// If the commit was successfully created. This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<gix::ObjectId>,
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    pub workspace: WorkspaceState,
}

pub struct MoveChangesResult {
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
    pub commit_id: gix::ObjectId,
    /// All changes requested for this commit.
    pub changes: Vec<DiffSpec>,
    /// Human-readable failure reason.
    pub error: String,
}

pub struct UncommitChangesFromCommitsResult {
    /// Workspace state after uncommitting successful sources.
    pub workspace: WorkspaceState,
    pub failures: Vec<UncommitChangesFailure>,
}

pub struct CommitRewordResult {
    pub new_commit: gix::ObjectId,
    pub workspace: WorkspaceState,
}

pub struct CommitSquashResult {
    pub new_commit: gix::ObjectId,
    pub workspace: WorkspaceState,
}

pub struct CommitMoveResult {
    pub workspace: WorkspaceState,
}

pub struct CommitCherryPickResult {
    /// The final IDs of the cherry-picked commits in parent-first order.
    pub new_commits: Vec<gix::ObjectId>,
    pub workspace: WorkspaceState,
}

pub struct CommitInsertBlankResult {
    pub new_commit: gix::ObjectId,
    pub workspace: WorkspaceState,
}

pub struct CommitDiscardResult {
    pub discarded_commits: Vec<gix::ObjectId>,
    pub workspace: WorkspaceState,
}

pub struct UncommitResult {
    pub uncommitted_ids: Vec<gix::ObjectId>,
    pub workspace: WorkspaceState,
}
