use anyhow::Context as _;
use but_core::ref_metadata;

use crate::{Graph, SegmentIndex, projection::Stack};

mod api;
mod init;
pub(crate) use init::Downgrade;

/// A workspace reference is a list of [Stacks](Stack), with a reference to the underlying [`Graph`].
#[derive(Clone)]
pub struct Workspace {
    /// The underlying graph for providing simplified access to data.
    pub graph: Graph,
    /// An ID which uniquely identifies the [graph segment](crate::Segment) that represents the tip of the workspace.
    pub id: SegmentIndex,
    /// Specify what kind of workspace this is.
    pub kind: WorkspaceKind,
    /// One or more stacks that live in the workspace, in order of parents of the workspace commit if there are more than one.
    pub stacks: Vec<Stack>,
    /// The bound can be imagined as the commit from which all other commits in the workspace originate.
    /// It can also be imagined to be the delimiter at the bottom beyond which nothing belongs to the workspace,
    /// as antagonist to the first commit in tip of the segment with `id`, serving as first commit that is
    /// inside the workspace.
    ///
    /// As such, it's always the longest path to the first shared commit with the target among
    /// all of our stacks, or it is the first commit that is shared among all of our stacks in absence of a target.
    /// One can also think of it as the starting point from which all workspace commits can be reached when
    /// following all incoming connections and stopping at the tip of the workspace.
    ///
    /// It is `None` there is only a single stack and no target, so nothing was integrated.
    pub lower_bound: Option<gix::ObjectId>,
    /// If `lower_bound` is set, this is the segment owning the commit.
    pub lower_bound_segment_id: Option<SegmentIndex>,
    /// The target, as identified by a remote tracking branch, to integrate workspace stacks into.
    ///
    /// If `None`, and if `target_commit` is `None`, this is a local workspace that doesn't know when
    /// possibly pushed branches are considered integrated. This happens when there is a local branch
    /// checked out without a remote tracking branch.
    pub target_ref: Option<TargetRef>,
    /// A commit reachable by [`Self::target_ref`] which we chose to keep as base. That way we can extend the workspace
    /// past its computed lower bound.
    ///
    /// Indeed, it's valid to not set the reference, and to only set the commit which should act as an integration base.
    pub target_commit: Option<TargetCommit>,
    /// The segment index of the extra target as provided for traversal,
    /// useful for AdHoc workspaces, but generally applicable to all workspaces to keep the lower bound lower than it
    /// otherwise would be.
    // TODO: could extra-target and target_commit be one and the same? They kind of are, check usages.
    //       Probably better to keep the `target_commit`.
    pub extra_target: Option<SegmentIndex>,
    /// Read-only workspace metadata with additional information, or `None` if nothing was present.
    /// If this is `Some()` the `kind` is always [`WorkspaceKind::Managed`]
    pub metadata: Option<ref_metadata::Workspace>,
}

/// A copy of all workspace state, to pass it around internally.
pub(crate) struct WorkspaceState {
    pub id: SegmentIndex,
    pub kind: WorkspaceKind,
    pub stacks: Vec<Stack>,
    pub lower_bound: Option<gix::ObjectId>,
    pub lower_bound_segment_id: Option<SegmentIndex>,
    pub target_ref: Option<TargetRef>,
    pub target_commit: Option<TargetCommit>,
    pub extra_target: Option<SegmentIndex>,
    pub metadata: Option<ref_metadata::Workspace>,
}

impl Workspace {
    fn from_state(
        graph: Graph,
        WorkspaceState {
            id,
            kind,
            stacks,
            lower_bound,
            lower_bound_segment_id,
            target_ref,
            target_commit,
            extra_target,
            metadata,
        }: WorkspaceState,
    ) -> Self {
        Workspace {
            graph,
            id,
            kind,
            stacks,
            lower_bound,
            lower_bound_segment_id,
            target_ref,
            target_commit,
            extra_target,
            metadata,
        }
    }
}

/// A classifier for the workspace.
#[derive(Debug, Clone)]
pub enum WorkspaceKind {
    /// The `HEAD` is pointing to a dedicated workspace reference, like `refs/heads/gitbutler/workspace`.
    /// This also means that we have a workspace commit that `ref_name` points to directly, which is also owned
    /// exclusively by the underlying segment.
    Managed {
        /// The name of the reference pointing to the workspace commit, along with workspace info. Useful for deriving the workspace name.
        ref_info: crate::RefInfo,
    },
    /// Information for when a workspace reference was *possibly* advanced by hand and does not point to a
    /// managed workspace commit (anymore).
    /// That workspace commit, may be reachable by following the first parent from the workspace reference.
    ///
    /// Note that the stacks that follow *will* be in unusable if the workspace commit is in a segment below,
    /// but typically is usable if there is just a single real stack, or any amount of virtual stacks below
    /// (i.e. those that have no commits and are just marked by references).
    ManagedMissingWorkspaceCommit {
        /// The name of the reference pointing to the workspace commit. Useful for deriving the workspace name.
        ref_info: crate::RefInfo,
    },
    /// A segment is checked out directly.
    ///
    /// It can be inside or outside a workspace.
    /// If the respective segment is [not named](Workspace::ref_name), this means the `HEAD` id detached.
    /// The commit that the working tree is at is always implied to be the first commit of the [`crate::projection::StackSegment`]
    /// at [`Workspace::id`].
    AdHoc,
}

