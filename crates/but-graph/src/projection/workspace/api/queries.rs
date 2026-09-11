//! Discoverable queries over [`Workspace`](crate::Workspace).
//!
//! These functions name the question being asked instead of exposing legacy
//! presentation shapes.

use anyhow::Context;

use crate::{SegmentIndex, Workspace, segment, workspace::TargetRef};

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
        self.tip_commit_by_segment_id(self.id)
    }

    /// Return the `commit` at the tip of `segment_id`, or that its ref was pointing
    /// to in Git.
    ///
    /// This first uses [`Graph::tip_skip_empty()`](crate::Graph::tip_skip_empty)
    /// to follow an unambiguous chain of empty segments to the first commit.
    /// If that cannot resolve a commit, it falls back to the peeled commit id
    /// stored in the segment's [`crate::RefInfo`] and resolves that id in the
    /// graph.
    ///
    /// That fallback is what makes this useful for workspace-owned virtual
    /// segments whose ref points at a commit, but whose graph edges do not form
    /// a single unambiguous path to it.
    pub fn tip_commit_by_segment_id(&self, segment_id: SegmentIndex) -> Option<&segment::Commit> {
        self.graph.tip_skip_empty(segment_id).or_else(|| {
            let commit_id = self.graph[segment_id].ref_info.as_ref()?.commit_id?;
            self.graph
                .segment_by_commit_id(commit_id)
                .ok()?
                .commit_by_id(commit_id)
        })
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
}

/// # Sets of Interest
impl Workspace {
    /// Return all target-reference commits that are ahead of the workspace base,
    /// which is the commits counted with
    /// [`field@crate::workspace::TargetRef::commits_ahead`]
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

/// # Bases of Interest
impl Workspace {
    /// Return the child-most commit any stack rests on, which is where a new
    /// independent branch belongs.
    ///
    /// Stacks rest on the target's history, so among their bases the one that
    /// descends from all others wins; unrelated bases keep stack order. Without
    /// stacks this is the target commit itself, and without a target there is no
    /// base to return.
    pub fn highest_base(&self) -> Option<gix::ObjectId> {
        let target = self.target_commit.as_ref()?;
        self.stacks
            .iter()
            .filter_map(|stack| stack.base().zip(stack.base_segment_id()))
            .reduce(|best, other| {
                if self.graph.find_merge_base(best.1, other.1) == Some(best.1) {
                    other
                } else {
                    best
                }
            })
            .map(|(base, _)| base)
            .or(Some(target.commit_id))
    }
}
