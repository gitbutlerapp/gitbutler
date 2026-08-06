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
pub use stack::{
    AdvancedOutside, Stack, StackCommit, StackCommitDebugFlags, StackCommitFlags, StackSegment,
};

mod anchor;
pub(crate) mod api;
mod derive;
mod frame;
mod partition;
mod presence;
pub use api::StackTip;
pub(crate) use derive::derive_workspace;

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
    /// The options used to create the graph, which allow it to be regenerated ([`Workspace::rederive_with`])
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

/// The workspace: the commit graph (with its stored ref layout), the build context,
/// and the frame — everything the derivations run from — plus the scalar facts
/// captured at build time. The derived stacks are stored as their
/// [`stacks`](Self::stacks) reduction — the operations' authority; materialized
/// stacks exist per [`display_stacks`](Self::display_stacks) call, for display alone.
#[derive(Clone)]
pub struct Workspace {
    /// The target, as identified by a remote tracking branch, to integrate workspace stacks into.
    ///
    /// If `None`, and if [`stored_target_commit_id`](Self::stored_target_commit_id) is `None`, this is a local workspace that doesn't know when
    /// possibly pushed branches are considered integrated. This happens when there is a local branch
    /// checked out without a remote tracking branch.
    pub target_ref: Option<TargetRef>,
    /// Read-only workspace metadata with additional information, or `None` if nothing was present.
    /// If this is `Some()` the `kind` is always [`WorkspaceKind::Managed`]
    ///
    /// # WARNING
    ///
    /// Do not use this data to understand the workspace. It's raw, underived metadata which may
    /// have nothing to do with the actual workspace.
    /// To see that, look at the [`stacks`](Self::stacks) or
    /// [`display_stacks`](Self::display_stacks).
    pub metadata: Option<ref_metadata::Workspace>,

    // ── The derivation inputs. ──
    /// The frame the workspace was built from, retained VERBATIM so the derivation
    /// derives from the exact build-time inputs (the public `metadata` copy may
    /// be mutated by legacy callers; the derivation must not see that).
    pub(crate) frame: frame::WorkspaceFrame,
    /// The derived stacks as a fact: which refs form which stacks, in workspace (parent)
    /// order, at their full in-workspace extent — reduced from the derivation at
    /// construction like every other captured fact. The operations-facing shape.
    ///
    /// A partition, not a graph: no edges between stacks. Inter-stack structure is implicit in
    /// this Vec's order (= workspace-merge parent order) and each stack's
    /// [`base`](SegmentStack::base); the real commit DAG lives in
    /// [`commit_graph`](Self::commit_graph). What these uniquely hold — the only thing a plain
    /// commit-graph walk cannot reproduce — is the DECIDED EXTENT (a sibling stack's colouring
    /// can cut a segment where no first-parent walk could tell).
    pub stacks: Vec<SegmentStack>,
    /// The commit graph the workspace was projected from — the commit-addressed
    /// substrate all queries answer from.
    pub(crate) commit_graph: crate::CommitGraph,
    /// The build-time context: options, project metadata, and repository-derived
    /// remote relationships.
    pub(crate) ctx: GraphContext,
}

impl Workspace {
    // ── Construction — the workspace-level entry points, mirroring the graph traversals.
    // These are what operations use; the segment view is a build/projection detail nothing
    // outside the crate can build. (These fuse the graph build and the derivation.) ──

    /// Build the workspace as seen from `HEAD` — the standard entry point.
    pub fn from_head(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::walk::Options,
    ) -> anyhow::Result<Self> {
        let (cg, ctx) = crate::CommitGraph::from_head(repo, meta, project_meta, options)?;
        derive::derive_workspace(cg, ctx)
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
        derive::derive_workspace(cg, ctx)
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
        derive::derive_workspace(cg, ctx)
    }

    /// Validate the carried graph's invariants (see `CommitGraph::validation_errors`), passing `self`
    /// through for chaining — the workspace-level twin tests use after construction.
    pub fn validated(self) -> anyhow::Result<Self> {
        if let Some(err) = self.commit_graph.validation_errors().pop() {
            return Err(err);
        }
        Ok(self)
    }

    // ── Re-derivation — rebuild the workspace from an overlay or a mutated arena. ──

    /// Re-read the world with in-memory `overlay` refs and metadata, yielding the
    /// workspace as it would look after applying them — the mutation-facing rebuild.
    pub fn rederive_with(
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
                let ep = self.derived_entrypoint();
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
        // The same dispatch as `Workspace::from_tip`, with the overlay served from memory
        // by the builders.
        let (cg, ctx) = crate::CommitGraph::dispatch_tip(
            repo.for_attach_only(),
            seed,
            ref_name,
            meta.for_inner_only(),
            self.ctx.project_meta.clone(),
            self.ctx.options.clone(),
            overlay_for_flip,
        )?;
        derive::derive_workspace(cg, ctx)
    }

