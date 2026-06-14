use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, bail, ensure};
use bstr::ByteSlice;
use but_core::{
    RefMetadata, extract_remote_name_and_short_name,
    ref_metadata::{self, ProjectMeta},
};
use gix::refs::Category;

use crate::{CommitFlags, SegmentMetadata};

pub(crate) mod commit_walk;
mod walk;
pub(crate) use walk::disambiguate_refs_by_branch_metadata;
use walk::*;

pub(crate) mod types;
use types::{Goals, Limit, Queue, Step};

use crate::init::overlay::{OverlayMetadata, OverlayRepo};

pub(crate) mod remotes;

pub(crate) mod overlay;

pub(crate) type Entrypoint = Option<(gix::ObjectId, Option<gix::refs::FullName>)>;

/// A resolved commit tip to seed graph traversal without requiring it to be
/// discoverable through repository refs or workspace metadata.
///
/// ## Traversal invariants
///
/// The traversal will build a segment graph, where Segments follow specific rules.
/// We differentiate between tip segments, segments created from [Tip]s, (*TS*) and
/// ancestor segments (*AS*), which are ancestors of *TS* and connected to them by outgoing
/// connections.
///
/// - Virtual segments (*VS*) are created by the projection to represent refs
///   which are described in [but_core::ref_metadata::Workspace]. They are named
///   and always empty graph nodes, and ordinary virtual segments have *exactly one*
///   outgoing connection that lets the projection
///   find the commit named by the ref. The commit is owned by another segment, sometimes
///   because another segment was prioritized when multiple refs point to the same commit.
/// - The virtual workspace tip segment is a special kind of *VS*, which may have one or more
///   outgoing connections, pointing to one or more *VS* or *AS*. As such, such Segments cannot
///   unambiguously determine the commit their [Self::ref_name] points to as multiple paths can
///   be followed, yielding multiple commits.
///   Note that ordinary workspace tip segments may also exist as *TS*, which do own a commit,
///   which *typically* is the workspace commit.
/// - After the traversal, before the projection, forks and joins of the underlying
///   commit graph are represented by segments. This allows traversals or
///   graph computations, like merge-bases, to work the same as on the commit-graph, but
///   possibly with less jumps among nodes as segments may contain more than one commit,
///   allowing to skip over uninteresting commits naturally.
/// - After the projection, the graph may not fully represent the commit-graph anymore
///   due to the creation of *VS*. What makes a *VS* virtual is not the ref itself,
///   but that its relationship to other segments is not represented by the Git
///   commit-graph or by Git refs: to Git, these are refs pointing to the same commit,
///   while GitButler sees one or more stacks of branches with specific ordering.
/// - *TS* with [Self::ref_name] set will return that as its segment name.
/// - *TS* that contain [Self::id] contain it as first commit
/// - *TS* that don't contain [Self::id] are empty and can find their commit by following
///   their only outgoing connection until a non-empty commit is found which contains
///   [Self::id] as *first* commit!
/// - *TS* or *AS* with *more than one* outgoing connection have *at least one* commit.
#[derive(Debug, Clone)]
pub struct Tip {
    /// The commit id to start walking from.
    pub id: gix::ObjectId,
    /// The ref name to assign to the tip segment, if it should be named.
    pub ref_name: Option<gix::refs::FullName>,
    /// How this tip participates in traversal.
    pub role: TipRole,
    /// Metadata to attach to the initial segment.
    pub metadata: Option<SegmentMetadata>,
    /// Whether this tip is the user-facing traversal entrypoint.
    ///
    /// There may only be *one such tip*.
    /// Other tips try to connect to any commit reachable from this one.
    pub is_entrypoint: bool,
    /// Whether the entrypoint segment should remain anonymous even if refs
    /// point at the same commit.
    pub is_detached: bool,
}

/// Lifecycle
impl Tip {
    /// A traversal tip with default reachable semantics.
    ///
    /// This is the smallest tip description: it starts at `id`, is unnamed, is
    /// not the entrypoint, carries no metadata, and queues after existing
    /// initial work.
    pub fn new(id: gix::ObjectId) -> Self {
        Tip {
            id,
            ref_name: None,
            role: TipRole::default(),
            metadata: None,
            is_entrypoint: false,
            is_detached: false,
        }
    }

    /// A normal named or unnamed traversal entrypoint.
    ///
    /// `id` is the commit where graph traversal starts.
    /// `ref_name` names the entrypoint segment when the caller has a stable ref
    /// for it.
    pub fn entrypoint(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Tip::new(id).with_ref_name(ref_name).with_entrypoint()
    }

    /// An entrypoint whose segment should remain detached even if refs point to
    /// its commit.
    ///
    /// `id` is the commit where graph traversal starts.
    pub fn detached_entrypoint(id: gix::ObjectId) -> Self {
        Tip::new(id).with_detached_entrypoint()
    }

    /// A non-remote tip that should be included in the traversal.
    ///
    /// `id` is the commit to include as another non-remote traversal root.
    /// `ref_name` names the tip segment when the caller has a stable ref for it.
    pub fn reachable(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Tip::new(id).with_ref_name(ref_name)
    }

    /// A target/integration tip that bounds or extends traversal context.
    /// It represents part of the graph that [`Self::reachable()`] parts want to integrate with.
    ///
    /// `id` is the commit to treat as integrated history.
    /// `ref_name` names the target segment when the caller has a stable ref for
    /// it.
    pub fn integrated(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Tip::new(id)
            .with_ref_name(ref_name)
            .with_role(TipRole::TargetRemote)
    }
}

/// Builder
impl Tip {
    /// Set the ref name used to enforce the name this tip segment.
    pub fn with_ref_name(mut self, ref_name: Option<gix::refs::FullName>) -> Self {
        self.ref_name = ref_name;
        self
    }

    /// Set the traversal role for this tip.
    pub fn with_role(mut self, role: TipRole) -> Self {
        self.role = role;
        self
    }

    /// Set whether this tip is the traversal entrypoint.
    pub fn with_is_entrypoint(mut self, is_entrypoint: bool) -> Self {
        self.is_entrypoint = is_entrypoint;
        self
    }

    /// Set whether this tip should use detached entrypoint presentation, which makes it anonymous even
    /// if it could receive a name/unambiguous ref otherwise.
    pub fn with_is_detached(mut self, is_detached: bool) -> Self {
        self.is_detached = is_detached;
        self
    }

    /// Mark this tip as the traversal entrypoint.
    pub fn with_entrypoint(self) -> Self {
        self.with_is_entrypoint(true)
    }

    /// Mark this entrypoint as detached for segment presentation.
    pub fn with_detached_entrypoint(mut self) -> Self {
        self = self.with_is_entrypoint(true).with_is_detached(true);
        self
    }

