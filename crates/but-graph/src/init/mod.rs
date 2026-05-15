use std::collections::BTreeMap;

use anyhow::{Context as _, bail, ensure};
use bstr::ByteSlice;
use but_core::{RefMetadata, extract_remote_name_and_short_name, ref_metadata};
use gix::{
    hashtable::hash_map::Entry,
    prelude::{ObjectIdExt, ReferenceExt},
    refs::Category,
};
use petgraph::Direction;
use tracing::instrument;

use crate::{
    CommitFlags, CommitIndex, Edge, EntryPointCommit, Graph, Segment, SegmentIndex, SegmentMetadata,
};

mod walk;
use walk::*;

pub(crate) mod types;
use types::{EdgeOwned, Goals, Instruction, Limit, Queue};

use crate::init::overlay::{OverlayMetadata, OverlayRepo};

mod remotes;

mod overlay;
mod post;

pub(crate) type Entrypoint = Option<(gix::ObjectId, Option<gix::refs::FullName>)>;

/// A resolved commit tip to seed graph traversal without requiring it to be
/// discoverable through repository refs or workspace metadata.
#[derive(Debug, Clone)]
pub struct Tip {
    /// The commit id to start walking from.
    pub id: gix::ObjectId,
    /// The ref name to assign to the tip segment, if it should be named.
    pub ref_name: Option<gix::refs::FullName>,
    /// How this tip participates in traversal.
    pub role: TipRole,
}

/// Lifecycle
impl Tip {
    /// A normal named or unnamed traversal entrypoint.
    ///
    /// `id` is the commit where graph traversal starts.
    /// `ref_name` names the entrypoint segment when the caller has a stable ref
    /// for it.
    pub fn entrypoint(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Tip {
            id,
            ref_name,
            role: TipRole::EntryPoint,
        }
    }

    /// An entrypoint whose segment should remain detached even if refs point to
    /// its commit.
    ///
    /// `id` is the commit where graph traversal starts.
    pub fn detached_entrypoint(id: gix::ObjectId) -> Self {
        Tip {
            id,
            ref_name: None,
            role: TipRole::DetachedEntryPoint,
        }
    }

    /// A non-remote tip that should be included in the traversal.
    ///
    /// `id` is the commit to include as another non-remote traversal root.
    /// `ref_name` names the tip segment when the caller has a stable ref for it.
    pub fn reachable(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Tip {
            id,
            ref_name,
            role: TipRole::Reachable,
        }
    }