    /// Preview `commit_graph` — typically a not-yet-materialized rebase's mutated arena —
    /// with the rebase's pending ref edits and entrypoint served from `overlay`, yielding
    /// the workspace as it would look once materialized.
    ///
    /// Falls back to an overlay rewalk ([`Self::rederive_with`]) when the commit graph has nothing to
    /// project: the entrypoint is unborn or points outside the graph.
    pub fn preview_from_commit_graph(
        &self,
        commit_graph: crate::CommitGraph,
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        overlay: crate::walk::Overlay,
    ) -> anyhow::Result<Self> {
        match crate::workspace_from_commit_graph(
            commit_graph,
            repo,
            meta,
            self.ctx.project_meta.clone(),
            self.ctx.options.clone(),
            overlay.clone(),
        )? {
            Some(preview) => Ok(preview),
            None => self.rederive_with(repo, meta, overlay),
        }
    }

    /// THE derivation, derived fresh from the carried inputs — commit graph (with
    /// its stored ref layout), build context, and the retained frame. The [`stacks`](Self::stacks)
    /// is its build-time reduction (the stored authority); the display materializes
    /// from that stored reduction per [`display_stacks`](Self::display_stacks) call.
    ///
    /// STRUCTURE, not display: segments, extents and commit ids are decided here, but the refs
    /// riding on each commit are left exactly as the graph holds them. Which of them a commit
    /// shows, and in what order, is decided when the display materializes —
    /// [`display_stacks`](Self::display_stacks) is the only thing that answers that.
    pub fn derive_stacks(&self) -> Vec<Stack> {
        derive::derive_stacks(&self.commit_graph, &self.ctx, &self.frame)
    }

    // ── Test-only helpers ──

    /// An empty ad-hoc workspace over an empty graph, for tests that only exercise
    /// projection-level data.
    #[doc(hidden)]
    pub fn empty_ad_hoc_for_testing() -> Self {
        Workspace {
            target_ref: None,
            metadata: None,
            // Deriving over the empty graph + frame yields no stacks — lazily correct.
            frame: frame::WorkspaceFrame {
                kind: WorkspaceKind::AdHoc,
                metadata: None,
                tip_commit: None,
                tip_ref_info: None,
                entry_ref: None,
                entry_commit: None,
                entry_inside: false,
                lower_bound: None,
                lower_bound_ref_name: None,
                target_ref: None,
                target_commit: None,
                integrated_tip_commits: Vec::new(),
                missing_target_ref: None,
            },
            stacks: Vec::new(),
            commit_graph: crate::CommitGraph::default(),
            ctx: GraphContext::default(),
        }
    }

    /// Replace the derived `stacks` with ones reduced from hand-built stacks — for tests.
    #[doc(hidden)]
    pub fn set_stacks_from_derived_for_testing(&mut self, stacks: &[Stack]) {
        self.stacks = reduce_to_segment_stacks(stacks);
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
        /// The managed workspace commit still backing this degraded view — resolved as
        /// the projection anchor (materialized parents / a gitbutler-namespace ref naming
        /// the merge) or found on the first-parent line below the reference (the ref
        /// advanced past its merge). Stacks project from this merge; any commits between
        /// the reference and the merge belong to no stack and await relocation by the user.
        workspace_commit_below: Option<gix::ObjectId>,
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
}

/// The resolved tip is a lookup detail; keep snapshot output to the stable facts.
impl std::fmt::Debug for TargetRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TargetRef")
            .field("ref_name", &self.ref_name)
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

/// The `(stack_idx, segment_idx)` of the segment named `ref_name` in `stacks`, if any: the
/// ref→stack membership query as a free function over a stack slice, callable without a
/// whole [`Workspace`].
///
/// The indices are RELATIVE TO `stacks` and mean nothing outside it. Stack order is derived
/// — from merge-parent order, declared index and entrypoint steering — so two views of one
/// graph can order the same branches differently. Use these to walk the slice you were given,
/// never to address a stack in some other view of the same repository.
pub fn find_segment_owner_indexes_by_refname(
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

/// One stack of the partition: identity plus its segments, tip→base.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentStack {
    /// The stable stack id, when declared.
    pub id: Option<but_core::ref_metadata::StackId>,
    /// The segments, topmost first (declared topological order for DAG shapes). A
    /// self-contained node each — the stack is only the ordering container. Consumers
    /// read intra-stack adjacency from THIS order (`idx ± 1`), not from `edges`.
    pub segments: Vec<Segment>,
    /// Parent edges between segments, `(child_idx, parent_idx)` into
    /// [`segments`](Self::segments). An unforked stack stores plain `(i, i + 1)` adjacency; a
    /// forked or converged one stores real fork/merge edges. The display's LANE VIEW is derived
    /// from these — one lane per stored tip — so this is load-bearing, not scaffolding.
    pub edges: Vec<(usize, usize)>,
    /// The commit this stack rests on, if any.
    pub base: Option<gix::ObjectId>,
}

/// One segment node: a named (or anonymous) run of commits, reduced to its anchors.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// The branch naming this segment; `None` for anonymous runs.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commits assigned to this segment, tip→bottom — the extent as the derivation
    /// decided it (a sibling stack's colouring can cut it where no graph walk could tell).
    /// Ids only: materialization stays display's job.
    pub commits: Vec<gix::ObjectId>,
    /// The segment's base as the derivation materialized it — the commit below it
    /// ALONG THE WALKED GRAPH, `None` when the walk was severed or hit a root. Display
    /// range queries read this.
    pub base: Option<gix::ObjectId>,
    /// Whether the branch carries metadata (its sideband was found by name).
    pub has_metadata: bool,
}

