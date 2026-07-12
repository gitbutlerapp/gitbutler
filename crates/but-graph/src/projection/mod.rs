//! The workspace projection: GitButler's view of the world, derived from the
//! [`CommitGraph`](crate::CommitGraph), its stored ref layout and the build context
//! alone — cheap to (re)compute, and carrying the substrate it was derived from.
//!
//! These types simplify aggressively for display and interaction; they are not for
//! direct end-user consumption, but for the operations and UI layers above.

/// Types related to the stack representation for graphs.
///
/// These types deliberately simplify — detail is dropped, but a link back to the graph is kept.
mod stack;
pub use stack::{Stack, StackCommit, StackCommitDebugFlags, StackCommitFlags, StackSegment};

pub(crate) mod api;
mod collect;
mod frame;
mod init;
pub use api::StackTip;
pub(crate) use init::project_workspace;

#[cfg(feature = "legacy")]
pub use api::HeadStatus;

use anyhow::Context as _;
use but_core::ref_metadata;

/// utilities for workspace-related commits.
pub mod commit {
    use bstr::{BStr, ByteSlice};

    const GITBUTLER_INTEGRATION_COMMIT_TITLE: &str = "GitButler Integration Commit";
    const GITBUTLER_WORKSPACE_COMMIT_TITLE: &str = "GitButler Workspace Commit";

    /// Return `true` if this `commit_message` indicates a workspace commit managed by GitButler.
    /// If `false`, this is the tip of the stack itself which will be put underneath a *managed* workspace commit
    /// once another branch is added to the workspace.
    pub fn is_managed_workspace_by_message(commit_message: &BStr) -> bool {
        let message = gix::objs::commit::MessageRef::from_bytes(commit_message);
        let title = message.title.trim().as_bstr();
        title == GITBUTLER_INTEGRATION_COMMIT_TITLE || title == GITBUTLER_WORKSPACE_COMMIT_TITLE
    }
}

/// The workspace ref and its metadata, as the builder captured them.
#[derive(Debug, Clone)]
pub(crate) struct WorkspaceMeta {
    pub ref_name: gix::refs::FullName,
    pub metadata: but_core::ref_metadata::Workspace,
}

/// The build-time context carried alongside the graph store: everything projection and
/// re-traversal need that isn't commits, segments, or their connections. Produced by the
/// builders, owned by the [`Workspace`].
#[derive(Default, Clone, Debug)]
pub(crate) struct GraphContext {
    /// The options used to create the graph, which allow it to be regenerated ([`Workspace::redo`])
    /// and to simulate changes by injecting would-be information.
    pub options: crate::walk::Options,
    /// Project-wide metadata used for target ref, target commit, and push remote resolution.
    pub project_meta: but_core::ref_metadata::ProjectMeta,
    /// All remote names that aren't URLs, as the builder derived them from the repository.
    ///
    /// They are useful to extract remote names from remote tracking refs like
    /// `refs/remotes/origin/master`, which may have slashes in them.
    pub symbolic_remote_names: Vec<String>,
    /// The local → remote tracking-branch relationships the builder derived from the repository
    /// (git-configured bindings plus name-deduction against the workspace's symbolic remotes).
    pub remote_tracking: std::collections::HashMap<gix::refs::FullName, gix::refs::FullName>,
    /// The validated snapshot of [`branch_stack_order`](but_core::RefMetadata::branch_stack_order)
    /// for the entrypoint ref: the tip-to-base branch chains GitButler created in
    /// ad-hoc/single-branch mode, filtered at build time to refs that still exist.
    pub ad_hoc_branch_stack_orders: Vec<Vec<gix::refs::FullName>>,
    /// Name-keyed enrichment the builder captured for every named branch: its
    /// metadata sideband and worktree ownership, for the projection to attach.
    pub branch_details: std::collections::HashMap<gix::refs::FullName, BranchDetails>,
    /// The workspace ref and its metadata, when the graph contains one — the
    /// builder saw it as a segment with workspace metadata.
    pub workspace_meta: Option<WorkspaceMeta>,
    /// The commit the entrypoint UNAMBIGUOUSLY points to, resolved through its
    /// empty chain at build time — `None` when the chain forks or dead-ends.
    pub entry_resolved_commit: Option<gix::ObjectId>,
}