    /// A target/integration tip that bounds or extends traversal context.
    /// It represents part of the graph that [`Self::reachable()`] parts want to integrate with.
    ///
    /// `id` is the commit to treat as integrated history.
    /// `ref_name` names the target segment when the caller has a stable ref for
    /// it.
    pub fn integrated(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Tip {
            id,
            ref_name,
            role: TipRole::Integrated,
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipRole {
    /// The normal traversal entrypoint, typically used to represent `HEAD`.
    ///
    /// This is the single anchor tip for explicit traversal. It receives
    /// [`CommitFlags::NotInRemote`] plus a generated goal flag for its own
    /// commit, and that goal flag propagates down its ancestry so other tips can
    /// connect to it.
    ///
    /// It does not seek another tip. [`TipRole::Reachable`] and
    /// [`TipRole::Integrated`] tips seek connection to this role.
    EntryPoint,
    /// The traversal entrypoint, but with detached-HEAD presentation semantics.
    ///
    /// This behaves like [`TipRole::EntryPoint`] for traversal and is also the
    /// single anchor that reachable and integrated tips seek. Its segment is
    /// kept anonymous even if repository refs point to the same commit; those
    /// refs remain attached to the commit instead of naming the segment.
    DetachedEntryPoint,
    /// A non-remote tip that should be traversed and related to the entrypoint.
    ///
    /// This tip receives [`CommitFlags::NotInRemote`] and, unless it is the same
    /// commit as the entrypoint, an indirect goal for the entrypoint commit. It
    /// seeks connection to [`TipRole::EntryPoint`] or [`TipRole::DetachedEntryPoint`] by
    /// walking history until it reaches commits carrying the entrypoint's
    /// propagated goal flag.
    Reachable,
    /// A target/integration tip whose reachable history is considered integrated,
    /// and that reachable/unintegrated tips want to connect with.
    ///
    /// This tip receives [`CommitFlags::Integrated`] and an indirect goal for
    /// the entrypoint commit with no extra allowance once that goal is found. It
    /// seeks connection to [`TipRole::EntryPoint`] or
    /// [`TipRole::DetachedEntryPoint`] just far enough to connect target history
    /// to the entrypoint's ancestry.
    Integrated,
}

/// Selects the source used to build [`InitialTips`] before commit traversal
/// starts.
///
/// The rest of graph initialization works from the normalized [`InitialTips`]
/// plan so the traversal queue can be populated in one place,
/// regardless of whether the caller supplied tips explicitly or the tips were
/// discovered from workspace metadata.
enum TipSource {
    /// Discover tips from workspace metadata and repository refs.
    FromMetadata,
    /// Use caller-provided tips directly, bypassing workspace metadata tip
    /// discovery.
    Explicit(Vec<Tip>),
}

/// A workspace reference that has been resolved to the commit it points to and
/// paired with the metadata that describes its stacks and target.
///
/// These are kept separately from [`InitialTip`] because post-queue workspace
/// ownership fixups need all workspace tips as a group, even after each
/// workspace has already been turned into a traversal root.
type WorkspaceTip = (gix::ObjectId, gix::refs::FullName, ref_metadata::Workspace);

/// A local branch ref and the commit it points to, when it tracks a workspace
/// target ref.
type LocalTrackingTip = (gix::refs::FullName, gix::ObjectId);

/// A workspace target ref, its commit, and optionally the local branch tracking it.
type WorkspaceTargetTip = (gix::refs::FullName, gix::ObjectId, Option<LocalTrackingTip>);

/// The complete pre-traversal plan derived from either explicit tips or
/// workspace metadata.
///
/// [`queue_initial_tips()`] consumes this value to create graph segments, seed
/// the traversal queue, and provide the auxiliary ref and remote information
/// needed by traversal and post-processing.
struct InitialTips {
    /// Ordered traversal roots to turn into segments and queue items.
    tips: Vec<InitialTip>,
    /// Workspace commits used to ensure commits remain owned by the workspace
    /// roots that introduced them.
    workspace_tips: Vec<gix::ObjectId>,
    /// Workspace ref names that should be included while collecting refs by
    /// prefix, even when they are not reachable from the entrypoint yet.
    workspace_ref_names: Vec<gix::refs::FullName>,
    /// Remote target refs that were already scheduled as initial integrated
    /// tips.
    ///
    /// Workspace metadata seeds this list from `data.target_ref` while
    /// discovering workspaces. Explicit traversal seeds the same list from
    /// integrated tip ref names. During traversal,
    /// `try_queue_remote_tracking_branches()` uses it to avoid queueing those
    /// target refs again when local branch refs point at them as upstreams.
    target_refs: Vec<gix::refs::FullName>,
    /// Remote names to try when a local branch has no configured upstream.
    ///
    /// `lookup_remote_tracking_branch_or_deduce_it()` first asks Git for the
    /// branch's configured remote-tracking ref. If none exists, it tries each
    /// name here by constructing `refs/remotes/<remote>/<local-short-name>` and
    /// using it only if that ref exists and is not already configured for
    /// another branch.
    symbolic_remote_names: Vec<String>,
}

/// A single commit right before traversal begins.
///
/// Its role determines the flags, goals, and segment relationships assigned
/// when [`queue_initial_tips()`] creates the corresponding queue item.
struct InitialTip {
    /// Commit id to queue as a traversal root.
    id: gix::ObjectId,
    /// Optional ref name used to name the initial segment when the tip is backed
    /// by an unambiguous reference.
    ref_name: Option<gix::refs::FullName>,
    /// Metadata to attach to the initial segment, as extracted from [`RefMetadata`].
    metadata: Option<SegmentMetadata>,
    /// Traversal meaning of this tip. More detailed, richer, than [`TipRole`].
    role: InitialTipRole,
    /// Whether the queue item should be inserted before or after existing
    /// initial work.
    queue_position: InitialTipQueuePosition,
}

/// The traversal role assigned to an [`InitialTip`].
///
/// Roles translate the normalized tip list ([`InitialTips`]) into concrete
/// queue behavior: commit flags, traversal limits, graph entrypoint assignment,
/// and workspace stack branch recovery.
///
/// Target/local sibling links are created for integrated targets with a matching
/// local tracking branch. They connect the local branch segment with its
/// integrated remote target segment so later graph consumers can move between
/// the two sides without searching the graph again.
enum InitialTipRole {
    /// The commit that anchors the graph. Reachable and integrated tips use it
    /// as their connection goal.
    EntryPoint,
    /// A non-remote commit that should be walked until it connects back to the
    /// entrypoint or runs out of relevant history.
    Reachable,
    /// The workspace ref itself, paired with its workspace metadata.
    ///
    /// This marks commits as in-workspace and may also become the graph
    /// entrypoint when traversal starts from the workspace ref.
    Workspace {
        /// Whether this workspace tip is the user-facing traversal entrypoint.
        is_entrypoint: bool,
    },
    /// A branch from a stack listed in workspace metadata.
    ///
    /// This is distinct from [`InitialTipRole::Workspace`]: it is not the
    /// workspace ref, but one of the branch refs the workspace says belongs to a
    /// stack. Its current ref tip should be traversed even if it isn't reachable
    /// from the workspace commit.
    ///
    /// This can happen when a workspace commit records an older branch tip, but
    /// the branch ref later advances, is rebased, or is otherwise moved before
    /// the next workspace commit is written. Git can tell us the branch's
    /// current tip, but traversal is still needed to connect that tip into the
    /// graph and assign ownership/limits along its history.
    WorkspaceStackBranch {
        /// Ref name from workspace metadata to use for segment naming if the
        /// initial segment cannot infer an unambiguous ref from the tip commit.
        desired_ref_name: gix::refs::FullName,
    },
    /// A target commit whose history is considered integrated.
    ///
    /// If this tip has a matching [`InitialTipRole::TargetLocal`], it waits for
    /// that local tracking branch segment and goal flag so the target queue
    /// item can link siblings and search for the local side.
    Integrated {
        /// Key into `target_local_segments` and `pending_integrated_tips` used
        /// to match this target with its [`InitialTipRole::TargetLocal`].
        local_tracking_key: Option<usize>,
        /// Whether to reuse an already queued segment if another initial tip
        /// has the same commit id.
        ///
        /// `true` is used only for additional and metadata target-commit tips
        /// appended after workspace roots, because those commits may already
        /// have been queued by a workspace, target ref, or local tracking tip.
        /// `false` is used for caller-provided integrated tips and workspace
        /// target refs, whose duplicate commits should have been rejected or
        /// filtered before the initial list is queued.
        dedupe_if_queued: bool,
    },
    /// The local branch that tracks an integrated target branch.
    ///
    /// It receives a goal for the target and later provides the segment id that
    /// lets the target segment point back to its local sibling.
    TargetLocal {
        /// Correlation key used to store this local side in
        /// `target_local_segments` and to release any target waiting in
        /// `pending_integrated_tips` under the same key.
        key: usize,
        /// The expected local tracking ref name used to decide whether the new
        /// segment can be linked directly to the target segment.
        local_ref_name: gix::refs::FullName,
    },
}

/// Where to place an initial traversal item relative to already queued work.
///
/// Metadata-derived traversal uses this to preserve existing behavior where
/// workspace and integrated roots are considered before trailing workspace
/// stack branch tips.
#[derive(Clone, Copy)]
enum InitialTipQueuePosition {
    /// Queue before existing initial work.
    Front,
    /// Queue after existing initial work.
    Back,
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
    segment: SegmentIndex,
    /// Queue placement to use once the target is released.
    queue_position: InitialTipQueuePosition,
}

/// A way to define information to be served from memory, instead of from the underlying data source, when
/// [initializing](Graph::from_commit_traversal()) the graph.
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

pub(super) type PetGraph = petgraph::stable_graph::StableGraph<Segment, Edge>;

/// Options for use in [`Graph::from_head()`] and [`Graph::from_commit_traversal()`].
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
    /// As opposed to the limit-hint, if not `None` we will stop after pretty much this many commits have been seen.
    ///
    /// This is a last line of defense against runaway traversals and for not it's recommended to set it to a high
    /// but manageable value. Note that depending on the commit-graph, we may need more commits to find the local branch
    /// for a remote branch, leaving remote branches unconnected.
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
    /// Enabling this will prevent the postprocessing step to run which is what makes the graph useful through clean-up
    /// and to make it more amenable to a workspace project.
    ///
    /// This should only be used in case post-processing fails and one wants to preview the version before that.
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
    /// interest and re-resolved after post-processing so workspace projection
    /// can use it as a past target/base candidate.
    pub fn with_extra_target_commit_id(mut self, id: impl Into<gix::ObjectId>) -> Self {
        self.extra_target_commit_id = Some(id.into());
        self
    }
}

/// Lifecycle
impl Graph {
    /// Read the `HEAD` of `repo` and represent whatever is visible as a graph.
    ///
    /// See [`Self::from_commit_traversal()`] for details.
    pub fn from_head(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        options: Options,
    ) -> anyhow::Result<Self> {
        let head = repo.head()?;
        let mut is_detached = false;
        let (tip, maybe_name) = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                let mut graph = Graph::default();
                // It's OK to default-initialise this here as overlays are only used when redoing
                // the traversal.
                let (_repo, meta, _entrypoint) = Overlay::default().into_parts(repo, meta);
                let wt_by_branch = {
                    // Assume linked worktrees are never unborn!
                    let mut m = BTreeMap::new();
                    m.insert(ref_name.clone(), vec![crate::Worktree::Main]);
                    m
                };
                graph.insert_segment_set_entrypoint(branch_segment_from_name_and_meta(
                    Some((ref_name, None)),
                    &meta,
                    None,
                    &wt_by_branch,
                )?);
                return Ok(graph);
            }
            gix::head::Kind::Detached { target, peeled } => {
                is_detached = true;
                (peeled.unwrap_or(target).attach(repo), None)
            }
            gix::head::Kind::Symbolic(existing_reference) => {
                let mut existing_reference = existing_reference.attach(repo);
                let tip = existing_reference.peel_to_id()?;
                (tip, Some(existing_reference.inner.name))
            }
        };

        let mut graph = Self::from_commit_traversal(tip, maybe_name, meta, options)?;
        if is_detached {
            graph.detach_entrypoint_segment()?;
        }
        Ok(graph)
    }
    /// Produce a minimal but usable representation of the commit-graph reachable from the commit at `tip` such the returned instance
    /// can represent everything that's observed, without losing information.
    /// `ref_name` is assumed to point to `tip` if given.
    ///
    /// `meta` is used to learn more about the encountered references, and `options` is used for additional configuration.
    ///
    /// ### Features
    ///
    /// * discover a Workspace on the fly based on `meta`-data.
    /// * support the notion of a branch to integrate with, the *target*
    ///     - *target* branches consist of a local and remote tracking branch, and one can be ahead of the other.
    ///     - workspaces are relative to the local tracking branch of the target.
    ///     - options contain an [`extra_target_commit_id`](Options::extra_target_commit_id) for an additional target location.
    /// * remote tracking branches are seen in relation to their branches.
    /// * the graph of segments assigns each reachable commit to exactly one segment
    /// * one can use [`petgraph::algo`] and [`petgraph::visit`]
    ///     - It maintains information about the intended connections, so modifications afterward will show
    ///       in debugging output if edges are now in violation of this constraint.
    ///
    /// ### Rules
    ///
    /// These rules should help to create graphs and segmentations that feel natural and are desirable to the user,
    /// while avoiding traversing the entire commit-graph all the time.
    /// Change the rules as you see fit to accomplish this.
    ///
    /// * a commit can be governed by multiple workspaces
    /// * as workspaces and entry-points "grow" together, we don't know anything about workspaces until the very end,
    ///   or when two partitions of commits touch.
    ///   This means we can't make decisions based on [flags](CommitFlags) until the traversal
    ///   is finished.
    /// * an entrypoint always causes the start of a [`Segment`].
    /// * Segments are always named if their first commit has a single local branch pointing to it, or a branch that
    ///   otherwise can be disambiguated.
    /// * Anonymous segments are created if their name is ambiguous.
    /// * Anonymous segments are created if another segment connects to a commit that it contains that is not the first one.
    ///    - This means, all connections go *from the last commit in a segment to the first commit in another segment*.
    /// * Segments stored in the *workspace metadata* are used/relevant only if they are backed by an existing branch.
    /// * Remote tracking branches are picked up during traversal for any ref that we reached through traversal.
    ///     - This implies that remotes aren't relevant for segments added during post-processing, which would typically
    ///       be empty anyway.
    ///     - Remotes never take commits that are already owned.
    /// * The traversal is cut short when there is only tips which are integrated
    /// * The traversal is always as long as it needs to be to fully reconcile possibly disjoint branches, despite
    ///   this sometimes costing some time when the remote is far ahead in a huge repository.
    #[instrument(name = "Graph::from_commit_traversal", level = "trace", skip_all, fields(tip = ?tip, ref_name), err(Debug))]
    pub fn from_commit_traversal(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl RefMetadata,
        options: Options,
    ) -> anyhow::Result<Self> {
        let (repo, meta, _entrypoint) = Overlay::default().into_parts(tip.repo, meta);
        Graph::from_commit_traversal_inner(
            tip.detach(),
            &repo,
            ref_name.into(),
            &meta,
            options,
            TipSource::FromMetadata,
        )
    }

