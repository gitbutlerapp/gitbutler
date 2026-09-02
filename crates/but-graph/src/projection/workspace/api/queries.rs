//! Discoverable queries over [`Workspace`](crate::Workspace).
//!
//! These functions name the question being asked instead of exposing legacy
//! presentation shapes.

use crate::{RefInfo, Workspace, segment};

/// Legacy query helpers kept for callers that still depend on compatibility
/// semantics.
#[cfg(feature = "legacy")]
#[path = "legacy.rs"]
pub mod legacy;

/// # Points of Interest
impl Workspace {
    /// Return the `commit` at the tip of the workspace, or that the tip reference
    /// was pointing to in Git.
    ///
    /// Empty virtual workspace tip segments may fan out to multiple stack
    /// branches, so the workspace segment has no unique graph path to a commit.
    /// This falls back to the peeled commit id stored in the workspace segment's
    /// [`crate::RefInfo`] and resolves that id against the final graph.
    ///
    /// Note that this commit could also be the base of the workspace,
    /// particularly if there are no commits in the workspace.
    pub fn tip_commit(&self) -> Option<&segment::Commit> {
        self.commit_graph_ref()?.commit(self.tip_commit_id?)
    }

    /// Return the stored target commit id.
    ///
    /// This is the previous target position remembered in workspace metadata.
    /// It is normally the base the workspace last integrated with, and
    /// intentionally differs from [`Self::target_ref_tip_commit_id()`], which
    /// returns the current tip of the target reference.
    pub fn stored_target_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_commit.as_ref().map(|target| target.commit_id)
    }

    /// Return the current tip commit id of the target reference if it is
    /// present in the workspace graph.
    pub fn target_ref_tip_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_ref
            .as_ref()
            .and_then(|target| target.tip_commit_id)
    }

    /// Return the commit id that currently acts as the workspace target.
    ///
    /// This follows the same precedence as operations that need a concrete
    /// target side: target ref tip, then stored target commit, then the first
    /// integrated traversal tip.
    pub fn effective_target_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_ref
            .as_ref()
            .and_then(|target| target.tip_commit_id)
            .or_else(|| self.target_commit.as_ref().map(|target| target.commit_id))
            .or(self.integrated_target_tip_commit_id)
    }
}

/// # Refs of Interest
impl Workspace {
    /// Return the configured target reference name if the workspace target was
    /// resolved to a branch during graph traversal.
    /// This is mere convenience and it should only be used for displaying the target ref.
    /// For everything else, use [`Self::target_ref`].
    pub fn target_ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref())
    }

    /// Return the local tracking branch reference information with the configured
    ///  [target reference](Self::target_ref). This is available as long as a target
    /// ref exists (i.e. `refs/remotes/origin/main`) and a local tracking ref for it
    /// was configured or inferred.
    pub fn target_local_tracking_ref_info(&self) -> Option<&RefInfo> {
        self.target_ref
            .as_ref()
            .and_then(|target_ref| target_ref.local_tracking.as_ref())
    }
}
