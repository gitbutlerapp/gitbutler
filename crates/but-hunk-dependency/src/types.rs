use but_workspace::StackId;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// A hash over the universal diff of a hunk.
// TODO: using the hash directly like we do can collide, would have to use actual Hunk to prevent this issue.
pub type HunkHash = u64;

/// A way to represent all hunk dependencies that would make it possible to know what can be applied, and were.
///
/// Note that the [`errors`](Self::errors) field may contain information about specific failures, while other paths
/// may have succeeded computing.
#[derive(Debug, Clone)]
pub struct Dependencies {
    /// A map from diffs to branch and commit dependencies.
    // TODO: could this be a specific type? Is the mapping truly required?
    //       Is this because `commit_dependent_diffs` use `HunkHash`?
    pub diffs: HashMap<HunkHash, Vec<HunkLock>>,
    /// A map from stack id to commit dependencies.
    /// Commit dependencies map commit id to commits it depends on.
    pub commit_dependencies: HashMap<StackId, HashMap<gix::ObjectId, HashSet<gix::ObjectId>>>,
    /// A map from stack id to inverse commit dependencies.
    /// Inverse commit dependencies map commit id to commits that depend on it.
    pub inverse_commit_dependencies:
        HashMap<StackId, HashMap<gix::ObjectId, HashSet<gix::ObjectId>>>,
    /// A map from stack id to dependent commit dependent diffs.
    /// Commit dependent diffs map commit id to diffs that depend on it.
    // TODO: could this be a specific type so no mapping is required?
    pub commit_dependent_diffs: HashMap<StackId, HashMap<gix::ObjectId, HashSet<HunkHash>>>,
    /// Errors that occurred during the calculation that should be presented in some way.
    // TODO: Does the UI really use whatever partial result that there may be? Should this be a real error?
    pub errors: Vec<CalculationError>,
}

/// A commit that owns this lock, along with the stack that owns it.
/// A hunk is locked when it depends on changes in commits that are in your workspace. A hunk can
/// be locked to more than one branch if it overlaps with more than one committed hunk.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    /// The ID of the stack that contains [`commit_id`](Self::commit_id).
    pub stack_id: StackId,
    /// The commit the hunk applies to.
    pub commit_id: gix::ObjectId,
}

/// An error that occurred during the calculation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculationError {
    /// A message describing the issue.
    pub error_message: String,
    /// The stack where the calculation failed.
    pub stack_id: StackId,
    /// The commit where the calculation failed.
    #[serde(serialize_with = "gitbutler_serde::object_id::serialize")]
    pub commit_id: gix::ObjectId,
    /// The path whose calculation failed.
    pub path: PathBuf,
}
