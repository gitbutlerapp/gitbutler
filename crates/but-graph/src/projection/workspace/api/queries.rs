//! Discoverable queries over [`Workspace`](crate::Workspace).
//!
//! These functions name the question being asked instead of exposing legacy
//! presentation shapes.

use anyhow::Context;

use crate::{Workspace, workspace::TargetRef};

/// Legacy query helpers kept for callers that still depend on compatibility
/// semantics.
#[cfg(feature = "legacy")]
#[path = "legacy.rs"]
pub mod legacy;

/// # Points of Interest
impl Workspace {
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
            .and_then(|target| self.graph.tip_skip_empty(target.segment_index))
            .map(|commit| commit.id)
    }
}

/// # Sets of Interest
impl Workspace {
    /// Return the configured target reference name if the workspace target was
    /// resolved to a branch during graph traversal.
    pub fn target_ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref())
    }
}

/// # Sets of Interest
impl Workspace {
    /// Return all target-reference commits that are ahead of the workspace base,
    /// which is the commits counted with
    /// [workspace::TargetRef::commits_ahead](crate::workspace::TargetRef::commits_ahead)
    ///
    /// The traversal starts at the resolved target reference and stops at the
    /// workspace lower bound or at commits already marked as belonging to the
    /// workspace. The result is ordered in graph traversal order from newer
    /// commits toward older commits.
    pub fn incoming_target_commit_ids(&self) -> anyhow::Result<Vec<gix::ObjectId>> {
        let target_ref = self
            .target_ref
            .as_ref()
            .context("incoming target commits require a workspace with a target ref")?;
        let lower_bound = self
            .lower_bound_segment_id
            .map(|segment_id| (segment_id, self.graph[segment_id].generation));

        let mut commit_ids = Vec::new();
        TargetRef::visit_upstream_commits(
            &self.graph,
            target_ref.segment_index,
            lower_bound,
            |segment| {
                commit_ids.extend(segment.commits.iter().map(|commit| commit.id));
            },
        );
        Ok(commit_ids)
    }
}