    /// Produce a graph from already resolved tips and their traversal roles.
    ///
    /// This is useful for callers that already know the commits they want to
    /// relate, or whose tips are not represented by durable repository refs or
    /// workspace metadata.
    ///
    /// `repo` provides commit objects, refs, remotes, worktrees, and optional
    /// commit-graph acceleration for traversal.
    /// `tips` provides the resolved commits and their traversal roles. It must
    /// contain exactly one [`TipRole::EntryPoint`] or
    /// [`TipRole::DetachedEntryPoint`].
    /// `meta` provides branch metadata for any refs encountered while walking.
    /// `options` controls tag collection, traversal limits, additional
    /// integrated tips, and post-processing behavior.
    pub fn from_commit_traversal_tips(
        repo: &gix::Repository,
        tips: impl IntoIterator<Item = Tip>,
        meta: &impl RefMetadata,
        options: Options,
    ) -> anyhow::Result<Self> {
        let tips: Vec<_> = tips.into_iter().collect();
        let entrypoint = validate_explicit_tips(repo, &tips)?;
        let tip = entrypoint.id;
        let ref_name = match entrypoint.role {
            TipRole::EntryPoint => entrypoint.ref_name.clone(),
            TipRole::DetachedEntryPoint => None,
            TipRole::Reachable | TipRole::Integrated => unreachable!("filtered above"),
        };
        let is_detached = entrypoint.role == TipRole::DetachedEntryPoint;

        let (repo, meta, _entrypoint) = Overlay::default().into_parts(repo, meta);
        let mut graph = Graph::from_commit_traversal_inner(
            tip,
            &repo,
            ref_name,
            &meta,
            options,
            TipSource::Explicit(tips),
        )?;
        if is_detached {
            graph.detach_entrypoint_segment()?;
        }
        Ok(graph)
    }

