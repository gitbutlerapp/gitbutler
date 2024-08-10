use gitbutler_reference::ReferenceName;
use serde::{Deserialize, Serialize};

use crate::BranchId;

/// GitButler reference associated with a virtual branch.
/// These are not the same as regular Git references, but rather app-managed refs.
/// Represent a deployable / reviewable part of a virtual branch that can be pushed to a remote
/// and have a "Pull Request" created for it.
// TODO(kv): There is a name collision with `VirtualBranchReference` in `gitbutler-branch-actions/src/branch.rs` where this name means something entirerly different.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct BranchReference {
    /// Branch id of the virtual branch this reference belongs to
    /// Multiple references may belong to the same virtual branch, representing separate deployable / reviewable parts of the vbranch.
    pub branch_id: BranchId,
    /// Fully qualified reference name.
    /// The reference must be a remote reference.
    pub upstream: ReferenceName,
    /// The commit this reference points to. The commit must be part of the virtual branch.
    #[serde(with = "gitbutler_serde::oid")]
    pub commit_id: git2::Oid,
    /// The change id associated with the commit, if any.
    pub change_id: Option<String>,
}