/// The per-branch enrichment the projection attaches by name.
#[derive(Debug, Clone, Default)]
pub(crate) struct BranchDetails {
    /// The branch's metadata sideband, if any.
    pub metadata: Option<but_core::ref_metadata::Branch>,
    /// The worktree this branch is checked out in, if any.
    pub worktree: Option<crate::Worktree>,
    /// Where this branch's LINKED remote tracking branch rests — the walk start
    /// for remote reachability. Deliberately absent for ambiguous remotes the
    /// builder chose not to link.
    pub remote_walk_tip: Option<gix::ObjectId>,
}

/// The traversal entrypoint as captured by the projection: where `HEAD` pointed when the
/// graph was built, and how it relates to the workspace tip.
#[derive(Clone, Debug)]
pub(crate) struct EntrypointInfo {
    /// The commit the entrypoint sits on; `None` for unborn refs.
    pub commit_id: Option<gix::ObjectId>,
    /// The name of the entrypoint segment, if any — the re-traversal seed ref.
    pub ref_name: Option<gix::refs::FullName>,
    /// Whether the entrypoint segment is the workspace tip segment itself.
    pub is_ws_tip: bool,
}

/// A workspace reference is a list of [Stacks](Stack) projected over a [`CommitGraph`](crate::CommitGraph).
#[derive(Clone)]
pub struct Workspace {
    /// Specify what kind of workspace this is.
    pub kind: WorkspaceKind,
    /// One or more stacks that live in the workspace, in order of parents of the workspace commit if there are more than one.
    pub stacks: Vec<Stack>,
    /// The commit all workspace commits build on — the boundary at the bottom beyond which
    /// nothing belongs to the workspace. It is the longest path to the first commit shared
    /// with the target across all stacks, or, without a target, the first commit all stacks
    /// share. `None` when there is a single stack and no target — nothing was integrated.
    pub lower_bound: Option<gix::ObjectId>,
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
    /// Read-only workspace metadata with additional information, or `None` if nothing was present.
    /// If this is `Some()` the `kind` is always [`WorkspaceKind::Managed`]
    ///
    /// # WARNING
    ///
    /// Do not use this data to understand the workspace. It's unreconciled metadata which may
    /// have nothing to do with the actual workspace.
    /// To see that, look at [Self::stacks].
    pub metadata: Option<ref_metadata::Workspace>,

    // ── The carried substrate. ──
    /// The commit graph the workspace was projected from — the commit-addressed
    /// substrate all queries answer from.
    pub(crate) commit_graph: crate::CommitGraph,
    /// The build-time context: options, project metadata, and repository-derived
    /// remote relationships.
    pub(crate) ctx: GraphContext,

    // ── Facts captured at projection time, before the segment view is dropped. ──
    /// The ref info of the workspace tip segment.
    pub(crate) tip_ref_info: Option<crate::RefInfo>,
    /// The commit the workspace tip segment rests on.
    pub(crate) tip_commit_id: Option<gix::ObjectId>,
    /// The name of the segment owning the workspace's lower bound.
    pub(crate) lower_bound_ref_name: Option<gix::refs::FullName>,
    /// The traversal entrypoint as the projection saw it. `None` only for hand-assembled
    /// graphs whose entrypoint was never set.
    pub(crate) entrypoint: Option<EntrypointInfo>,
    /// The commit the target ref's segment rests on.
    pub(crate) target_ref_commit_id: Option<gix::ObjectId>,
    /// The commit acting as the workspace target — target ref tip, then stored target
    /// commit, then the first integrated traversal tip.
    pub(crate) effective_target_commit_id: Option<gix::ObjectId>,
    /// The commits in the ancestry of the workspace tip segment — excluding that segment's
    /// own commits — in traversal order down to the lower bound.
    pub(crate) commits_below_tip: Vec<gix::ObjectId>,
    /// The target-side commits ahead of the workspace base, from the target tip downward.
    /// `None` without a resolved target ref.
    pub(crate) incoming_target_commit_ids: Option<Vec<gix::ObjectId>>,
    /// Whether more than one unique worktree is referenced by the graph — the commit debug
    /// renderer keys ownership markers off it.
    pub(crate) has_multiple_worktrees: bool,
}

