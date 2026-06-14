use but_core::ref_metadata;

use super::Stack;
use usize;

pub(super) mod api;
mod debug;

/// A projected workspace: the display [stacks](Stack) plus the carried commit graph and branch
/// records the rebase derives its [`BranchGraph`](crate::BranchGraph) from.
#[derive(Clone)]
pub struct Workspace {
    /// The commit-level graph the traversal built: one node per commit, edges child → parent.
    /// Carried so merge-base and reachability queries read it without the record graph.
    /// `None` for default/unborn workspaces that have no commits.
    pub(crate) commit_graph: Option<crate::commit_graph::CommitGraph>,
    /// Project-wide metadata (target ref, target commit, push remote) used to resolve the
    /// workspace and to re-run the traversal via
    /// [`Self::redo_traversal_into_workspace_with_overlay`].
    pub project_meta: ref_metadata::ProjectMeta,
    /// The options the traversal ran with, kept so the workspace can regenerate itself.
    /// Public so callers can tweak them (e.g. clear `extra_target_commit_id`) before a redo.
    pub options: crate::init::Options,
    /// The entrypoint segment's ref name, kept so redo-traversal can re-resolve the entrypoint
    /// when an overlay doesn't supply one.
    pub(crate) entrypoint_ref: Option<gix::refs::FullName>,
    /// All non-URL remote names retrieved during the traversal, used to extract remote names from
    /// remote tracking refs. Carried from the graph so consumers need not navigate it.
    pub(crate) symbolic_remote_names: Vec<String>,
    /// The traversal's branch records — the flat adjacency list carrying the full topology, for
    /// consumers that need more than the projected stacks (the rebase step graph, assembled via
    /// [`BranchGraph`](crate::BranchGraph)). Only the direct projection fills this.
    pub(crate) branches: Option<Vec<crate::branch_graph::Branch>>,
    /// An ID which uniquely identifies the graph segment that represents the tip of the workspace.
    pub id: usize,
    /// The resolved tip commit of the workspace segment (skip-empty, ref-info fallback), or `None`
    /// if it does not point to a commit (e.g. unborn). Computed at build time so consumers need not
    /// navigate the graph for the workspace tip.
    pub tip_commit_id: Option<gix::ObjectId>,
    /// The ref-info of the workspace segment (its name, peeled commit and worktree), resolved at
    /// build time. This is what [`Self::ref_name`]/[`Self::ref_info`] return without navigating the
    /// graph; for managed workspaces it matches the ref-info in [`Self::kind`].
    pub ref_info: Option<crate::RefInfo>,
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
    /// The ref name of the segment owning [`lower_bound`](Self::lower_bound), if it has one.
    /// Resolved at build time so the rebase editor can select the common base by ref without
    /// navigating the graph.
    pub lower_bound_ref_name: Option<gix::refs::FullName>,
    /// The target, as identified by a remote tracking branch, to integrate workspace stacks into.
    ///
    /// If `None`, and if `target_commit` is `None`, this is a local workspace that doesn't know when
    /// possibly pushed branches are considered integrated. This happens when there is a local branch
    /// checked out without a remote tracking branch.
    pub target_ref: Option<TargetRef>,
    /// A commit *typically* reachable by [`Self::target_ref`] which we chose to keep as base. That way we can extend the workspace
    /// past its computed lower bound.
    ///
    /// Indeed, it's valid to not set the reference, and to only set the commit which should act as an integration base.
    /// This can be done by direct user override, who simply wants to cut off history at a certain movable point in time.
    ///
    /// It is also valid to have this field point to the same Segment as [Self::target_ref]. Both have different purposes,
    /// semantically.
    pub target_commit: Option<TargetCommit>,
    /// The tip commit of the first integrated traversal tip, resolved at build time. It is the
    /// fallback target side used by [`Self::effective_target_commit_id`] when neither
    /// [`Self::target_ref`] nor [`Self::target_commit`] is set.
    pub integrated_target_tip_commit_id: Option<gix::ObjectId>,
    /// If the workspace reference was advanced past its managed workspace commit, this carries that
    /// commit (found in the ancestry) and the commits sitting on top of it. The projection resolves
    /// it at build time so consumers need not navigate the graph. `None` for managed workspaces and
    /// when no managed commit is in the ancestry.
    pub ancestor_workspace_commit: Option<AncestorWorkspaceCommit>,
    /// The disambiguated name and resolved tip of every named segment the traversal saw, resolved
    /// at build time. Lets consumers answer "is this ref a primary segment, and where does it point"
    /// (as `segment_by_ref_name` + `tip_skip_empty` did) without navigating the graph.
    pub named_segments: Vec<(gix::refs::FullName, gix::ObjectId)>,
    /// Every ref name (segment names and refs sitting on commits) to its resolved commit, as
    /// `segment_and_commit_by_ref_name` resolved them. Lets consumers resolve any ref - including a
    /// secondary ref the segment isn't named after - to its commit without the graph.
    pub ref_tips: Vec<(gix::refs::FullName, gix::ObjectId)>,
    /// `true` if the traversal stopped early at its hard limit, so the workspace may be incomplete.
    /// Carried from the graph so consumers need not hold it to know.
    pub hard_limit_hit: bool,
    /// `true` if the repository has more than one worktree, used only to render worktree-ownership
    /// markers in debug output. Carried from the graph.
    pub has_multiple_worktrees: bool,
    /// The commit at the traversal entrypoint - i.e. what `HEAD` resolved to - or `None` if unborn.
    /// Resolved at build time so consumers need not navigate the graph for the checked-out commit.
    pub entrypoint_commit_id: Option<gix::ObjectId>,
    /// Read-only workspace metadata with additional information, or `None` if nothing was present.
    /// If this is `Some()` the `kind` is always [`WorkspaceKind::Managed`]
    ///
    /// # WARNING
    ///
    /// Do not use this data to understand the workspace. It's unreconciled metadata which may
    /// have nothing to do with the actual workspace.
    /// To see that, look at [Self::stacks].
    pub metadata: Option<ref_metadata::Workspace>,
}