impl Segment {
    /// The segment's name, if not anonymous.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_name.as_ref().map(|n| n.as_ref())
    }

    /// The segment's topmost own commit; `None` for empty segments.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.commits.first().copied()
    }
}

impl Workspace {
    /// The stack carrying `id`, if any. `None` finds the first identityless stack.
    pub fn stack_by_id(
        &self,
        id: impl Into<Option<but_core::ref_metadata::StackId>>,
    ) -> Option<&SegmentStack> {
        let id = id.into();
        self.stacks.iter().find(|s| s.id == id)
    }

    /// Every named segment's branch, across all stacks, in workspace order.
    pub fn segment_names(&self) -> impl Iterator<Item = &gix::refs::FullNameRef> {
        self.stacks.iter().flat_map(|s| s.segment_names())
    }

    /// The `(stack_idx, segment_idx)` of the segment named `name`, if any.
    pub fn segment_location(&self, name: &gix::refs::FullNameRef) -> Option<(usize, usize)> {
        self.stacks
            .iter()
            .enumerate()
            .find_map(|(stack_idx, stack)| {
                stack
                    .segments
                    .iter()
                    .position(|s| s.ref_name() == Some(name))
                    .map(|seg_idx| (stack_idx, seg_idx))
            })
    }
}

impl SegmentStack {
    /// The stack's name: its topmost segment's branch, if named.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.top().and_then(|s| s.ref_name())
    }

    /// The topmost segment, if any.
    pub fn top(&self) -> Option<&Segment> {
        self.segments.first()
    }

    /// The first non-empty segment's tip — the stack's resting commit above the base.
    pub fn tip_skip_empty(&self) -> Option<gix::ObjectId> {
        self.segments.iter().find_map(|s| s.tip())
    }

    /// The commit this stack RESTS on toward the workspace merge: its first non-empty
    /// segment's tip, else its base.
    pub fn resting_commit(&self) -> Option<gix::ObjectId> {
        self.tip_skip_empty().or(self.base)
    }

    /// The named segments' branches, top→base.
    pub fn segment_names(&self) -> impl Iterator<Item = &gix::refs::FullNameRef> {
        self.segments.iter().filter_map(|s| s.ref_name())
    }

    /// The segments from `idx` toward the base — a segment and its successors.
    pub fn segments_at_or_below(&self, idx: usize) -> &[Segment] {
        self.segments.get(idx..).unwrap_or(&[])
    }
}

/// Reduce materialized `stacks` — the derivation output — to the stored segment stacks.
pub fn reduce_to_segment_stacks(stacks: &[Stack]) -> Vec<SegmentStack> {
    stacks
        .iter()
        .map(|stack| SegmentStack {
            id: stack.id,
            base: stack.base(),
            // A forked or converged stack carries real edges; an unforked
            // one is plain adjacency.
            edges: stack
                .branch_parents
                .clone()
                .unwrap_or_else(|| (1..stack.segments.len()).map(|i| (i - 1, i)).collect()),
            segments: stack
                .segments
                .iter()
                .map(|segment| Segment {
                    ref_name: segment.ref_name().map(ToOwned::to_owned),
                    commits: segment.commits.iter().map(|c| c.id).collect(),
                    base: segment.base,
                    has_metadata: segment.metadata.is_some(),
                })
                .collect(),
        })
        .collect()
}

/// The stack with `id` within `stacks`, if any — bind [`Workspace::display_stacks`] (or the
/// derivation) at the call site and look up in it.
pub fn find_stack_by_id(
    stacks: &[Stack],
    id: impl Into<Option<but_core::ref_metadata::StackId>>,
) -> Option<&Stack> {
    let id = id.into();
    stacks.iter().find(|s| s.id == id)
}

/// The commit `oid` within `stacks`, with its containing segment and stack.
pub fn find_commit_in(
    stacks: &[Stack],
    oid: gix::ObjectId,
) -> Option<(&Stack, &StackSegment, &StackCommit)> {
    stacks.iter().find_map(|stack| {
        stack.segments.iter().find_map(|seg| {
            seg.commits
                .iter()
                .find(|c| c.id == oid)
                .map(|c| (stack, seg, c))
        })
    })
}

/// Whether `name` names the topmost NAMED segment of some stack in `stacks` (a stack TIP): the
/// tip-ness query as a free function over a stack slice, callable without a [`Workspace`].
pub fn is_stack_tip(stacks: &[Stack], name: &gix::refs::FullNameRef) -> bool {
    stacks
        .iter()
        .any(|stack| stack.segments.iter().find_map(|segment| segment.ref_name()) == Some(name))
}