    fn from_commit_traversal_inner<T: RefMetadata>(
        tip: gix::ObjectId,
        repo: &OverlayRepo<'_>,
        ref_name: Option<gix::refs::FullName>,
        meta: &OverlayMetadata<'_, T>,
        options: Options,
        tip_source: TipSource,
    ) -> anyhow::Result<Self> {
        {
            if let Some(name) = &ref_name {
                let span = tracing::Span::current();
                span.record("ref_name", name.as_bstr().to_str_lossy().as_ref());
            }
        }
        let mut graph = Graph {
            options: options.clone(),
            entrypoint_ref: ref_name.clone(),
            ..Graph::default()
        };
        let Options {
            collect_tags,
            extra_target_commit_id,
            commits_limit_hint: limit,
            commits_limit_recharge_location: mut max_commits_recharge_location,
            hard_limit,
            dangerously_skip_postprocessing_for_debugging,
        } = options;
        if let Some(extra_tip) = extra_target_commit_id {
            graph
                .tips_of_interest
                .push(Tip::integrated(extra_tip, None));
        }
        if let TipSource::Explicit(tips) = &tip_source {
            graph.tips_of_interest.extend(
                tips.iter()
                    .filter(|tip| tip.role == TipRole::Integrated)
                    .cloned(),
            );
        }

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
        let initial_tips = initial_tips_from_source(
            repo,
            meta,
            tip,
            ref_name.as_ref(),
            tip_source,
            extra_target_commit_id,
        )?;
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
        let mut seen = gix::revwalk::graph::IdMap::<SegmentIndex>::default();
        let mut goals = Goals::default();
        // The tip transports itself.
        let tip_flags = CommitFlags::NotInRemote
            | goals
                .flag_for(tip)
                .expect("we more than one bitflags for this");

        let mut next = Queue::new_with_limit(hard_limit);
        let worktree_by_branch =
            repo.worktree_branches(graph.entrypoint_ref.as_ref().map(|r| r.as_ref()))?;

        let mut ctx = post::Context {
            repo,
            symbolic_remote_names: &initial_tips.symbolic_remote_names,
            configured_remote_tracking_branches: &configured_remote_tracking_branches,
            inserted_proxy_segments: Vec::new(),
            refs_by_id,
            hard_limit: false,
            dangerously_skip_postprocessing_for_debugging,
            worktree_by_branch,
        };

        let target_limit = max_limit
            .with_indirect_goal(tip, &mut goals)
            .without_allowance();
        ctx.inserted_proxy_segments = queue_initial_tips(
            &mut graph,
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
        let mut points_of_interest_to_traverse_first = next.iter().count();
        while let Some((info, mut propagated_flags, instruction, mut limit)) = next.pop_front() {
            points_of_interest_to_traverse_first =
                points_of_interest_to_traverse_first.saturating_sub(1);

            let id = info.id;
            if max_commits_recharge_location.binary_search(&id).is_ok() {
                limit.set_but_keep_goal(max_limit);
            }
            let src_flags = graph[instruction.segment_idx()]
                .commits
                .last()
                .map(|c| c.flags)
                .unwrap_or_default();

            // These flags might be outdated as they have been queued, meanwhile we may have propagated flags.
            // So be sure this gets picked up.
            propagated_flags |= src_flags;
            let is_shallow_boundary = shallow_commits
                .as_ref()
                .is_some_and(|boundary| boundary.binary_search(&id).is_ok());
            if is_shallow_boundary {
                propagated_flags |= CommitFlags::ShallowBoundary;
            }
            let segment_idx_for_id = match instruction {
                Instruction::CollectCommit { into: src_sidx } => match seen.entry(id) {
                    Entry::Occupied(_) => {
                        possibly_split_occupied_segment(
                            &mut graph,
                            &mut seen,
                            &mut next,
                            id,
                            propagated_flags,
                            src_sidx,
                            limit,
                            0,
                        )?;
                        continue;
                    }
                    Entry::Vacant(e) => {
                        let src_sidx = try_split_non_empty_segment_at_branch(
                            &mut graph,
                            src_sidx,
                            &info,
                            &ctx.refs_by_id,
                            meta,
                            &ctx.worktree_by_branch,
                        )?
                        .unwrap_or(src_sidx);
                        e.insert(src_sidx);
                        src_sidx
                    }
                },
                Instruction::ConnectNewSegment {
                    parent_above,
                    at_commit,
                    parent_order,
                } => match seen.entry(id) {
                    Entry::Occupied(_) => {
                        possibly_split_occupied_segment(
                            &mut graph,
                            &mut seen,
                            &mut next,
                            id,
                            propagated_flags,
                            parent_above,
                            limit,
                            parent_order,
                        )?;
                        continue;
                    }
                    Entry::Vacant(e) => {
                        let segment_below = branch_segment_from_name_and_meta(
                            None,
                            meta,
                            Some((&ctx.refs_by_id, id)),
                            &ctx.worktree_by_branch,
                        )?;
                        let segment_below = graph.connect_new_segment(
                            parent_above,
                            at_commit as CommitIndex,
                            segment_below,
                            0,
                            id,
                            parent_order,
                        );
                        e.insert(segment_below);
                        segment_below
                    }
                },
            };

            let refs_at_commit_before_removal = ctx.refs_by_id.remove(&id).unwrap_or_default();
            let RemoteQueueOutcome {
                items_to_queue_later: remote_items_to_queue_later,
                maybe_make_id_a_goal_so_remote_can_find_local,
                limit_to_let_local_find_remote,
            } = try_queue_remote_tracking_branches(
                repo,
                &refs_at_commit_before_removal,
                &mut graph,
                &initial_tips.symbolic_remote_names,
                &configured_remote_tracking_branches,
                &initial_tips.target_refs,
                meta,
                id,
                limit,
                &mut goals,
                &next,
                &ctx.worktree_by_branch,
                commit_graph.as_ref(),
                repo.for_find_only(),
                &mut buf,
            )?;

            let segment = &mut graph[segment_idx_for_id];
            let commit_idx_for_possible_fork = segment.commits.len();
            let propagated_flags = propagated_flags | maybe_make_id_a_goal_so_remote_can_find_local;
            let hard_limit_hit = queue_parents(
                &mut next,
                &info.parent_ids,
                propagated_flags,
                segment_idx_for_id,
                commit_idx_for_possible_fork,
                limit.additional_goal(limit_to_let_local_find_remote),
                is_shallow_boundary,
                commit_graph.as_ref(),
                repo.for_find_only(),
                &mut buf,
            )?;
            if hard_limit_hit {
                return graph.post_processed(meta, tip, ctx.with_hard_limit());
            }

            segment.commits.push(
                info.into_commit(
                    segment
                        .commits
                        // Flags are additive, and meanwhile something may have dumped flags on us
                        // so there is more compared to when the 'flags' value was put onto the queue.
                        .last()
                        .map_or(propagated_flags, |last| last.flags | propagated_flags),
                    refs_at_commit_before_removal
                        .clone()
                        .into_iter()
                        .filter(|rn| segment.ref_name() != Some(rn.as_ref()))
                        .collect(),
                    &ctx.worktree_by_branch,
                )?,
            );

            for item in remote_items_to_queue_later {
                if next.push_back_exhausted(item) {
                    return graph.post_processed(meta, tip, ctx.with_hard_limit());
                }
            }

            prune_integrated_tips(&mut graph, &mut next)?;
            if points_of_interest_to_traverse_first == 0 {
                next.sort();
            }
        }

        graph.post_processed(meta, tip, ctx)
    }

    /// Take the ref-info from a named segment and put it back onto the first commit
    /// where it pointed to before it was lifted up.
    ///
    /// Graph traversal eagerly names segments from refs pointing at their
    /// first commit. Detached entrypoints keep those refs on the commit, but
    /// the entrypoint segment itself must stay anonymous.
    fn detach_entrypoint_segment(&mut self) -> anyhow::Result<()> {
        let sidx = self
            .entrypoint
            .context("BUG: entrypoint is set after first traversal")?
            .0;
        let s = &mut self[sidx];
        if let Some((rn, first_commit)) = s
            .commits
            .first_mut()
            .and_then(|first_commit| s.ref_info.take().map(|rn| (rn, first_commit)))
        {
            first_commit.refs.push(rn);
        }
        Ok(())
    }

    /// Repeat the traversal that generated this graph using `repo` and `meta`, but allow to set an in-memory
    /// `overlay` to amend the data available from `repo` and `meta`.
    /// This way, one can see this graph as it will be in the future once the changes to `repo` and `meta` are actually made.
    pub fn redo_traversal_with_overlay(
        &self,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        overlay: Overlay,
    ) -> anyhow::Result<Self> {
        let (repo, meta, entrypoint) = overlay.into_parts(repo, meta);
        let (tip, ref_name) = match entrypoint {
            Some(t) => t,
            None => {
                let (entrypoint_sidx, commit) = self
                    .entrypoint
                    .context("BUG: entrypoint must always be set")?;
                let entrypoint_segment = self
                    .inner
                    .node_weight(entrypoint_sidx)
                    .context("BUG: entrypoint segment must be present")?;
                let ref_name = entrypoint_segment.ref_info.clone().map(|ri| ri.ref_name);
                let tip = if ref_name.is_some() {
                    match ref_name.as_ref() {
                        Some(ref_name) => repo
                            .try_find_reference(ref_name.as_ref())?
                            .map(|mut reference| reference.peel_to_id().map(|id| id.detach()))
                            .transpose()?,
                        None => None,
                    }
                } else {
                    None
                };
                let tip = tip
                    .or_else(|| commit.object_id())
                    .context(
                        "BUG: entrypoint must either remember the original commit id or have a resolvable ref",
                    )?;
                (tip, ref_name)
            }
        };
        Graph::from_commit_traversal_inner(
            tip,
            &repo,
            ref_name,
            &meta,
            self.options.clone(),
            TipSource::FromMetadata,
        )
    }

    /// Like [`Self::redo_traversal_with_overlay()`], but replaces this instance, without overlay, and returns
    /// a newly computed workspace for it.
    pub fn into_workspace_of_redone_traversal(
        mut self,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
    ) -> anyhow::Result<crate::Workspace> {
        let new = self.redo_traversal_with_overlay(repo, meta, Default::default())?;
        self = new;
        self.into_workspace()
    }
}

/// Validate caller-provided traversal tips before they seed graph traversal.
///
/// Explicit tips must name exactly one entrypoint, must not reuse commit ids or
/// ref names, must keep detached entrypoints unnamed, and any supplied ref name
/// must resolve to the same commit id as its tip.
fn validate_explicit_tips<'a>(repo: &gix::Repository, tips: &'a [Tip]) -> anyhow::Result<&'a Tip> {
    let mut entrypoints = tips
        .iter()
        .filter(|tip| matches!(tip.role, TipRole::EntryPoint | TipRole::DetachedEntryPoint));
    let entrypoint = entrypoints
        .next()
        .context("explicit traversal tips require exactly one entrypoint")?;
    ensure!(
        entrypoints.next().is_none(),
        "explicit traversal tips require exactly one entrypoint"
    );

    for (idx, tip) in tips.iter().enumerate() {
        ensure!(
            tip.role != TipRole::DetachedEntryPoint || tip.ref_name.is_none(),
            "explicit detached entrypoint tip cannot have a ref name"
        );

        for previous in &tips[..idx] {
            ensure!(
                tip.id != previous.id,
                "explicit traversal tips contain duplicate commit id {id}",
                id = tip.id
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
            let resolved_id = repo
                .try_find_reference(ref_name.as_ref())?
                .with_context(|| format!("explicit traversal tip ref {ref_name} does not exist"))?
                .peel_to_id()?
                .detach();
            ensure!(
                resolved_id == tip.id,
                "explicit traversal tip ref {ref_name} points to {resolved_id}, not {tip_id}",
                tip_id = tip.id
            );
        }
    }

    Ok(entrypoint)
}

