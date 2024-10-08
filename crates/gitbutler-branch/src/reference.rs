use gitbutler_reference::ReferenceName;
use serde::{Deserialize, Serialize};

use gitbutler_stack::StackId;

/// GitButler reference associated with a change (commit) on a virtual branch.
/// These are not the same as regular Git references, but rather app-managed refs.
/// Represent a deployable / reviewable part of a virtual branch that can be pushed to a remote
/// and have a "Pull Request" created for it.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeReference {
    /// Branch id of the virtual branch this reference belongs to
    /// Multiple references may belong to the same virtual branch, representing separate deployable / reviewable parts of the vbranch.
    pub branch_id: StackId,
    /// Fully qualified reference name.
    /// The reference must be a remote reference.
    pub name: ReferenceName,
    /// The change id this reference points to.
    pub change_id: String,
}