    /// Attach metadata to the initial segment created for this tip.
    pub fn with_metadata(mut self, metadata: SegmentMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Utilities
impl Tip {
    /// Return whether this tip is anonymous integrated target context.
    ///
    /// Named target remotes can represent refs that need their own segment and
    /// target/local sibling relationship. Anonymous target remotes have no ref
    /// to preserve in the projection; they represent commit-only target
    /// context such as `extra_target_commit_id` or a persisted workspace target
    /// commit.
    fn is_anonymous_integrated_target_context(&self) -> bool {
        matches!(self.role, TipRole::TargetRemote) && self.ref_name.is_none()
    }

    /// Return whether this anonymous integrated target tip is auxiliary
    /// traversal context.
    ///
    /// Anonymous target remotes can be provided explicitly by callers and
    /// usually remain normal traversal seeds. The `auxiliary_integrated_tip_ids`
    /// set records the anonymous integrated targets that normalization derived
    /// from metadata or options such as `extra_target_commit_id`; those tips act
    /// as mergeable limits/context and should be ordered or deduplicated as
    /// auxiliary work rather than as user-visible roots.
    ///
    /// If an anonymous target points to the same commit as a named target ref,
    /// normalization collapses it into the named tip.
    fn is_auxiliary_integrated_tip(
        &self,
        auxiliary_integrated_tip_ids: &BTreeSet<gix::ObjectId>,
    ) -> bool {
        self.is_anonymous_integrated_target_context()
            && auxiliary_integrated_tip_ids.contains(&self.id)
    }

    /// Return whether this anonymous integrated target should reuse the named
    /// target traversal seed for the same commit.
    ///
    /// The anonymous tip only contributes commit-level target context
    /// (tips with [TipRole::TargetRemote]). It does not need its own segment or
    /// queue item when a named target ref already points at that commit,
    /// and keeping both can make the anonymous seed own the commit while
    /// the named ref is left as a duplicate empty segment.
    fn collapses_into_named_integrated_target(
        &self,
        named_integrated_target_ids: &BTreeSet<gix::ObjectId>,
    ) -> bool {
        self.is_anonymous_integrated_target_context()
            && named_integrated_target_ids.contains(&self.id)
    }
}

/// The role a resolved traversal tip plays when constructing a graph.
///
/// Roles decide the initial [`CommitFlags`] and `Limit` goals used by the
/// walk. The explicit entrypoint is the shared goal: reachable and integrated
/// tips seek connection to it by walking history until they encounter the entrypoint's
/// propagated goal flag.
///
/// Remote-tracking tips are not modeled as explicit [`TipRole`] values. They
/// are discovered during traversal from refs found at visited commits and their
/// configured or deduced remote-tracking branches. When such a remote tip is
/// queued, it receives an indirect goal for the local commit where it was
/// discovered, while that local side receives a goal for the remote tip. This
/// reciprocal goal setup lets remote and local tracking histories converge until
/// the graph can connect them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TipRole {
    /// A non-remote tip that should be traversed and related to the entrypoint.
    ///
    /// This tip marks all commits it traverses with [`CommitFlags::NotInRemote`].
    #[default]
    Reachable,
    /// The workspace ref itself, paired with workspace metadata on [`Tip`].
    ///
    /// This marks commits as in-workspace with [`CommitFlags::InWorkspace`].
    Workspace,
    /// A branch from a stack listed in workspace metadata.
    ///
    /// Its current ref tip should be traversed even if it is not reachable from
    /// the workspace commit.
    WorkspaceStackBranch {
        /// Ref name from workspace metadata to use for segment naming if the
        /// initial segment cannot infer an unambiguous ref from the tip commit.
        ///
        /// This is not [`Tip::ref_name`] because that field forces the initial
        /// segment to use the supplied name. Workspace stack branches should
        /// still allow normal ref discovery to pick an unambiguous local branch
        /// at the tip commit, or to leave the segment anonymous when local
        /// naming is ambiguous. The desired name is only a fallback for
        /// remote-only stack refs that cannot be discovered by local-branch
        /// disambiguation.
        ///
        /// Note that [Tip::id] is assumed to be the peeled commit that this
        /// ref points to.
        desired_ref_name: gix::refs::FullName,
    },
    /// A target/integration tip whose reachable history is considered integrated,
    /// and that reachable/unintegrated tips want to connect with.
    ///
    /// This tip receives [`CommitFlags::Integrated`] and an indirect goal for
    /// the entrypoint commit with no extra allowance once that goal is found. It
    /// walks just far enough to connect target history to the entrypoint's
    /// ancestry.
    TargetRemote,
    /// The local branch that tracks an integrated target branch.
    ///
    /// It receives a goal for the target and later provides the segment id that
    /// lets the target segment point back to its local sibling.
    TargetLocal {
        /// The expected local tracking ref name used to verify whether the
        /// segment that normal ref discovery created is actually the local side
        /// of this target.
        ///
        /// This is not [`Tip::ref_name`] because that would force the segment
        /// to use this name and bypass ambiguity checks. If multiple local
        /// branches point to the same commit, or discovery chooses a different
        /// unambiguous name, the target should still get the local goal but not
        /// a direct sibling link.
        ///
        /// This matters when the target's local tracking branch shares its tip
        /// with another local branch, such as a workspace stack branch or a
        /// second branch with metadata. In that state, the segment may
        /// represent that other branch or stay anonymous; linking it as the
        /// target local side would make target ahead/behind and remote-reachability
        /// queries treat the wrong segment as the tracking branch.
        local_ref_name: gix::refs::FullName,
    },
}

/// Access
impl TipRole {
    /// Whether this role represents integrated history.
    pub fn is_integrated(&self) -> bool {
        matches!(self, TipRole::TargetRemote)
    }
}

/// A local branch ref and the commit it points to, when it tracks a workspace
/// target ref.
type LocalTrackingTip = (gix::refs::FullName, gix::ObjectId);

/// A workspace target ref, its commit, and optionally the local branch tracking it.
type WorkspaceTargetTip = (gix::refs::FullName, gix::ObjectId, Option<LocalTrackingTip>);

/// The complete pre-traversal plan derived from either explicit tips or
/// workspace metadata.
///
/// [`queue_initial_tips()`] consumes this value to create graph *segments*, seed
/// the traversal queue, and provide the auxiliary ref and remote information
/// needed by traversal and the projection.
///
/// This means that each of these tip *will get its own possibly empty* graph segment.
pub(crate) struct InitialTips {
    /// Ordered traversal roots to turn into segments and queue items.
    tips: Vec<Tip>,
    /// Workspace commits used to ensure commits remain owned by the workspace
    /// roots that introduced them.
    workspace_tips: Vec<gix::ObjectId>,
    /// Workspace ref names that should be included while collecting refs by
    /// prefix, even when they are not reachable from the entrypoint yet.
    workspace_ref_names: Vec<gix::refs::FullName>,
    /// Remote target refs that were already scheduled as initial integrated
    /// tips.
    ///
    /// Workspace traversals seed this list from the project metadata target
    /// ref. Explicit traversal seeds the same list from integrated tip ref
    /// names. During traversal, `try_queue_remote_tracking_branches()` uses
    /// it to avoid queueing those target refs again when local branch refs
    /// point at them as upstreams.
    // TODO: could this be removed in favor os using `Graph::traversal_tips`?
    target_refs: Vec<gix::refs::FullName>,
    /// Remote names to try when a local branch has no configured upstream.
    ///
    /// `lookup_remote_tracking_branch_or_deduce_it()` first asks Git for the
    /// branch's configured remote-tracking ref. If none exists, it tries each
    /// name here by constructing `refs/remotes/<remote>/<local-short-name>` and
    /// using it only if that ref exists and is not already configured for
    /// another branch.
    symbolic_remote_names: Vec<String>,
    /// Whether metadata-derived workspace/target tips should be front-loaded
    /// into the traversal queue after their segments are created.
    frontload_workspace_related_tips: bool,
    /// Target remote/local tracking relationships inferred from tip refs and
    /// repository branch configuration.
    ///
    /// These links are needed before traversal starts because target and local
    /// tracking tips may point to the same commit, or may be reached in either
    /// order. Queueing uses this map to delay the target side until the local
    /// side has a segment and goal, then links both segments as siblings before
    /// their commits can be claimed by unrelated stack or reachable tips. That
    /// keeps target ownership, ahead/behind, and remote-reachability queries
    /// anchored to the intended target/local pair.
    target_local_links: TargetLocalLinks,
    /// Anonymous target-remote tips that are auxiliary traversal context rather
    /// than primary target refs.
    auxiliary_integrated_tip_ids: BTreeSet<gix::ObjectId>,
}

/// Bidirectional lookup between target remote refs and their local tracking refs.
#[derive(Default)]
struct TargetLocalLinks {
    /// Local tracking ref by target remote ref.
    local_by_target: BTreeMap<gix::refs::FullName, gix::refs::FullName>,
    /// Target remote ref by local tracking ref.
    target_by_local: BTreeMap<gix::refs::FullName, gix::refs::FullName>,
}

/// An integrated target that has a segment but cannot be queued yet.
///
/// This temporary state is needed when the target should be linked to a local
/// tracking branch that appears later in the normalized initial-tip list. Once
/// the local side exists, the pending target can be queued with the correct
/// sibling relationship and goal.
struct PendingIntegratedTip {
    /// Commit id of the integrated target.
    id: gix::ObjectId,
    /// Segment created for the integrated target before it is queued.
    segment: usize,
    /// Whether to insert the target before existing initial queue work once it
    /// is released.
    queue_front: bool,
}

/// A way to define information to be served from memory, instead of from the underlying data source, when
/// [initializing](crate::Workspace::from_commit_traversal()) the graph.
#[derive(Debug, Default, Clone)]
pub struct Overlay {
    entrypoint: Entrypoint,
    nonoverriding_references: Vec<gix::refs::Reference>,
    overriding_references: Vec<gix::refs::Reference>,
    /// A list of references that should not be picked up anymore in the
    /// re-traversal.
    ///
    /// For example, if the `but_rebase::graph_rebase::Editor` converts a
    /// `Reference` step to a `None` step which is the equivalent of running
    /// `git update-ref -d`, it should no longer be part of the [`Graph`], so we
    /// would list the particular reference as a dropped reference.
    dropped_references: Vec<gix::refs::FullName>,
    meta_branches: Vec<(gix::refs::FullName, ref_metadata::Branch)>,
    workspace: Option<(gix::refs::FullName, ref_metadata::Workspace)>,
}

/// Options for use in [`Workspace::from_head()`](crate::Workspace::from_head) and
/// [`Workspace::from_commit_traversal()`](crate::Workspace::from_commit_traversal).
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Associate tag references with commits.
    ///
    /// If `false`, tags are not collected.
    pub collect_tags: bool,
    /// The (soft) maximum number of commits we should traverse.
    /// Workspaces with a target branch automatically have unlimited traversals as they rely on the target
    /// branch to eventually stop the traversal.
    ///
    /// If `None`, there is no limit, which typically means that when lacking a workspace, the traversal
    /// will end only when no commit is left to traverse.
    /// `Some(0)` means nothing but the first commit is going to be returned, but it should be avoided.
    ///
    /// Note that this doesn't affect the traversal of integrated commits, which is always stopped once there
    /// is nothing interesting left to traverse.
    ///
    /// Also note: This is a hint and not an exact measure, and it's always possible to receive a more commits
    /// for various reasons, for instance the need to let remote branches find their local branch independently
    /// of the limit.
    pub commits_limit_hint: Option<usize>,
    /// A list of the last commits of partial segments previously returned that reset the amount of available
    /// commits to traverse back to `commit_limit_hint`.
    /// Imagine it like a gas station that can be chosen to direct where the commit-budge should be spent.
    pub commits_limit_recharge_location: Vec<gix::ObjectId>,
    /// As opposed to the limit-hint, if not `None` we will stop queuing new commits after pretty much this many
    /// commits have been seen.
    ///
    /// This is a last line of defense against runaway traversals and for now it's recommended to set it to a high
    /// but manageable value. Note that depending on the commit-graph, we may need more commits to find the local branch
    /// for a remote branch, leaving remote branches unconnected. Commits that are already queued are still processed so
    /// their existing graph connections can be completed.
    ///
    /// Due to multiple paths being taken, more commits may be queued (which is what's counted here) than actually
    /// end up in the graph, so usually one will see many less.
    pub hard_limit: Option<usize>,
    /// Provide the commit that should act like the tip of an additional target reference,
    /// just as if it was set by one of the workspaces.
    /// Everything it touches will be considered integrated, and it can be used
    /// to extend the border of the workspace. Typically, it's a past position
    /// of an existing target, or a target chosen by the user.
    pub extra_target_commit_id: Option<gix::ObjectId>,
    /// Enabling this skips the projection's enrichment of the graph (naming, remote pairing,
    /// metadata materialization) — the clean-up that makes it useful as a workspace.
    ///
    /// Only for previewing the raw graph, e.g. when that enrichment misbehaves.
    pub dangerously_skip_postprocessing_for_debugging: bool,
}