/// Build the normalized pre-traversal tip plan from the selected tip source.
fn initial_tips_from_source<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    tip_source: TipSource,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> anyhow::Result<InitialTips> {
    let initial = match tip_source {
        TipSource::Explicit(tips) => {
            let mut initial = initial_tips_from_explicit(repo, tips);
            if let Some(extra_target) = extra_target_commit_id {
                push_integrated_initial_tip(&mut initial, extra_target);
            }
            initial
        }
        TipSource::FromMetadata => initial_tips_from_workspace_metadata(
            repo,
            meta,
            entrypoint,
            entrypoint_ref,
            extra_target_commit_id,
        )?,
    };

    Ok(initial)
}

/// Convert validated caller-provided tips into deterministic initial traversal
/// roots.
fn initial_tips_from_explicit(repo: &OverlayRepo<'_>, tips: Vec<Tip>) -> InitialTips {
    let target_refs = tips
        .iter()
        .filter(|tip| tip.role == TipRole::Integrated)
        .filter_map(|tip| tip.ref_name.clone())
        .collect();
    let symbolic_remote_names =
        symbolic_remote_names_from_refs(repo, tips.iter().filter_map(|tip| tip.ref_name.as_ref()));
    let mut tips: Vec<_> = tips.into_iter().enumerate().collect();
    // Match metadata-derived traversal setup: integrated tips establish the
    // base context, reachable tips connect to it, and the entrypoint anchors
    // the graph once the other roots are queued.
    tips.sort_by_key(|(idx, tip)| (explicit_tip_priority(tip.role), *idx));
    let tips = tips
        .into_iter()
        .map(|(_, tip)| {
            let role = match tip.role {
                TipRole::EntryPoint | TipRole::DetachedEntryPoint => InitialTipRole::EntryPoint,
                TipRole::Reachable => InitialTipRole::Reachable,
                TipRole::Integrated => InitialTipRole::Integrated {
                    local_tracking_key: None,
                    dedupe_if_queued: false,
                },
            };
            InitialTip {
                id: tip.id,
                ref_name: tip.ref_name,
                metadata: None,
                role,
                queue_position: InitialTipQueuePosition::Back,
            }
        })
        .collect();

    InitialTips {
        tips,
        workspace_tips: Vec::new(),
        workspace_ref_names: Vec::new(),
        target_refs,
        symbolic_remote_names,
    }
}