/// Construction: the workspace-level entry points, mirroring the graph traversals. These are
/// what operations use — the segment view is a build/projection detail, and nothing
/// outside the crate can build one. (The fold: builder and projection fuse behind these.)
impl Workspace {
    /// An empty ad-hoc workspace over an empty graph, for tests that only exercise
    /// projection-level data.
    #[doc(hidden)]
    pub fn empty_ad_hoc_for_testing() -> Self {
        Workspace {
            kind: WorkspaceKind::AdHoc,
            stacks: vec![],
            lower_bound: None,
            target_ref: None,
            target_commit: None,
            metadata: None,
            commit_graph: crate::CommitGraph::default(),
            ctx: GraphContext::default(),
            tip_ref_info: None,
            tip_commit_id: None,
            lower_bound_ref_name: None,
            entrypoint: None,
            target_ref_commit_id: None,
            effective_target_commit_id: None,
            commits_below_tip: Vec::new(),
            incoming_target_commit_ids: None,
            has_multiple_worktrees: false,
        }
    }

    /// Build the workspace as seen from `HEAD` — the standard entry point.
    pub fn from_head(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::walk::Options,
    ) -> anyhow::Result<Self> {
        let (cg, ctx) = crate::CommitGraph::from_head(repo, meta, project_meta, options)?;
        project_workspace(cg, ctx)
    }

    /// Build the workspace as seen from `tip` (a checkout or preview position), named by
    /// `ref_name` if the position is checked out as a ref.
    pub fn from_tip(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl but_core::RefMetadata,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::walk::Options,
    ) -> anyhow::Result<Self> {
        let (cg, ctx) = crate::CommitGraph::from_tip(tip, ref_name, meta, project_meta, options)?;
        project_workspace(cg, ctx)
    }

    /// Build the workspace from multiple `tips` at once — the multi-tip variant of
    /// [`Self::from_tip`].
    pub fn from_seeds(
        repo: &gix::Repository,
        tips: impl IntoIterator<Item = crate::walk::Seed>,
        meta: &impl but_core::RefMetadata,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::walk::Options,
    ) -> anyhow::Result<Self> {
        let (cg, ctx) = crate::CommitGraph::from_seeds(repo, tips, meta, project_meta, options)?;
        project_workspace(cg, ctx)
    }

    /// Re-read the world with in-memory `overlay` refs and metadata, yielding the workspace as
    /// it would look after applying them.
    pub fn redo(
        &self,
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        overlay: crate::walk::Overlay,
    ) -> anyhow::Result<Self> {
        let overlay_for_flip = overlay.clone();
        let (repo, meta, entrypoint) = overlay.into_parts(repo, meta);
        let (seed, ref_name) = match entrypoint {
            Some(t) => t,
            None => {
                let ep = self
                    .entrypoint
                    .as_ref()
                    .context("BUG: entrypoint must always be set")?;
                let mut ref_name = ep.ref_name.clone();
                let seed = if let Some(name) = ref_name.as_ref() {
                    match repo.try_find_reference(name.as_ref())? {
                        Some(mut reference) => Some(reference.peel_to_id()?.detach()),
                        None => {
                            // The previous traversal may have had a named entrypoint, but
                            // this overlay can drop that ref. If so, don't carry a stale
                            // entrypoint_ref override into the new traversal; it would fail
                            // validation instead of re-traversing from the remembered commit.
                            ref_name = None;
                            None
                        }
                    }
                } else {
                    None
                };
                let seed = seed.or(ep.commit_id).context(
                    "BUG: entrypoint must either remember the original commit id or have a resolvable ref",
                )?;
                (seed, ref_name)
            }
        };
        // The same dispatch as `from_tip`, with the overlay served from memory by
        // the builders.
        let (cg, ctx) = crate::CommitGraph::dispatch_tip(
            repo.for_attach_only(),
            seed,
            ref_name,
            meta.for_inner_only(),
            self.ctx.project_meta.clone(),
            self.ctx.options.clone(),
            overlay_for_flip,
        )?;
        project_workspace(cg, ctx)
    }