/// Presets
impl Options {
    /// Return options that won't traverse the whole graph if there is no workspace, but will show
    /// more than enough commits by default.
    pub fn limited() -> Self {
        Options {
            collect_tags: false,
            commits_limit_hint: Some(300),
            ..Default::default()
        }
    }
}

/// Builder
impl Options {
    /// Set the maximum amount of commits that each lane in a tip may traverse, but that's less important
    /// than building consistent, connected graphs.
    pub fn with_limit_hint(mut self, limit: usize) -> Self {
        self.commits_limit_hint = Some(limit);
        self
    }

    /// Set a hard limit for the amount of commits to traverse. Even though it may be off by a couple, it's not dependent
    /// on any additional logic.
    ///
    /// ### Warning
    ///
    /// This stops traversal early despite not having discovered all desired graph partitions, possibly leading to
    /// incorrect results. Ideally, this is not used.
    pub fn with_hard_limit(mut self, limit: usize) -> Self {
        self.hard_limit = Some(limit);
        self
    }

    /// Keep track of commits at which the traversal limit should be reset to the [`limit`](Self::with_limit_hint()).
    pub fn with_limit_extension_at(
        mut self,
        commits: impl IntoIterator<Item = gix::ObjectId>,
    ) -> Self {
        self.commits_limit_recharge_location.extend(commits);
        self
    }

    /// Set an additional integrated traversal tip.
    /// It's most useful for tests which want to affect the target of the workspace
    /// without the respective setup.
    /// Application code may use it to set global targets, to reduce the amount of
    /// commits in the workspace even if the entrypoint otherwise is the target branch.
    ///
    /// The commit is queued like an integrated target so traversal can connect
    /// the workspace to history that may otherwise be outside the ordinary
    /// target ref or workspace metadata. The tip is also kept as a tip of
    /// interest and re-resolved by the projection so it can be used as a past
    /// target/base candidate.
    pub fn with_extra_target_commit_id(mut self, id: impl Into<gix::ObjectId>) -> Self {
        self.extra_target_commit_id = Some(id.into());
        self
    }
}

/// Lifecycle
impl crate::Workspace {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn traverse_tips_with_overlay_impl<T: RefMetadata>(
        repo: &OverlayRepo<'_>,
        tips: Vec<Tip>,
        meta: &OverlayMetadata<'_, T>,
        project_meta: ProjectMeta,
        options: Options,
        entrypoint_ref_override: Option<gix::refs::FullName>,
        detached_head: bool,
    ) -> anyhow::Result<crate::Workspace> {
        let entrypoint = validate_explicit_tips(repo, &tips, entrypoint_ref_override.as_ref())?;
        let tip = entrypoint.id;
        let ref_name = if entrypoint.is_detached {
            None
        } else {
            entrypoint_ref_override.or_else(|| entrypoint.ref_name.clone())
        };
        let detach_entrypoint = entrypoint.is_detached;

        {
            if let Some(name) = &ref_name {
                let span = tracing::Span::current();
                span.record("ref_name", name.as_bstr().to_str_lossy().as_ref());
            }
        }
        // The traversal facts the projection reads, populated directly during the walk. The four
        // scalars here are known up front; the entrypoint and per-segment naming are recorded as the
        // walk creates segments.
        let mut state = commit_walk::State::new();
        state.options = options.clone();
        state.entrypoint_ref = ref_name.clone();
        state.project_meta = project_meta;
        let Options {
            collect_tags,
            extra_target_commit_id,
            commits_limit_hint: limit,
            commits_limit_recharge_location: mut max_commits_recharge_location,
            hard_limit,
            dangerously_skip_postprocessing_for_debugging: _,
        } = options;
        let max_limit = Limit::new(limit);
        if ref_name
            .as_ref()
            .is_some_and(|name| name.category() == Some(Category::RemoteBranch))
        {
            // TODO: see if this is a thing - Git doesn't like to checkout remote tracking branches by name,
            //       and if we should handle it, we need to setup the initial flags accordingly.
            //       Also we have to assure not to double-traverse the ref, once as tip and once by discovery.
            bail!("Cannot currently handle remotes as start position");
        }
        let commit_graph = repo.commit_graph_if_enabled()?;
        let shallow_commits = repo.shallow_commits()?;
        let mut buf = Vec::new();

        let configured_remote_tracking_branches =
            remotes::configured_remote_tracking_branches(repo)?;
        let initial_tips =
            initial_tips_from_tips(repo, tips, &state.project_meta, extra_target_commit_id);
        state.traversal_tips = initial_tips.tips.clone();
        let refs_by_id = repo.collect_ref_mapping_by_prefix(
            [
                "refs/heads/",
                // Remote refs are special as we collect them into commits to know about them,
                // just to later remove them unless they are on an actual remote commit.
                // In that case, we also split the segment there if the previous segment then wouldn't be empty.
                // Naturally we only pick them up and segment them if they are added by the local tracking branch
                // that was seen in the walk before.
                "refs/remotes/",
            ]
            .into_iter()
            .chain(if collect_tags {
                Some("refs/tags/")
            } else {
                None
            }),
            &initial_tips
                .workspace_ref_names
                .iter()
                .map(|ref_name| ref_name.as_ref())
                .collect::<Vec<_>>(),
        )?;
        let mut goals = Goals::default();
        // The tip transports itself.
        let tip_flags = CommitFlags::NotInRemote
            | goals
                .flag_for(tip)
                .expect("we more than one bitflags for this");

        let mut next = Queue::new_with_limit(hard_limit);
        let worktree_by_branch =
            repo.worktree_branches(state.entrypoint_ref.as_ref().map(|r| r.as_ref()))?;

        let mut ctx = commit_walk::Context {
            repo,
            symbolic_remote_names: &initial_tips.symbolic_remote_names,
            configured_remote_tracking_branches: &configured_remote_tracking_branches,
            inserted_proxy_segments: Vec::new(),
            refs_by_id,
            hard_limit: false,
            detach_entrypoint,
            worktree_by_branch,
        };

        let target_limit = max_limit
            .with_indirect_goal(tip, &mut goals)
            .without_allowance();
        ctx.inserted_proxy_segments = queue_initial_tips(
            &mut state,
            &mut next,
            &initial_tips,
            tip,
            tip_flags,
            max_limit,
            target_limit,
            &mut goals,
            commit_graph.as_ref(),
            repo,
            meta,
            &ctx,
            &mut buf,
        )?;
        max_commits_recharge_location.sort();

        // Traverse commit-first, recording segment provenance instead of building and splitting
        // segments. The walk populates `state` entirely (scalars, per-segment naming, entrypoint);
        // the returned record graph is no longer read — dropping its construction is the next step.
        let state = commit_walk::traverse(
            state,
            next,
            repo,
            meta,
            &mut ctx,
            &initial_tips,
            &configured_remote_tracking_branches,
            commit_graph.as_ref(),
            shallow_commits.as_ref(),
            &max_commits_recharge_location,
            max_limit,
            &mut goals,
            &mut buf,
        )?;
        crate::workspace::direct::build(state, repo, meta, &ctx, tip, detached_head)
    }
}