/// Sort key for explicit tips so traversal starts from integrated context,
/// then reachable roots, then the entrypoint.
fn explicit_tip_priority(role: TipRole) -> usize {
    match role {
        TipRole::Integrated => 0,
        TipRole::Reachable => 1,
        TipRole::EntryPoint | TipRole::DetachedEntryPoint => 2,
    }
}

/// Discover workspaces, targets, local tracking branches, and workspace stack
/// branch refs and turn them into initial traversal tips.
fn initial_tips_from_workspace_metadata<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> anyhow::Result<InitialTips> {
    let (workspaces, target_refs) =
        obtain_workspace_infos(repo, entrypoint_ref.map(|rn| rn.as_ref()), meta)?;
    let symbolic_remote_names = symbolic_remote_names_from_workspaces(repo, &workspaces);
    let workspace_ref_names = workspaces
        .iter()
        .map(|(_, ref_name, _)| ref_name.clone())
        .collect();
    let tip_ref_matches_ws_ref = workspaces
        .iter()
        .find_map(|(ws_tip, ws_rn, _)| (Some(ws_rn) == entrypoint_ref).then_some(ws_tip));

    let mut initial = InitialTips {
        tips: Vec::new(),
        workspace_tips: Vec::new(),
        workspace_ref_names,
        target_refs,
        symbolic_remote_names,
    };
    let mut workspace_metas = Vec::new();
    let mut additional_target_commits = Vec::new();
    let mut next_target_local_key = 0;
    let mut queued_ids = Vec::new();

    match tip_ref_matches_ws_ref {
        None => {
            initial.tips.push(InitialTip {
                id: entrypoint,
                ref_name: None,
                metadata: None,
                role: InitialTipRole::EntryPoint,
                queue_position: InitialTipQueuePosition::Back,
            });
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
        initial.workspace_tips.push(ws_tip);
        workspace_metas.push(ws_meta.clone());
        additional_target_commits.extend(ws_meta.target_commit_id);
        initial.tips.push(InitialTip {
            id: ws_tip,
            ref_name: Some(ws_ref.clone()),
            metadata: Some(SegmentMetadata::Workspace(ws_meta.clone())),
            role: InitialTipRole::Workspace {
                is_entrypoint: Some(&ws_ref) == entrypoint_ref,
            },
            queue_position: InitialTipQueuePosition::Front,
        });

        let target = if let Some((target_ref, target_ref_id, local_info)) =
            workspace_target_tip(repo, ws_meta.target_ref.as_ref())?
        {
            let local_info =
                local_info.filter(|(_local_ref_name, local_tip)| !queued_ids.contains(local_tip));
            let local_tracking_key = local_info.as_ref().map(|(local_ref_name, local_tip)| {
                let key = next_target_local_key;
                next_target_local_key += 1;
                (key, local_ref_name.clone(), *local_tip)
            });
            initial.tips.push(InitialTip {
                id: target_ref_id,
                ref_name: Some(target_ref),
                metadata: None,
                role: InitialTipRole::Integrated {
                    local_tracking_key: local_tracking_key.as_ref().map(|(key, _, _)| *key),
                    dedupe_if_queued: false,
                },
                queue_position: InitialTipQueuePosition::Front,
            });
            if let Some((key, local_ref_name, local_tip)) = local_tracking_key {
                initial.tips.push(InitialTip {
                    id: local_tip,
                    ref_name: None,
                    metadata: None,
                    role: InitialTipRole::TargetLocal {
                        key,
                        local_ref_name,
                    },
                    queue_position: InitialTipQueuePosition::Front,
                });
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
        push_integrated_initial_tip(&mut initial, extra_target);
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
        push_integrated_initial_tip(&mut initial, target_commit_id);
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
            initial.tips.push(InitialTip {
                id: segment_tip.detach(),
                ref_name: None,
                metadata: None,
                role: InitialTipRole::WorkspaceStackBranch {
                    desired_ref_name: segment.ref_name,
                },
                queue_position: InitialTipQueuePosition::Back,
            });
        }
    }

    Ok(initial)
}

/// Append an integrated target tip that can be deduplicated against already
/// queued initial work.
fn push_integrated_initial_tip(initial: &mut InitialTips, id: gix::ObjectId) {
    initial.tips.push(InitialTip {
        id,
        ref_name: None,
        metadata: None,
        role: InitialTipRole::Integrated {
            local_tracking_key: None,
            dedupe_if_queued: true,
        },
        queue_position: InitialTipQueuePosition::Front,
    });
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

/// Collect symbolic remote names implied by workspace target refs, workspace
/// `push_remote` settings, and stack branch refs.
fn symbolic_remote_names_from_workspaces(
    repo: &OverlayRepo<'_>,
    workspaces: &[WorkspaceTip],
) -> Vec<String> {
    let remote_names = repo.remote_names();
    let names = workspaces
        .iter()
        .flat_map(|(_, _, data)| {
            data.target_ref
                .as_ref()
                .and_then(|target| {
                    extract_remote_name_and_short_name(target.as_ref(), &remote_names)
                        .map(|(remote, _short_name)| (1, remote))
                })
                .into_iter()
                .chain(data.push_remote.clone().map(|push_remote| (0, push_remote)))
        })
        .chain(workspaces.iter().flat_map(|(_, _, data)| {
            data.stacks.iter().flat_map(|s| {
                s.branches.iter().flat_map(|b| {
                    extract_remote_name_and_short_name(b.ref_name.as_ref(), &remote_names)
                        .map(|(remote, _short_name)| (1, remote))
                })
            })
        }));
    sorted_symbolic_remote_names(names)
}

/// Collect symbolic remote names implied by explicit tip refs.
fn symbolic_remote_names_from_refs<'a>(
    repo: &OverlayRepo<'_>,
    refs: impl Iterator<Item = &'a gix::refs::FullName>,
) -> Vec<String> {
    let remote_names = repo.remote_names();
    sorted_symbolic_remote_names(refs.filter_map(|ref_name| {
        extract_remote_name_and_short_name(ref_name.as_ref(), &remote_names)
            .map(|(remote, _short_name)| (1, remote))
    }))
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
/// ownership roots for post-processing.
#[expect(clippy::too_many_arguments)]
fn queue_initial_tips<T: RefMetadata>(
    graph: &mut Graph,
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
    ctx: &post::Context<'_>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Vec<SegmentIndex>> {
    // Target/local pairs are correlated by the synthetic key stored on
    // `Integrated::local_tracking_key` and `TargetLocal::key`.
    //
    // `target_local_segments` holds the local side once its segment and goal
    // exist. `pending_integrated_tips` holds the integrated target side if it
    // appears first. Once both maps have the same key, the target can be queued
    // with sibling links and an additional goal for the local side.
    let mut target_local_segments = BTreeMap::<usize, (Option<SegmentIndex>, CommitFlags)>::new();
    let mut pending_integrated_tips = BTreeMap::<usize, PendingIntegratedTip>::new();

    for tip in &initial_tips.tips {
        match &tip.role {
            InitialTipRole::WorkspaceStackBranch { .. }
                if next.iter().any(|t| t.0.id == tip.id) =>
            {
                next.add_goal_to(tip.id, goals.flag_for(entrypoint).unwrap_or_default());
                continue;
            }
            InitialTipRole::Integrated {
                dedupe_if_queued: true,
                ..
            } if next.iter().any(|(info, _, _, _)| info.id == tip.id) => {
                continue;
            }
            _ => {}
        }

        let mut segment = branch_segment_from_name_and_meta(
            tip.ref_name
                .clone()
                .map(|ref_name| (ref_name, tip.metadata.clone())),
            meta,
            Some((&ctx.refs_by_id, tip.id)),
            &ctx.worktree_by_branch,
        )?;
        if let InitialTipRole::WorkspaceStackBranch { desired_ref_name } = &tip.role {
            let is_remote = desired_ref_name
                .category()
                .is_some_and(|c| c == Category::RemoteBranch);
            if segment.ref_info.is_none() && is_remote {
                segment.ref_info = Some(crate::RefInfo::from_ref(
                    desired_ref_name.clone(),
                    &ctx.worktree_by_branch,
                ));
                segment.metadata = meta
                    .branch_opt(desired_ref_name.as_ref())?
                    .map(SegmentMetadata::Branch);
            }
        }
        let segment = graph.insert_segment(segment);
        if let InitialTipRole::Integrated {
            local_tracking_key, ..
        } = &tip.role
        {
            let pending = PendingIntegratedTip {
                id: tip.id,
                segment,
                queue_position: tip.queue_position,
            };
            if let Some(key) = local_tracking_key {
                let Some(local) = target_local_segments.get(key).copied() else {
                    pending_integrated_tips.insert(*key, pending);
                    continue;
                };
                queue_pending_integrated_tip(
                    graph,
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
                    graph,
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
            InitialTipRole::EntryPoint => {
                graph.entrypoint = Some((segment, EntryPointCommit::AtCommit(tip.id)));
                (entrypoint_flags, max_limit)
            }
            InitialTipRole::Reachable => {
                reachable_tip_flags_and_limit(tip.id, entrypoint, max_limit, goals)
            }
            InitialTipRole::Integrated { .. } => unreachable!("handled above"),
            InitialTipRole::Workspace { is_entrypoint } => {
                if *is_entrypoint && graph.entrypoint.is_none() {
                    graph.entrypoint = Some((segment, EntryPointCommit::AtCommit(tip.id)));
                }
                let extra_flags = is_entrypoint
                    .then_some(entrypoint_flags)
                    .unwrap_or_default();
                let limit = if *is_entrypoint {
                    max_limit
                } else {
                    max_limit.with_indirect_goal(entrypoint, goals)
                };
                (
                    CommitFlags::InWorkspace | CommitFlags::NotInRemote | extra_flags,
                    limit,
                )
            }
            InitialTipRole::TargetLocal {
                key,
                local_ref_name,
            } => {
                let has_remote_link = {
                    let s = &graph[segment];
                    s.ref_name()
                        .is_some_and(|ref_name| ref_name == local_ref_name.as_ref())
                };
                let goal = goals.flag_for(tip.id).unwrap_or_default();
                target_local_segments.insert(*key, (has_remote_link.then_some(segment), goal));
                next.add_goal_to(entrypoint, goal);
                (CommitFlags::NotInRemote | goal, target_limit)
            }
            InitialTipRole::WorkspaceStackBranch { .. } => (
                CommitFlags::NotInRemote,
                max_limit.with_indirect_goal(entrypoint, goals),
            ),
        };
        let tip_info = find(commit_graph, repo.for_find_only(), tip.id, buf)?;
        let item = (
            tip_info,
            flags,
            Instruction::CollectCommit { into: segment },
            limit,
        );
        // A target ref and its local tracking branch can point at the same
        // commit. In that case, the integrated target was held back only until
        // the local side created its segment and goal above. Queue the
        // integrated item before pushing the current local item so the shared
        // commit is owned as integrated history while still carrying the local
        // goal that lets both sides connect.
        let pending_before_current = match &tip.role {
            InitialTipRole::TargetLocal { key, .. } => pending_integrated_tips
                .get(key)
                .is_some_and(|pending| pending.id == tip.id)
                .then(|| pending_integrated_tips.remove(key))
                .flatten(),
            _ => None,
        };
        if let Some(pending) = pending_before_current {
            let local = match &tip.role {
                InitialTipRole::TargetLocal { key, .. } => target_local_segments
                    .get(key)
                    .copied()
                    .unwrap_or((None, CommitFlags::empty())),
                _ => (None, CommitFlags::empty()),
            };
            queue_pending_integrated_tip(
                graph,
                next,
                pending,
                local,
                target_limit,
                commit_graph,
                repo,
                buf,
            )?;
        }
        match tip.queue_position {
            InitialTipQueuePosition::Front => _ = next.push_front_exhausted(item),
            InitialTipQueuePosition::Back => _ = next.push_back_exhausted(item),
        }

        if let InitialTipRole::TargetLocal { key, .. } = &tip.role
            && let Some(pending) = pending_integrated_tips.remove(key)
        {
            let local = target_local_segments
                .get(key)
                .copied()
                .unwrap_or((None, CommitFlags::empty()));
            queue_pending_integrated_tip(
                graph,
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
        graph,
        next,
        (initial_tips.workspace_tips.clone(), repo, meta),
        &ctx.worktree_by_branch,
    )
}

/// Queue an integrated target after optionally linking it to its local tracking segment.
#[expect(clippy::too_many_arguments)]
fn queue_pending_integrated_tip(
    graph: &mut Graph,
    next: &mut Queue,
    pending: PendingIntegratedTip,
    local: (Option<SegmentIndex>, CommitFlags),
    target_limit: Limit,
    commit_graph: Option<&gix::commitgraph::Graph>,
    repo: &OverlayRepo<'_>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let (local_sidx, local_goal) = local;
    if let Some(local_sidx) = local_sidx {
        graph[local_sidx].remote_tracking_branch_segment_id = Some(pending.segment);
        graph[pending.segment].sibling_segment_id = Some(local_sidx);
    }
    let tip_info = find(commit_graph, repo.for_find_only(), pending.id, buf)?;
    let item = (
        tip_info,
        CommitFlags::Integrated,
        Instruction::CollectCommit {
            into: pending.segment,
        },
        target_limit.additional_goal(local_goal),
    );
    match pending.queue_position {
        InitialTipQueuePosition::Front => _ = next.push_front_exhausted(item),
        InitialTipQueuePosition::Back => _ = next.push_back_exhausted(item),
    }
    Ok(())
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

impl Graph {
    /// Connect two existing segments `src` from `src_commit` to point `dst_commit` of `b`.
    pub(crate) fn connect_segments(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
    ) {
        self.connect_segments_with_ids(src, src_commit, None, dst, dst_commit, None, 0)
    }

    #[expect(clippy::too_many_arguments)]
    pub(crate) fn connect_segments_with_ids(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        src_id: Option<gix::ObjectId>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
        dst_id: Option<gix::ObjectId>,
        parent_order: u32,
    ) {
        let src_commit = src_commit.into();
        let dst_commit = dst_commit.into();
        let new_edge_id = self.inner.add_edge(
            src,
            dst,
            Edge {
                src: src_commit,
                src_id: src_id.or_else(|| self[src].commit_id_by_index(src_commit)),
                dst: dst_commit,
                dst_id: dst_id.or_else(|| self[dst].commit_id_by_index(dst_commit)),
                parent_order,
            },
        );
        self.rebuild_outgoing_edges_for_traversal_order(src, new_edge_id);
    }

    fn rebuild_outgoing_edges_for_traversal_order(
        &mut self,
        src: SegmentIndex,
        new_edge_id: petgraph::stable_graph::EdgeIndex,
    ) {
        let mut new_edge = None;
        let mut outgoing_edges = Vec::new();
        for edge in self.inner.edges_directed(src, Direction::Outgoing) {
            let edge = EdgeOwned::from(edge);
            if edge.id == new_edge_id {
                new_edge = Some(edge);
            } else {
                outgoing_edges.push(edge);
            }
        }

        let Some(new_edge) = new_edge else {
            return;
        };
        if outgoing_edges.is_empty() {
            return;
        }

        let insert_at = outgoing_edges
            .partition_point(|edge| edge.weight.parent_order <= new_edge.weight.parent_order);
        outgoing_edges.insert(insert_at, new_edge);

        for edge in &outgoing_edges {
            self.inner.remove_edge(edge.id);
        }
        for edge in outgoing_edges.into_iter().rev() {
            self.inner.add_edge(edge.source, edge.target, edge.weight);
        }
    }
}