impl WorkspaceKind {
    /// Return `true` if this workspace has a managed reference, meaning we control certain aspects of it
    /// by means of workspace metadata that is associated with that ref.
    /// If `false`, we are more conservative and may not support all features.
    pub fn has_managed_ref(&self) -> bool {
        matches!(
            self,
            WorkspaceKind::Managed { .. } | WorkspaceKind::ManagedMissingWorkspaceCommit { .. }
        )
    }

    /// Return `true` if we have a workspace commit, a commit that merges all stacks together.
    /// Implies `has_managed_ref() == true`.
    pub fn has_managed_commit(&self) -> bool {
        matches!(self, WorkspaceKind::Managed { .. })
    }
}

impl WorkspaceKind {
    fn managed(ref_info: &Option<crate::RefInfo>) -> anyhow::Result<Self> {
        let ref_info = ref_info
            .clone()
            .context("BUG: managed workspaces must always be on a named segment")?;
        Ok(WorkspaceKind::Managed { ref_info })
    }
}

/// Information about the target reference.
#[derive(Debug, Clone)]
pub struct TargetRef {
    /// The name of the target branch, i.e. the branch that all [Stacks](Stack) want to get merged into.
    /// Typically, this is `refs/remotes/origin/main`.
    pub ref_name: gix::refs::FullName,
    /// The index to the respective segment in the graph, it's the segment with [`Self::ref_name`] as name.
    pub segment_index: SegmentIndex,
    /// The amount of commits that aren't included in any segment in the workspace, they are in its future.
    pub commits_ahead: usize,
}

/// Information about the target commit.
#[derive(Debug, Clone)]
pub struct TargetCommit {
    /// The hash of the commit that was once included in the [target ref](TargetRef), and that we remember to expand
    /// the reach of the workspace.
    pub commit_id: gix::ObjectId,
    /// The index to the respective segment in the graph for which [`Self::commit_id`] is the first commit.
    pub segment_index: SegmentIndex,
}

impl TargetCommit {
    /// Find `target_commit_id` in the `graph` and store its segment in this instance, or return `None` if not found.
    fn from_commit(target_commit_id: gix::ObjectId, graph: &Graph) -> Option<Self> {
        graph.node_weights().find_map(|s| {
            s.commits.first().and_then(|c| {
                (c.id == target_commit_id).then_some(TargetCommit {
                    commit_id: target_commit_id,
                    segment_index: s.id,
                })
            })
        })
    }
}

impl TargetRef {
    /// Return `None` if `ref_name` wasn't found as segment in `graph`.
    /// This can happen if a reference is configured, but not actually present as reference.
    /// Note that `commits_ahead` isn't set yet, see [`Self::compute_and_set_commits_ahead()`].
    fn from_ref_name_without_commits_ahead(ref_name: &gix::refs::FullName, graph: &Graph) -> Option<Self> {
        let target_segment_sidx = graph.inner.node_indices().find_map(|n| {
            let s = &graph[n];
            (s.ref_name() == Some(ref_name.as_ref())).then_some(s.id)
        })?;
        Some(TargetRef {
            ref_name: ref_name.to_owned(),
            segment_index: target_segment_sidx,
            commits_ahead: 0,
        })
    }

    fn compute_and_set_commits_ahead(&mut self, graph: &Graph, lower_bound_segment: Option<SegmentIndex>) {
        let lower_bound = lower_bound_segment.map(|sidx| (sidx, graph[sidx].generation));
        self.commits_ahead = 0;
        Self::visit_upstream_commits(graph, self.segment_index, lower_bound, |s| {
            self.commits_ahead += s.commits.len();
        })
    }
}

fn find_segment_owner_indexes_by_refname(
    stacks: &[Stack],
    ref_name: &gix::refs::FullNameRef,
) -> Option<(usize, usize)> {
    stacks.iter().enumerate().find_map(|(stack_idx, stack)| {
        stack.segments.iter().enumerate().find_map(|(seg_idx, seg)| {
            seg.ref_name()
                .is_some_and(|rn| rn == ref_name)
                .then_some((stack_idx, seg_idx))
        })
    })
}