impl Workspace {
    /// The traversal's branch records (the flat adjacency list), if this workspace was built by the
    /// direct projection. The rebase reads these via [`BranchGraph`](crate::BranchGraph).
    pub fn branches(&self) -> Option<&[crate::branch_graph::Branch]> {
        self.branches.as_deref()
    }

    /// An empty ad-hoc workspace around the given `stacks`, for downstream tests that don't
    /// involve a repository.
    #[doc(hidden)]
    pub fn for_testing(stacks: Vec<Stack>) -> Self {
        Workspace {
            commit_graph: None,
            project_meta: Default::default(),
            options: Default::default(),
            entrypoint_ref: None,
            symbolic_remote_names: Vec::new(),
            branches: None,
            id: Default::default(),
            tip_commit_id: None,
            ref_info: None,
            kind: WorkspaceKind::AdHoc,
            stacks,
            lower_bound: None,
            lower_bound_ref_name: None,
            target_ref: None,
            target_commit: None,
            integrated_target_tip_commit_id: None,
            ancestor_workspace_commit: None,
            named_segments: Vec::new(),
            ref_tips: Vec::new(),
            hard_limit_hit: false,
            has_multiple_worktrees: false,
            entrypoint_commit_id: None,
            metadata: None,
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
    /// The commit that the working tree is at is always implied to be the first commit of the [`crate::workspace::StackSegment`]
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

/// Information about the target reference, which marks a portion in the commit-graph
/// that the workspace wants to integrate with.
#[derive(Debug, Clone)]
pub struct TargetRef {
    /// The name of the target branch, i.e. the branch that all [Stacks](Stack) want to get merged into.
    /// Typically, this is `refs/remotes/origin/main`.
    pub ref_name: gix::refs::FullName,
    /// The current tip commit of the target reference, resolved when the workspace was built.
    /// `None` if the target ref did not resolve to a commit in the graph.
    pub tip_commit_id: Option<gix::ObjectId>,
    /// The local tracking branch of the target (its sibling in the graph), if one was configured
    /// or inferred. Its `commit_id` is the resolved tip. Checkout fallbacks and target-membership
    /// checks resolve through it.
    pub local_tracking: Option<crate::RefInfo>,
    /// The amount of *all* commits that aren't included in any segment in the workspace, they are in its future.
    pub commits_ahead: usize,
}

/// Information about the target commit, which marks a portion in the commit-graph
/// that the workspace wants to integrate with.
///
/// It's an unnamed point of interest which may
/// be set by any means. Typically, it's set by using a stored value, which makes it
/// a point in time at which we have seen the [`TargetRef`].
#[derive(Debug, Clone)]
pub struct TargetCommit {
    /// The hash of the commit that was once included in the [target ref](TargetRef), and that we remember to expand
    /// the reach of the workspace.
    pub commit_id: gix::ObjectId,
}

/// A managed workspace commit found in the ancestry of an advanced workspace reference, along with
/// the commits that sit on top of it (between the reference and the managed commit, in walk order).
#[derive(Debug, Clone)]
pub struct AncestorWorkspaceCommit {
    /// The id of the managed workspace commit found in the ancestry.
    pub managed_commit_id: gix::ObjectId,
    /// The commits between the workspace reference and the managed workspace commit.
    pub commits_outside: Vec<crate::Commit>,
}

fn find_segment_owner_indexes_by_refname(
    stacks: &[Stack],
    ref_name: &gix::refs::FullNameRef,
) -> Option<(usize, usize)> {
    stacks.iter().enumerate().find_map(|(stack_idx, stack)| {
        stack
            .segments
            .iter()
            .enumerate()
            .find_map(|(seg_idx, seg)| {
                seg.ref_name()
                    .is_some_and(|rn| rn == ref_name)
                    .then_some((stack_idx, seg_idx))
            })
    })
}