/// Re-run the traversal from the rebuild context the projection carried, no record graph required.
impl crate::Workspace {
    /// Like [`Self::redo_traversal_into_workspace_with_overlay()`], without an overlay.
    pub fn into_workspace_of_redone_traversal(
        self,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
    ) -> anyhow::Result<crate::Workspace> {
        self.redo_traversal_into_workspace_with_overlay(repo, meta, Default::default())
    }

    /// Redo the traversal that produced this workspace, applying `overlay`, and return the freshly
    /// projected workspace. Uses [`project_meta`](crate::Workspace::project_meta), the carried
    /// options and entrypoint, so it needs no record graph.
    pub fn redo_traversal_into_workspace_with_overlay(
        &self,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        overlay: Overlay,
    ) -> anyhow::Result<crate::Workspace> {
        let (repo, meta, entrypoint) = overlay.into_parts(repo, meta);
        let (tip, ref_name) = match entrypoint {
            Some(t) => t,
            None => {
                let mut ref_name = self.entrypoint_ref.clone();
                let tip = if let Some(name) = ref_name.as_ref() {
                    match repo.try_find_reference(name.as_ref())? {
                        Some(mut reference) => Some(reference.peel_to_id()?.detach()),
                        None => {
                            ref_name = None;
                            None
                        }
                    }
                } else {
                    None
                };
                let tip = tip.or(self.entrypoint_commit_id).context(
                    "BUG: entrypoint must either remember the original commit id or have a resolvable ref",
                )?;
                (tip, ref_name)
            }
        };
        let tips = initial_tips_from_workspace_metadata(
            &repo,
            &meta,
            tip,
            ref_name.as_ref(),
            &self.project_meta,
            self.options.extra_target_commit_id,
        )?;
        Self::traverse_tips_with_overlay_impl(
            &repo,
            tips,
            &meta,
            self.project_meta.clone(),
            self.options.clone(),
            ref_name,
            false,
        )
    }
}

/// Validate caller-provided traversal tips before they seed graph traversal.
///
/// Explicit tips must name exactly one entrypoint, must not contain duplicate
/// traversal seeds or repeated ref names, must keep detached entrypoints
/// unnamed, and any supplied ref name must resolve to the same commit id as its
/// tip.
fn validate_explicit_tips<'a>(
    repo: &OverlayRepo<'_>,
    tips: &'a [Tip],
    entrypoint_ref_override: Option<&gix::refs::FullName>,
) -> anyhow::Result<&'a Tip> {
    let mut entrypoints = tips.iter().filter(|tip| tip.is_entrypoint);
    let entrypoint = entrypoints
        .next()
        .context("explicit traversal tips require exactly one entrypoint")?;
    ensure!(
        entrypoints.next().is_none(),
        "explicit traversal tips require exactly one entrypoint"
    );

    for (idx, tip) in tips.iter().enumerate() {
        ensure!(
            !tip.is_detached || tip.is_entrypoint,
            "explicit detached tip must also be the entrypoint"
        );
        ensure!(
            !tip.is_detached || tip.ref_name.is_none(),
            "explicit detached entrypoint tip cannot have a ref name"
        );
        ensure!(
            !tip.is_entrypoint || matches!(tip.role, TipRole::Reachable | TipRole::Workspace),
            "explicit entrypoint tip must be reachable or workspace"
        );

        for previous in &tips[..idx] {
            ensure!(
                !tips_have_same_traversal_seed(previous, tip),
                "explicit traversal tips contain duplicate traversal seed {tip:?}"
            );
            if let Some(ref_name) = tip
                .ref_name
                .as_ref()
                .filter(|ref_name| previous.ref_name.as_ref() == Some(*ref_name))
            {
                bail!("explicit traversal tips contain duplicate ref name {ref_name}");
            }
        }

        if let Some(ref_name) = tip.ref_name.as_ref() {
            validate_tip_ref(repo, ref_name, tip.id, "explicit traversal tip ref")?;
        }
    }

    if !entrypoint.is_detached
        && let Some(ref_name) = entrypoint_ref_override
    {
        validate_tip_ref(
            repo,
            ref_name,
            entrypoint.id,
            "explicit traversal entrypoint ref",
        )?;
    }

    Ok(entrypoint)
}

fn validate_tip_ref(
    repo: &OverlayRepo<'_>,
    ref_name: &gix::refs::FullName,
    tip_id: gix::ObjectId,
    context: &str,
) -> anyhow::Result<()> {
    let resolved_id = repo
        .try_find_reference(ref_name.as_ref())?
        .with_context(|| format!("{context} {ref_name} does not exist"))?
        .peel_to_id()?
        .detach();
    ensure!(
        resolved_id == tip_id,
        "{context} {ref_name} points to {resolved_id}, not {tip_id}"
    );
    Ok(())
}

/// Return whether two tips would seed the same traversal work.
///
/// The traversal seed is the commit id, the traversal role, and whether the tip
/// is the entrypoint. Labels and presentation data like `ref_name`, metadata,
/// detached entrypoint mode, and caller order are intentionally ignored here:
/// they can affect naming, the projection, or stable tie-breaking, but they
/// don't make it useful to enqueue the same commit with the same traversal
/// semantics twice.
fn tips_have_same_traversal_seed(previous: &Tip, tip: &Tip) -> bool {
    previous.id == tip.id
        && tips_have_same_seed_role(previous, tip)
        && previous.is_entrypoint == tip.is_entrypoint
}

/// Return whether two tips have the same traversal role for deduplication.
///
/// [`TipRole::TargetRemote`] is special because named and anonymous target
/// remotes with the same commit can have different responsibilities. A named
/// target remote represents a ref that may need its own segment,
/// metadata-derived target identity, and target/local sibling link. An
/// anonymous target remote represents commit-only target context, such as
/// `extra_target_commit_id` or a persisted target commit. Validation accepts
/// those two forms so callers can pass metadata-equivalent tips directly;
/// normalization later collapses the anonymous form into the named tip if they
/// point to the same commit.
fn tips_have_same_seed_role(previous: &Tip, tip: &Tip) -> bool {
    match (&previous.role, &tip.role) {
        (TipRole::TargetRemote, TipRole::TargetRemote) => {
            previous.ref_name.is_some() == tip.ref_name.is_some()
        }
        _ => previous.role == tip.role,
    }
}