    /// Validate the carried graph's invariants (see `CommitGraph::validation_errors`), passing `self`
    /// through for chaining — the workspace-level twin tests use after construction.
    pub fn validated(self) -> anyhow::Result<Self> {
        if let Some(err) = self.commit_graph.validation_errors().pop() {
            return Err(err);
        }
        Ok(self)
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
    /// The stacks that follow will be unusable if the workspace commit sits in a segment
    /// below; with just one real stack, or only empty (ref-only) stacks, they typically
    /// remain usable.
    ManagedMissingWorkspaceCommit {
        /// The name of the reference pointing to the workspace commit. Useful for deriving the workspace name.
        ref_info: crate::RefInfo,
    },
    /// A segment is checked out directly.
    ///
    /// It can be inside or outside a workspace.
    /// If the respective segment is [not named](Workspace::ref_name), this means the `HEAD` is detached.
    /// The commit that the working tree is at is always implied to be the first commit of the [`crate::workspace::StackSegment`]
    /// at the workspace's tip segment.
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
#[derive(Clone)]
pub struct TargetRef {
    /// The name of the target branch, i.e. the branch that all [Stacks](Stack) want to get merged into.
    /// Typically, this is `refs/remotes/origin/main`.
    pub ref_name: gix::refs::FullName,
    /// The commit the target rests on, resolved through empty segments.
    pub(crate) tip_commit_id: Option<gix::ObjectId>,
    /// The number of commits on the target that the workspace doesn't have yet — its future.
    pub commits_ahead: usize,
}

/// Information about the target commit, which marks a portion in the commit-graph
/// that the workspace wants to integrate with.
///
/// It's an unnamed point of interest which may
/// be set by any means. Typically, it's set by using a stored value, which makes it
/// a point in time at which we have seen the [`TargetRef`].
#[derive(Clone)]
pub struct TargetCommit {
    /// The hash of the commit that was once included in the [target ref](TargetRef), and that we remember to expand
    /// the reach of the workspace.
    pub commit_id: gix::ObjectId,
}

/// The resolved tip is a lookup detail; keep snapshot output to the stable facts.
impl std::fmt::Debug for TargetRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TargetRef")
            .field("ref_name", &self.ref_name)
            .field("commits_ahead", &self.commits_ahead)
            .finish()
    }
}

impl std::fmt::Debug for TargetCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TargetCommit")
            .field("commit_id", &self.commit_id)
            .finish()
    }
}

/// The commits upstream of `target_tip`: everything reachable from it that is
/// neither the workspace's shared history nor the workspace itself.
///
/// Shared-history semantics (the disjoint-target rule): paint the lower bound's
/// ancestor set, then collect from the target tip until the walk TOUCHES shared
/// history — the paint, or any workspace-reachable commit. Diverged commits are
/// never in either, so a rewritten remote is collected at any depth. A walk that
/// never touches shared history found a DISJOINT target — nothing is upstream —
/// unless either side's ancestry ends at a traversal CUT, in which case the
/// shared base may lie beyond the window and everything visible stays.
pub(crate) fn upstream_commits(
    cg: &crate::CommitGraph,
    target_tip: gix::ObjectId,
    lower_bound: Option<gix::ObjectId>,
) -> Vec<gix::ObjectId> {
    let shared_history =
        lower_bound.and_then(|lb| cg.node(lb).is_some().then(|| cg.ancestor_marks(lb)));
    let mut touched_shared_history = false;
    let mut collected = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut runs = std::collections::VecDeque::from([target_tip]);
    while let Some(run_start) = runs.pop_front() {
        let mut cursor = Some(run_start);
        while let Some(id) = cursor.take() {
            if !seen.insert(id) {
                break;
            }
            let Some(node) = cg.node(id) else { break };
            let in_shared_history = shared_history
                .as_ref()
                .is_some_and(|shared| cg.index_of(id).is_some_and(|i| shared[i]));
            let in_workspace = node.flags.contains(crate::CommitFlags::InWorkspace);
            touched_shared_history |= in_shared_history || in_workspace;
            if Some(id) == lower_bound || in_shared_history || in_workspace {
                break;
            }
            collected.push(id);
            let parents = cg.all_parent_ids(id);
            cursor = parents.first().copied();
            runs.extend(parents.into_iter().skip(1));
        }
    }
    if let Some(shared) = shared_history.as_ref().filter(|_| !touched_shared_history) {
        let genuinely_disjoint = !collected.iter().any(|id| cg.has_cut_parents(*id))
            && !shared
                .iter()
                .enumerate()
                .any(|(i, &marked)| marked && cg.has_cut_parents_at(i));
        if genuinely_disjoint {
            return Vec::new();
        }
    }
    collected
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