/// Build auxiliary traversal inputs from normalized tips.
fn initial_tips_from_tips(
    repo: &OverlayRepo<'_>,
    mut tips: Vec<Tip>,
    project_meta: &ProjectMeta,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> InitialTips {
    let mut auxiliary_integrated_tip_ids = BTreeSet::new();
    if let Some(extra_target) = extra_target_commit_id {
        auxiliary_integrated_tip_ids.insert(extra_target);
        push_integrated_tip_once(&mut tips, extra_target);
    }
    let frontload_workspace_related_tips = has_workspace_related_tips(&tips);
    if frontload_workspace_related_tips {
        auxiliary_integrated_tip_ids.extend(tips.iter().filter_map(|tip| {
            tip.is_anonymous_integrated_target_context()
                .then_some(tip.id)
        }));
    }
    collapse_anonymous_integrated_tips_into_named_targets(&mut tips);
    let tips = tips_in_queue_order(tips, &auxiliary_integrated_tip_ids);
    let workspace_tips = tips
        .iter()
        .filter(|tip| matches!(tip.role, TipRole::Workspace))
        .map(|tip| tip.id)
        .collect();
    let workspace_ref_names = tips
        .iter()
        .filter(|tip| matches!(tip.role, TipRole::Workspace))
        .filter_map(|tip| tip.ref_name.clone())
        .collect();
    let include_tip_refs = !tips
        .iter()
        .any(|tip| matches!(tip.metadata, Some(SegmentMetadata::Workspace(_))));
    let target_refs = target_refs_from_tips(&tips, project_meta, include_tip_refs);
    let symbolic_remote_names =
        symbolic_remote_names_from_tips(repo, &tips, project_meta, include_tip_refs);
    let target_local_links = target_local_links_from_tips(repo, &tips);

    InitialTips {
        tips,
        workspace_tips,
        workspace_ref_names,
        target_refs,
        symbolic_remote_names,
        frontload_workspace_related_tips,
        target_local_links,
        auxiliary_integrated_tip_ids,
    }
}

/// Remove anonymous integrated target tips that point to the same commit as a
/// named integrated target.
///
/// Workspace projection derives target context from target-remote tips by graph
/// position, so a same-commit anonymous target does not contribute anything
/// once a named target ref covers that commit. Collapsing here keeps one
/// effective traversal seed and lets the named target segment own the commit.
fn collapse_anonymous_integrated_tips_into_named_targets(tips: &mut Vec<Tip>) {
    let named_integrated_target_ids = tips
        .iter()
        .filter_map(|tip| {
            (matches!(tip.role, TipRole::TargetRemote) && tip.ref_name.is_some()).then_some(tip.id)
        })
        .collect::<BTreeSet<_>>();
    tips.retain(|tip| !tip.collapses_into_named_integrated_target(&named_integrated_target_ids));
}

/// Convert validated tips into deterministic initial traversal roots.
///
/// The caller can provide explicit tips in any order, but queue order still
/// matters because the first item that reaches a commit owns the segment for
/// that commit. This function recreates the ordering that metadata-derived
/// traversal would have produced for workspace tips, while keeping the simpler
/// historical ordering for plain commit traversal.
///
/// The sort is intentionally heuristic: role priority establishes the broad
/// traversal shape, workspace metadata restores stack/branch order when it is
/// available, and stable tie-breakers make equivalent inputs independent of
/// caller order. For non-workspace traversals, equal-priority tips keep caller
/// order so existing explicit traversal behavior stays predictable.
fn tips_in_queue_order(
    tips: Vec<Tip>,
    auxiliary_integrated_tip_ids: &BTreeSet<gix::ObjectId>,
) -> Vec<Tip> {
    let has_workspace_related_tips = has_workspace_related_tips(&tips);
    let workspace_branch_order = workspace_branch_order_from_tips(&tips);
    let mut tips: Vec<_> = tips.into_iter().enumerate().collect();
    tips.sort_by(|(a_idx, a), (b_idx, b)| {
        tip_queue_priority(a, has_workspace_related_tips, auxiliary_integrated_tip_ids)
            .cmp(&tip_queue_priority(
                b,
                has_workspace_related_tips,
                auxiliary_integrated_tip_ids,
            ))
            .then_with(|| {
                tip_workspace_branch_order(a, &workspace_branch_order)
                    .cmp(&tip_workspace_branch_order(b, &workspace_branch_order))
            })
            .then_with(|| {
                if has_workspace_related_tips {
                    tip_sort_name(a).cmp(&tip_sort_name(b))
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| {
                if has_workspace_related_tips {
                    a.id.cmp(&b.id)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| a_idx.cmp(b_idx))
    });
    tips.into_iter().map(|(_, tip)| tip).collect()
}

/// Return whether tip ordering has to emulate workspace metadata traversal.
///
/// Workspace, workspace-stack, and target-local tips are not just additional
/// roots. Their relative order influences which segment owns a shared commit
/// and how the projection reconstructs virtual workspace and stack segments.
/// Detecting such tips switches sorting from "mostly preserve caller order" to
/// "rebuild the metadata order deterministically".
fn has_workspace_related_tips(tips: &[Tip]) -> bool {
    tips.iter().any(|tip| {
        matches!(
            tip.role,
            TipRole::Workspace | TipRole::TargetLocal { .. } | TipRole::WorkspaceStackBranch { .. }
        ) || matches!(tip.metadata, Some(SegmentMetadata::Workspace(_)))
    })
}

/// Primary sort key for initial tips.
///
/// This is the main heuristic. For workspace-related traversals we recreate
/// the metadata-derived segment creation order:
///
/// 1. A non-workspace reachable entrypoint first, if there is one.
/// 2. The workspace ref so it can become the traversal anchor.
/// 3. The integrated target ref, then its local tracking branch, so they can
///    be linked as siblings and agree on target ownership.
/// 4. Synthetic integrated targets, like extra target commits.
/// 5. Workspace stack branches, whose order is refined later from workspace
///    metadata.
/// 6. Other reachable roots.
///
/// For non-workspace traversals there is no metadata order to recover, so
/// integrated context still comes first, non-entry reachable roots follow, and
/// the entrypoint anchors the graph last. Synthetic integrated tips remain
/// last because they are auxiliary limits, not primary user roots.
fn tip_queue_priority(
    tip: &Tip,
    has_workspace_related_tips: bool,
    auxiliary_integrated_tip_ids: &BTreeSet<gix::ObjectId>,
) -> usize {
    if has_workspace_related_tips {
        match &tip.role {
            TipRole::Reachable if tip.is_entrypoint => 0,
            TipRole::Workspace => 1,
            TipRole::TargetRemote if tip.ref_name.is_some() => 2,
            TipRole::TargetLocal { .. } => 3,
            TipRole::TargetRemote
                if tip.is_auxiliary_integrated_tip(auxiliary_integrated_tip_ids) =>
            {
                4
            }
            TipRole::TargetRemote => 2,
            TipRole::WorkspaceStackBranch { .. } => 5,
            TipRole::Reachable => 6,
        }
    } else {
        match &tip.role {
            TipRole::TargetRemote
                if tip.is_auxiliary_integrated_tip(auxiliary_integrated_tip_ids) =>
            {
                3
            }
            TipRole::TargetRemote => 0,
            TipRole::TargetLocal { .. } => 0,
            TipRole::Reachable | TipRole::Workspace | TipRole::WorkspaceStackBranch { .. } => {
                if tip.is_entrypoint { 2 } else { 1 }
            }
        }
    }
}

/// Recover stack-branch order from workspace metadata.
///
/// Workspace metadata stores the user-visible ordering of workspaces, stacks,
/// and branches. When explicit tips are equivalent to metadata-derived tips,
/// this order is the only reliable way to make scrambled input produce the same
/// graph and workspace projection as `from_commit_traversal()`.
///
/// The return value maps a branch ref name to the position where that branch
/// appears in workspace metadata. The value tuple is
/// `(workspace_order, stack_order, branch_order)`:
///
/// - `workspace_order` is the index of the workspace metadata tip after all
///   workspace metadata tips have been sorted by their optional ref name. This
///   makes multi-workspace input deterministic even when the caller provided
///   tips in a different order.
/// - `stack_order` is the zero-based index among stacks that are currently in
///   the workspace. Archived or otherwise inactive stacks are ignored and don't
///   consume an order slot.
/// - `branch_order` is the zero-based index of the branch within that stack's
///   branch list.
///
/// Branch refs not found in this map have no metadata-derived order and fall
/// back to later tie-breakers. If the same branch ref appears more than once,
/// the first metadata occurrence wins, matching the "first configured stack
/// owns the branch" behavior expected by workspace projection.
fn workspace_branch_order_from_tips(
    tips: &[Tip],
) -> BTreeMap<gix::refs::FullName, (usize, usize, usize)> {
    let mut workspaces: Vec<_> = tips
        .iter()
        .filter_map(|tip| match tip.metadata.as_ref() {
            Some(SegmentMetadata::Workspace(data)) => Some((tip.ref_name.as_ref(), data)),
            Some(SegmentMetadata::Branch(_)) | None => None,
        })
        .collect();
    workspaces.sort_by_key(|(ref_name, _)| *ref_name);

    let mut out = BTreeMap::new();
    for (workspace_order, (_ref_name, data)) in workspaces.into_iter().enumerate() {
        for (stack_order, stack) in data
            .stacks
            .iter()
            .filter(|stack| stack.is_in_workspace())
            .enumerate()
        {
            for (branch_order, branch) in stack.branches.iter().enumerate() {
                out.entry(branch.ref_name.clone()).or_insert((
                    workspace_order,
                    stack_order,
                    branch_order,
                ));
            }
        }
    }
    out
}

/// Return the metadata order for a workspace stack branch tip.
///
/// Only `WorkspaceStackBranch` tips participate in this secondary ordering.
/// Other roles intentionally return `None` so their relative order is governed
/// by the primary role priority and later tie-breakers.
fn tip_workspace_branch_order(
    tip: &Tip,
    workspace_branch_order: &BTreeMap<gix::refs::FullName, (usize, usize, usize)>,
) -> Option<(usize, usize, usize)> {
    match &tip.role {
        TipRole::WorkspaceStackBranch { desired_ref_name } => {
            workspace_branch_order.get(desired_ref_name).copied()
        }
        TipRole::Reachable
        | TipRole::Workspace
        | TipRole::TargetRemote
        | TipRole::TargetLocal { .. } => None,
    }
}

/// Stable name tie-breaker used only in workspace-related sorting.
///
/// After role priority and metadata branch order, tips may still be equivalent
/// from the traversal's point of view. Sorting by the ref that will name or
/// identify the segment keeps explicit workspace-tip input order irrelevant.
/// For non-workspace traversals this helper is deliberately ignored so equal
/// priorities preserve the caller's order instead.
fn tip_sort_name(tip: &Tip) -> Option<String> {
    match &tip.role {
        TipRole::WorkspaceStackBranch { desired_ref_name } => {
            Some(desired_ref_name.as_bstr().to_string())
        }
        TipRole::TargetLocal { local_ref_name } => Some(local_ref_name.as_bstr().to_string()),
        TipRole::Reachable | TipRole::Workspace | TipRole::TargetRemote => {
            tip.ref_name.as_ref().map(|ref_name| ref_name.to_string())
        }
    }
}

/// Discover workspaces, targets, local tracking branches, and workspace stack
/// branch refs and turn them into initial traversal tips.
pub(crate) fn initial_tips_from_workspace_metadata<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    project_meta: &ProjectMeta,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> anyhow::Result<Vec<Tip>> {
    let workspaces = obtain_workspace_infos(repo, entrypoint_ref.map(|rn| rn.as_ref()), meta)?;
    let tip_ref_matches_ws_ref = workspaces
        .iter()
        .find_map(|(ws_tip, ws_rn, _)| (Some(ws_rn) == entrypoint_ref).then_some(ws_tip));

    let mut tips = Vec::new();
    let mut workspace_metas = Vec::new();
    let mut additional_target_commits = Vec::new();
    let mut queued_ids = Vec::new();

    match tip_ref_matches_ws_ref {
        None => {
            // We don't name the tip of the entrypoint as we want the segment
            // naming to be handled by tips created from metadata.
            tips.push(Tip::entrypoint(entrypoint, None));
            queued_ids.push(entrypoint);
        }
        Some(ws_tip) => {
            ensure!(
                *ws_tip == entrypoint,
                format!(
                    "BUG:: {entrypoint_ref:?} points to {ws_tip}, but the caller claimed it points to {entrypoint}"
                )
            );
        }
    }

    for (ws_tip, ws_ref, ws_meta) in workspaces {
        workspace_metas.push(ws_meta.clone());
        additional_target_commits.extend(project_meta.target_commit_id);
        tips.push(
            Tip::new(ws_tip)
                .with_ref_name(Some(ws_ref.clone()))
                .with_role(TipRole::Workspace)
                .with_metadata(SegmentMetadata::Workspace(ws_meta.clone()))
                .with_is_entrypoint(Some(&ws_ref) == entrypoint_ref),
        );

        let target = if let Some((target_ref, target_ref_id, local_info)) =
            workspace_target_tip(repo, project_meta.target_ref.as_ref())?
        {
            let local_info =
                local_info.filter(|(_local_ref_name, local_tip)| !queued_ids.contains(local_tip));
            tips.push(
                Tip::new(target_ref_id)
                    .with_ref_name(Some(target_ref))
                    .with_role(TipRole::TargetRemote),
            );
            if let Some((local_ref_name, local_tip)) = local_info.clone() {
                tips.push(Tip::new(local_tip).with_role(TipRole::TargetLocal { local_ref_name }));
            }
            Some((
                target_ref_id,
                local_info.map(|(_local_ref_name, local_tip)| local_tip),
            ))
        } else {
            None
        };
        queued_ids.push(ws_tip);
        if let Some((target_ref_id, local_tip)) = target {
            queued_ids.push(target_ref_id);
            if let Some(local_tip) = local_tip {
                queued_ids.push(local_tip);
            }
        }
    }

    if let Some(extra_target) = extra_target_commit_id {
        push_integrated_tip_once(&mut tips, extra_target);
    }

    for target_commit_id in additional_target_commits {
        // These are possibly from metadata, and thus might not exist (anymore). Ignore if that's the case.
        if let Err(err) = repo.find_commit(target_commit_id) {
            tracing::warn!(
                ?target_commit_id,
                ?err,
                "Ignoring stale target commit id as it didn't exist"
            );
            continue;
        }
        // We don't really have a place to store the segment index of the segment owning the target commit
        // so we will re-acquire it later when building the workspace projection.
        push_integrated_tip_once(&mut tips, target_commit_id);
    }

    // Queue workspace stack branch refs that may have advanced since the
    // workspace commit was written, and thus would not be reached from that
    // commit alone.
    for ws_metadata in workspace_metas {
        for segment in ws_metadata
            .stacks
            .into_iter()
            .filter(|s| s.is_in_workspace())
            .flat_map(|s| s.branches.into_iter())
        {
            let Some(segment_tip) = repo
                .try_find_reference(segment.ref_name.as_ref())?
                .map(|mut r| r.peel_to_id())
                .transpose()?
            else {
                continue;
            };
            push_tip_once(
                &mut tips,
                Tip::new(segment_tip.detach()).with_role(TipRole::WorkspaceStackBranch {
                    desired_ref_name: segment.ref_name,
                }),
            );
        }
    }

    Ok(tips)
}

fn push_integrated_tip_once(tips: &mut Vec<Tip>, id: gix::ObjectId) {
    let tip = Tip::new(id).with_role(TipRole::TargetRemote);
    push_tip_once(tips, tip);
}

fn push_tip_once(tips: &mut Vec<Tip>, tip: Tip) {
    if !tips
        .iter()
        .any(|existing| tips_have_same_traversal_seed(existing, &tip))
    {
        tips.push(tip);
    }
}

/// Resolve a workspace target ref and, when possible, its local tracking branch
/// tip.
fn workspace_target_tip(
    repo: &OverlayRepo<'_>,
    target_ref: Option<&gix::refs::FullName>,
) -> anyhow::Result<Option<WorkspaceTargetTip>> {
    let Some(target_ref) = target_ref else {
        return Ok(None);
    };
    let target_ref_id = match try_refname_to_id(repo, target_ref.as_ref()).map_err(|err| {
        tracing::warn!("Ignoring non-existing target branch {target_ref}: {err}");
        err
    }) {
        Ok(Some(target_ref_id)) => target_ref_id,
        Ok(None) | Err(_) => return Ok(None),
    };
    let local_info = repo
        .upstream_branch_and_remote_for_tracking_branch(target_ref.as_ref())
        .ok()
        .flatten()
        .and_then(|(local_tracking_name, _remote_name)| {
            let target_local_tip = try_refname_to_id(repo, local_tracking_name.as_ref()).ok()??;
            Some((local_tracking_name, target_local_tip))
        });
    Ok(Some((target_ref.clone(), target_ref_id, local_info)))
}

/// Return remote target refs that are already represented by initial tips.
///
/// The result is passed to remote-tracking discovery so it does not queue a
/// target ref a second time when walking a local branch that tracks it.
/// Workspace traversals get this from the project metadata target ref, which
/// is where their target lives now. Explicit traversals have no workspace
/// discovery source, so named integrated tips may also act as target refs
/// when `include_integrated_tip_refs` is set.
fn target_refs_from_tips(
    tips: &[Tip],
    project_meta: &ProjectMeta,
    include_integrated_tip_refs: bool,
) -> Vec<gix::refs::FullName> {
    let has_workspace_metadata_tip = tips
        .iter()
        .any(|tip| matches!(tip.metadata, Some(SegmentMetadata::Workspace(_))));
    let mut target_refs: Vec<_> = tips
        .iter()
        .filter(|tip| include_integrated_tip_refs && tip.role.is_integrated())
        .filter_map(|tip| tip.ref_name.clone())
        .chain(
            has_workspace_metadata_tip
                .then(|| project_meta.target_ref.clone())
                .flatten(),
        )
        .collect();
    target_refs.sort();
    target_refs.dedup();
    target_refs
}

/// Infer target remote/local tracking links without exposing correlation ids on
/// public tips.
///
/// The target side is represented by a named [`TipRole::TargetRemote`] tip. The
/// local side is represented by a [`TipRole::TargetLocal`] tip whose
/// `local_ref_name` matches the local branch configured to track that remote
/// target ref. If either side is absent, the tips still participate in
/// traversal but no sibling link is prepared up front.
fn target_local_links_from_tips(repo: &OverlayRepo<'_>, tips: &[Tip]) -> TargetLocalLinks {
    let remote_target_refs: Vec<_> = tips
        .iter()
        .filter(|tip| matches!(tip.role, TipRole::TargetRemote))
        .filter_map(|tip| tip.ref_name.clone())
        .collect();
    let local_refs: BTreeSet<_> = tips
        .iter()
        .filter_map(|tip| match &tip.role {
            TipRole::TargetLocal { local_ref_name } => Some(local_ref_name.clone()),
            TipRole::Reachable
            | TipRole::Workspace
            | TipRole::WorkspaceStackBranch { .. }
            | TipRole::TargetRemote => None,
        })
        .collect();

    let mut links = TargetLocalLinks::default();
    for target_ref in remote_target_refs {
        let Some((local_ref, _remote_name)) = repo
            .upstream_branch_and_remote_for_tracking_branch(target_ref.as_ref())
            .ok()
            .flatten()
        else {
            continue;
        };
        if !local_refs.contains(&local_ref) {
            continue;
        }
        links
            .local_by_target
            .insert(target_ref.clone(), local_ref.clone());
        links.target_by_local.insert(local_ref, target_ref);
    }
    links
}

/// Collect symbolic remote names implied by tip refs, workspace target refs,
/// workspace `push_remote` settings, and stack branch refs.
fn symbolic_remote_names_from_tips(
    repo: &OverlayRepo<'_>,
    tips: &[Tip],
    project_meta: &ProjectMeta,
    include_tip_refs: bool,
) -> Vec<String> {
    let remote_names = repo.remote_names();
    let refs = tips
        .iter()
        .filter_map(|tip| include_tip_refs.then_some(tip.ref_name.as_ref()).flatten())
        .filter_map({
            let remote_names = &remote_names;
            move |ref_name| {
                extract_remote_name_and_short_name(ref_name.as_ref(), remote_names)
                    .map(|(remote, _short_name)| (1, remote))
            }
        });
    let workspace_metadata_names = tips
        .iter()
        .filter_map(|tip| match tip.metadata.as_ref() {
            Some(SegmentMetadata::Workspace(data)) => Some(data),
            Some(SegmentMetadata::Branch(_)) | None => None,
        })
        .flat_map(|data| {
            data.stacks.iter().flat_map(|s| {
                s.branches.iter().flat_map(|b| {
                    extract_remote_name_and_short_name(b.ref_name.as_ref(), &remote_names)
                        .map(|(remote, _short_name)| (1, remote))
                })
            })
        });
    let desired_refs = tips.iter().filter_map(|tip| match &tip.role {
        _ if !include_tip_refs => None,
        TipRole::WorkspaceStackBranch { desired_ref_name } => {
            extract_remote_name_and_short_name(desired_ref_name.as_ref(), &remote_names)
                .map(|(remote, _short_name)| (1, remote))
        }
        TipRole::Reachable
        | TipRole::Workspace
        | TipRole::TargetLocal { .. }
        | TipRole::TargetRemote => None,
    });
    let target_ref = project_meta.target_ref.as_ref().and_then(|target_ref| {
        extract_remote_name_and_short_name(target_ref.as_ref(), &remote_names)
            .map(|(remote, _short_name)| (1, remote))
    });
    let push_remote = project_meta
        .push_remote
        .as_ref()
        .map(|push_remote| (0, push_remote.clone()));
    sorted_symbolic_remote_names(
        refs.chain(workspace_metadata_names)
            .chain(desired_refs)
            .chain(target_ref)
            .chain(push_remote),
    )
}

/// Sort and deduplicate remote names, preserving explicit push remotes before
/// remotes inferred from refs with the same name.
fn sorted_symbolic_remote_names(names: impl Iterator<Item = (usize, String)>) -> Vec<String> {
    let mut names: Vec<_> = names.collect();
    names.sort();
    names.dedup();
    names.into_iter().map(|(_order, remote)| remote).collect()
}

/// Insert initial segments, seed the traversal queue, and return workspace
/// ownership roots for the projection.
#[expect(clippy::too_many_arguments)]
fn queue_initial_tips<T: RefMetadata>(
    state: &mut commit_walk::State,
    next: &mut Queue,
    initial_tips: &InitialTips,
    entrypoint: gix::ObjectId,
    entrypoint_flags: CommitFlags,
    max_limit: Limit,
    target_limit: Limit,
    goals: &mut Goals,
    commit_graph: Option<&gix::commitgraph::Graph>,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    ctx: &commit_walk::Context<'_>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Vec<usize>> {
    // `target_local_segments` holds the local side once its segment and goal
    // exist. `pending_integrated_tips` holds the remote target side if it
    // appears first. Both maps are keyed by target remote ref names inferred
    // from tip refs and repository branch configuration.
    let mut target_local_segments =
        BTreeMap::<gix::refs::FullName, (Option<usize>, CommitFlags)>::new();
    let mut pending_integrated_tips = BTreeMap::<gix::refs::FullName, PendingIntegratedTip>::new();

    for tip in &initial_tips.tips {
        match &tip.role {
            TipRole::WorkspaceStackBranch { .. } if next.iter().any(|t| t.0.id == tip.id) => {
                next.add_goal_to(tip.id, goals.flag_for(entrypoint).unwrap_or_default());
                continue;
            }
            TipRole::TargetRemote
                if tip.is_auxiliary_integrated_tip(&initial_tips.auxiliary_integrated_tip_ids)
                    && next.iter().any(|(info, _, _, _)| info.id == tip.id) =>
            {
                continue;
            }
            _ => {}
        }

        let (mut ref_info, mut metadata) = branch_segment_from_name_and_meta(
            tip.ref_name
                .clone()
                .map(|ref_name| (ref_name, tip.metadata.clone())),
            meta,
            Some((&ctx.refs_by_id, tip.id)),
            &ctx.worktree_by_branch,
        )?;
        if let TipRole::WorkspaceStackBranch { desired_ref_name } = &tip.role {
            let is_remote = desired_ref_name
                .category()
                .is_some_and(|c| c == Category::RemoteBranch);
            if ref_info.is_none() && is_remote {
                ref_info = Some(crate::RefInfo::from_ref(
                    desired_ref_name.clone(),
                    tip.id,
                    &ctx.worktree_by_branch,
                ));
                metadata = meta
                    .branch_opt(desired_ref_name.as_ref())?
                    .map(SegmentMetadata::Branch);
            }
        }
        let segment = state.insert_recording((ref_info, metadata));
        if let TipRole::TargetRemote = &tip.role {
            let pending = PendingIntegratedTip {
                id: tip.id,
                segment,
                queue_front: queue_should_frontload_tip(
                    tip,
                    initial_tips.frontload_workspace_related_tips,
                    &initial_tips.auxiliary_integrated_tip_ids,
                ),
            };
            if let Some(target_ref) = tip
                .ref_name
                .as_ref()
                .filter(|ref_name| {
                    initial_tips
                        .target_local_links
                        .local_by_target
                        .contains_key(*ref_name)
                })
                .cloned()
            {
                let Some(local) = target_local_segments.get(&target_ref).copied() else {
                    pending_integrated_tips.insert(target_ref, pending);
                    continue;
                };
                queue_pending_integrated_tip(
                    next,
                    pending,
                    local,
                    target_limit,
                    commit_graph,
                    repo,
                    buf,
                )?;
            } else {
                queue_pending_integrated_tip(
                    next,
                    pending,
                    (None, CommitFlags::empty()),
                    target_limit,
                    commit_graph,
                    repo,
                    buf,
                )?;
            }
            continue;
        }

        let (flags, limit) = match &tip.role {
            TipRole::Reachable if tip.is_entrypoint => {
                state.entrypoint = Some(segment);
                (entrypoint_flags, max_limit)
            }
            TipRole::Reachable => {
                reachable_tip_flags_and_limit(tip.id, entrypoint, max_limit, goals)
            }
            TipRole::TargetRemote => unreachable!("handled above"),
            TipRole::Workspace => {
                if tip.is_entrypoint && state.entrypoint.is_none() {
                    state.entrypoint = Some(segment);
                }
                let extra_flags = if tip.is_entrypoint {
                    entrypoint_flags
                } else {
                    CommitFlags::empty()
                };
                let limit = if tip.is_entrypoint {
                    max_limit
                } else {
                    max_limit.with_indirect_goal(entrypoint, goals)
                };
                (
                    CommitFlags::InWorkspace | CommitFlags::NotInRemote | extra_flags,
                    limit,
                )
            }
            TipRole::TargetLocal { local_ref_name } => {
                let has_remote_link = state
                    .seg_ref_name(segment)
                    .is_some_and(|ref_name| ref_name == local_ref_name.as_ref());
                let goal = goals.flag_for(tip.id).unwrap_or_default();
                if let Some(target_ref) = initial_tips
                    .target_local_links
                    .target_by_local
                    .get(local_ref_name)
                {
                    target_local_segments.insert(
                        target_ref.clone(),
                        (has_remote_link.then_some(segment), goal),
                    );
                }
                next.add_goal_to(entrypoint, goal);
                (CommitFlags::NotInRemote | goal, target_limit)
            }
            TipRole::WorkspaceStackBranch { .. } => (
                CommitFlags::NotInRemote,
                max_limit.with_indirect_goal(entrypoint, goals),
            ),
        };
        let tip_info = find(commit_graph, repo.for_find_only(), tip.id, buf)?;
        let item = (tip_info, flags, Step::SeedTip { into: segment }, limit);
        // A target ref and its local tracking branch can point at the same
        // commit. In that case, the integrated target was held back only until
        // the local side created its segment and goal above. Queue the
        // integrated item before pushing the current local item so the shared
        // commit is owned as integrated history while still carrying the local
        // goal that lets both sides connect.
        let paired_target_ref = match &tip.role {
            TipRole::TargetLocal { local_ref_name } => initial_tips
                .target_local_links
                .target_by_local
                .get(local_ref_name)
                .cloned(),
            TipRole::Reachable
            | TipRole::Workspace
            | TipRole::WorkspaceStackBranch { .. }
            | TipRole::TargetRemote => None,
        };
        let pending_before_current = paired_target_ref.as_ref().and_then(|target_ref| {
            pending_integrated_tips
                .get(target_ref)
                .is_some_and(|pending| pending.id == tip.id)
                .then(|| pending_integrated_tips.remove(target_ref))
                .flatten()
        });
        if let Some(pending) = pending_before_current {
            let local = paired_target_ref
                .as_ref()
                .and_then(|target_ref| target_local_segments.get(target_ref))
                .copied()
                .unwrap_or((None, CommitFlags::empty()));
            queue_pending_integrated_tip(
                next,
                pending,
                local,
                target_limit,
                commit_graph,
                repo,
                buf,
            )?;
        }
        if queue_should_frontload_tip(
            tip,
            initial_tips.frontload_workspace_related_tips,
            &initial_tips.auxiliary_integrated_tip_ids,
        ) {
            _ = next.push_front_exhausted(item);
        } else {
            _ = next.push_back_exhausted(item);
        }

        if let Some(target_ref) = paired_target_ref
            && let Some(pending) = pending_integrated_tips.remove(&target_ref)
        {
            let local = target_local_segments
                .get(&target_ref)
                .copied()
                .unwrap_or((None, CommitFlags::empty()));
            queue_pending_integrated_tip(
                next,
                pending,
                local,
                target_limit,
                commit_graph,
                repo,
                buf,
            )?;
        }
    }

    prioritize_initial_tips_and_assure_ws_commit_ownership(
        state,
        next,
        (initial_tips.workspace_tips.clone(), repo, meta),
        &ctx.worktree_by_branch,
    )
}

/// Queue an integrated target after optionally linking it to its local tracking segment.
fn queue_pending_integrated_tip(
    next: &mut Queue,
    pending: PendingIntegratedTip,
    local: (Option<usize>, CommitFlags),
    target_limit: Limit,
    commit_graph: Option<&gix::commitgraph::Graph>,
    repo: &OverlayRepo<'_>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    // The local<->remote pairing the record graph stored here (sibling_segment_id /
    // remote_tracking_branch_segment_id) is never read by the projection, which re-derives it from
    // facts in `enrich_with_remotes`; so only the goal flag is needed.
    let (_local_sidx, local_goal) = local;
    let tip_info = find(commit_graph, repo.for_find_only(), pending.id, buf)?;
    let item = (
        tip_info,
        CommitFlags::Integrated,
        Step::SeedTip {
            into: pending.segment,
        },
        target_limit.additional_goal(local_goal),
    );
    if pending.queue_front {
        _ = next.push_front_exhausted(item);
    } else {
        _ = next.push_back_exhausted(item);
    }
    Ok(())
}

/// Return whether an initial queue item should be pushed to the front.
///
/// This is the second half of the ordering heuristic. `tips_in_queue_order()`
/// decides the order in which initial segments are created. Once those segments
/// are converted into traversal queue items, some roles must still be
/// front-loaded so their commits are visited before ordinary reachable or stack
/// branch work that may point at the same commits so they can own them.
///
/// Synthetic integrated tips are always front-loaded because they represent
/// additional target/limit commits rather than user-visible branch roots. For
/// workspace-related traversals, workspace, integrated target, and target-local
/// tips are also front-loaded so target ownership and target/local sibling
/// links are established before stack-branch traversal can claim shared commits.
/// Workspace stack branches are deliberately not front-loaded: their segment
/// creation order is recovered from metadata, but their traversal work should
/// follow the workspace/target context.
fn queue_should_frontload_tip(
    tip: &Tip,
    frontload_workspace_related_tips: bool,
    auxiliary_integrated_tip_ids: &BTreeSet<gix::ObjectId>,
) -> bool {
    tip.is_auxiliary_integrated_tip(auxiliary_integrated_tip_ids)
        || (frontload_workspace_related_tips
            && matches!(
                tip.role,
                TipRole::Workspace | TipRole::TargetRemote | TipRole::TargetLocal { .. }
            ))
}

/// Return the flags and limit used by a reachable tip seeking the entrypoint.
fn reachable_tip_flags_and_limit(
    tip: gix::ObjectId,
    entrypoint: gix::ObjectId,
    max_limit: Limit,
    goals: &mut Goals,
) -> (CommitFlags, Limit) {
    let limit = if tip == entrypoint {
        max_limit
    } else {
        max_limit.with_indirect_goal(entrypoint, goals)
    };
    (CommitFlags::NotInRemote, limit)
}
