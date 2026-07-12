//! The walk: observe the repository into a [`CommitGraph`](crate::CommitGraph).
//!
//! Stage one of the pipeline. [`Seed`](crate::walk::Seed)s resolve from `HEAD`, the workspace ref, the
//! target and the stack branches (each with a [`SeedRole`](crate::walk::SeedRole)); the traversal (the private
//! `walker`) expands them under [`Options`](crate::walk::Options) limits and goals; and what it saw becomes
//! data — an arena of commits with ordered parent arrays, every encountered ref attached
//! to its commit, flags settled once the partitions reconcile, and the seeds themselves
//! recorded on the graph for later passes. Nothing here decides workspace structure;
//! the build does that afterwards, from what the walk observed.
//!
//! An [`Overlay`](crate::walk::Overlay) previews unwritten state: extra or dropped references and metadata
//! overrides are merged into every repository/metadata read (the private
//! `overlay::OverlayRepo`/`overlay::OverlayMetadata`), so re-running the walk sees the
//! hypothetical world through the same code path as the real one.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, bail, ensure};
use but_core::{
    RefMetadata, extract_remote_name_and_short_name,
    ref_metadata::{self, ProjectMeta},
};
use gix::prelude::{ObjectIdExt, ReferenceExt};
use tracing::instrument;

use crate::{CommitFlags, SegmentMetadata, workspace::GraphContext};

pub(crate) mod utils;
use utils::*;

pub(crate) mod types;
use types::{Goals, Limit};

use crate::walk::overlay::{OverlayMetadata, OverlayRepo};

mod remotes;

pub(crate) mod overlay;
pub(crate) mod walker;

pub(crate) type Entrypoint = Option<(gix::ObjectId, Option<gix::refs::FullName>)>;

/// A resolved commit that seeds graph traversal without requiring it to be
/// discoverable through repository refs or workspace metadata.
///
/// The traversal accumulates the commit arena; every structural notion built on
/// top of seeds — chains, empties, naming — is decided by the build from the
/// arena and the seed records carried on it.
#[derive(Debug, Clone)]
pub struct Seed {
    /// The commit id to start walking from.
    pub id: gix::ObjectId,
    /// The ref name to assign to the seed segment, if it should be named.
    pub ref_name: Option<gix::refs::FullName>,
    /// How this seed participates in traversal.
    pub role: SeedRole,
    /// Metadata to attach to the initial segment.
    pub metadata: Option<SegmentMetadata>,
    /// Whether this seed is the user-facing traversal entrypoint.
    ///
    /// There may only be *one such seed*.
    /// Other seeds try to connect to any commit reachable from this one.
    pub is_entrypoint: bool,
    /// Whether the entrypoint segment should remain anonymous even if refs
    /// point at the same commit.
    pub is_detached: bool,
}

/// Lifecycle
impl Seed {
    /// A minimal seed at `id`: unnamed, not the entrypoint, no metadata,
    /// default reachable semantics.
    pub fn new(id: gix::ObjectId) -> Self {
        Seed {
            id,
            ref_name: None,
            role: SeedRole::default(),
            metadata: None,
            is_entrypoint: false,
            is_detached: false,
        }
    }

    /// A traversal entrypoint at `id`, named by `ref_name` if the caller has a
    /// stable ref for it.
    pub fn entrypoint(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Seed::new(id).with_ref_name(ref_name).with_entrypoint()
    }

    /// An entrypoint at `id` whose segment should remain detached even if refs
    /// point to its commit.
    pub fn detached_entrypoint(id: gix::ObjectId) -> Self {
        Seed::new(id).with_detached_entrypoint()
    }

    /// A non-remote traversal root at `id`, named by `ref_name` if available.
    pub fn reachable(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Seed::new(id).with_ref_name(ref_name)
    }

    /// A target/integration seed at `id` that bounds or extends traversal
    /// context — the part of the graph that [`Self::reachable()`] parts want
    /// to integrate with. Named by `ref_name` if available.
    pub fn integrated(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Self {
        Seed::new(id)
            .with_ref_name(ref_name)
            .with_role(SeedRole::TargetRemote)
    }
}

/// Builder
impl Seed {
    /// Set the ref name that will name this seed's segment, bypassing normal ref discovery.
    pub fn with_ref_name(mut self, ref_name: Option<gix::refs::FullName>) -> Self {
        self.ref_name = ref_name;
        self
    }

    /// Set the traversal role for this seed.
    pub fn with_role(mut self, role: SeedRole) -> Self {
        self.role = role;
        self
    }

    /// Set whether this seed is the traversal entrypoint.
    pub(crate) fn with_is_entrypoint(mut self, is_entrypoint: bool) -> Self {
        self.is_entrypoint = is_entrypoint;
        self
    }

    /// Set whether this seed should use detached entrypoint presentation, which makes it anonymous even
    /// if it could receive a name/unambiguous ref otherwise.
    pub fn with_is_detached(mut self, is_detached: bool) -> Self {
        self.is_detached = is_detached;
        self
    }

    /// Mark this seed as the traversal entrypoint.
    pub fn with_entrypoint(self) -> Self {
        self.with_is_entrypoint(true)
    }

    /// Mark this entrypoint as detached for segment presentation.
    pub(crate) fn with_detached_entrypoint(mut self) -> Self {
        self = self.with_is_entrypoint(true).with_is_detached(true);
        self
    }

    /// Attach metadata to the initial segment created for this seed.
    pub fn with_metadata(mut self, metadata: SegmentMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Utilities
impl Seed {
    /// Whether this seed is commit-only integrated target context, like
    /// `extra_target_commit_id` or a persisted workspace target commit —
    /// no ref to preserve in the projection.
    fn is_anonymous_integrated_target_context(&self) -> bool {
        matches!(self.role, SeedRole::TargetRemote) && self.ref_name.is_none()
    }

    /// Whether this anonymous integrated target was derived by normalization
    /// (from metadata or `extra_target_commit_id`) rather than passed by the
    /// caller. Such seeds are limits/context and get ordered and deduplicated
    /// as auxiliary work, not as user-visible roots.
    fn is_auxiliary_integrated_seed(
        &self,
        auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
    ) -> bool {
        self.is_anonymous_integrated_target_context()
            && auxiliary_integrated_seed_ids.contains(&self.id)
    }

    /// Whether a named target ref already covers this anonymous target's
    /// commit. Keeping both would let the anonymous seed own the commit and
    /// leave the named ref as a duplicate empty segment.
    fn collapses_into_named_integrated_target(
        &self,
        named_integrated_target_ids: &BTreeSet<gix::ObjectId>,
    ) -> bool {
        self.is_anonymous_integrated_target_context()
            && named_integrated_target_ids.contains(&self.id)
    }
}

/// The role a resolved traversal seed plays when constructing a graph.
///
/// Roles decide the initial [`CommitFlags`] and `Limit` goals used by the
/// walk. The explicit entrypoint is the shared goal: reachable and integrated
/// seeds seek connection to it by walking history until they encounter the entrypoint's
/// propagated goal flag.
///
/// Remote-tracking seeds are not modeled as explicit [`SeedRole`] values. They
/// are discovered during traversal from refs found at visited commits and their
/// configured or deduced remote-tracking branches. When such a remote seed is
/// queued, it receives an indirect goal for the local commit where it was
/// discovered, while that local side receives a goal for the remote seed. This
/// reciprocal goal setup lets remote and local tracking histories converge until
/// the graph can connect them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SeedRole {
    /// A non-remote seed that should be traversed and related to the entrypoint.
    ///
    /// This seed marks all commits it traverses with [`CommitFlags::NotInRemote`].
    #[default]
    Reachable,
    /// The workspace ref itself, paired with workspace metadata on [`Seed`].
    ///
    /// This marks commits as in-workspace with [`CommitFlags::InWorkspace`].
    Workspace,
    /// A branch from a stack listed in workspace metadata.
    ///
    /// Its current ref tip should be traversed even if it is not reachable from
    /// the workspace commit.
    WorkspaceStackBranch {
        /// Ref name from workspace metadata, used as a naming fallback when
        /// the initial segment can't infer an unambiguous ref from the seed
        /// commit (e.g. remote-only stack refs).
        ///
        /// Not [`Seed::ref_name`], which would force the name and bypass
        /// normal ref discovery and ambiguity handling.
        ///
        /// [Seed::id] is assumed to be the peeled commit this ref points to.
        desired_ref_name: gix::refs::FullName,
    },
    /// A target/integration seed whose reachable history is considered integrated,
    /// and that reachable/unintegrated seeds want to connect with.
    ///
    /// This seed receives [`CommitFlags::Integrated`] and an indirect goal for
    /// the entrypoint commit with no extra allowance once that goal is found. It
    /// walks just far enough to connect target history to the entrypoint's
    /// ancestry.
    TargetRemote,
    /// The local branch that tracks an integrated target branch.
    ///
    /// It receives a goal for the target and later provides the segment id that
    /// lets the target segment point back to its local sibling.
    TargetLocal {
        /// The expected local tracking ref, used to verify that the segment
        /// ref discovery created really is the local side of this target.
        ///
        /// Not [`Seed::ref_name`], which would force the name and bypass
        /// ambiguity checks: if another local branch shares the seed commit,
        /// the segment may represent that branch or stay anonymous, and
        /// linking it as the target's local side would point ahead/behind and
        /// remote-reachability queries at the wrong segment.
        local_ref_name: gix::refs::FullName,
    },
}

/// Access
impl SeedRole {
    /// Whether this role represents integrated history.
    pub fn is_integrated(&self) -> bool {
        matches!(self, SeedRole::TargetRemote)
    }
}

/// A local branch ref and the commit it points to, when it tracks a workspace
/// target ref.
pub(crate) type LocalTrackingTip = (gix::refs::FullName, gix::ObjectId);

/// A workspace target ref, its commit, and optionally the local branch tracking it.
pub(crate) type WorkspaceTargetTip = (gix::refs::FullName, gix::ObjectId, Option<LocalTrackingTip>);

/// The complete pre-traversal plan derived from either explicit seeds or
/// workspace metadata.
///
/// `queue_initial_seeds()` consumes this to mint the seed-table records and fill the traversal
/// queue, and provides the auxiliary ref/remote information the traversal and build need. Each
/// seed gets its own (possibly empty) segment during the walk.
struct InitialSeeds {
    /// Ordered traversal roots to turn into segments and queue items.
    seeds: Vec<Seed>,
    /// Workspace commits used to ensure commits remain owned by the workspace
    /// roots that introduced them.
    workspace_seeds: Vec<gix::ObjectId>,
    /// Workspace ref names that should be included while collecting refs by
    /// prefix, even when they are not reachable from the entrypoint yet.
    workspace_ref_names: Vec<gix::refs::FullName>,
    /// Remote target refs already scheduled as initial integrated seeds, so
    /// `try_queue_remote_tracking_branches()` won't queue them again when
    /// local branches point at them as upstreams.
    // TODO: could this be removed in favor of using `CommitGraph::seeds`?
    target_refs: Vec<gix::refs::FullName>,
    /// Remote names to try when a local branch has no configured upstream:
    /// `refs/remotes/<remote>/<local-short-name>` is used if that ref exists
    /// and isn't configured for another branch.
    symbolic_remote_names: Vec<String>,
    /// Whether metadata-derived workspace/target seeds should be front-loaded
    /// into the traversal queue after their segments are created.
    frontload_workspace_related_seeds: bool,
    /// Target remote/local tracking links inferred from seed refs and branch
    /// config.
    ///
    /// Needed up front because the two sides may share a commit or arrive in
    /// either order: queueing delays the target until the local side has a
    /// segment, then links both as siblings before other seeds can claim
    /// their commits.
    target_local_links: TargetLocalLinks,
    /// Anonymous target-remote seeds that are auxiliary traversal context rather
    /// than primary target refs.
    auxiliary_integrated_seed_ids: BTreeSet<gix::ObjectId>,
}

/// Bidirectional lookup between target remote refs and their local tracking refs.
#[derive(Default)]
struct TargetLocalLinks {
    /// Local tracking ref by target remote ref.
    local_by_target: BTreeMap<gix::refs::FullName, gix::refs::FullName>,
    /// Target remote ref by local tracking ref.
    target_by_local: BTreeMap<gix::refs::FullName, gix::refs::FullName>,
}

/// Unwritten state served from memory instead of the repository when (re)building the graph:
/// extra or dropped references and metadata overrides.
#[derive(Debug, Default, Clone)]
pub struct Overlay {
    entrypoint: Entrypoint,
    nonoverriding_references: Vec<gix::refs::Reference>,
    overriding_references: Vec<gix::refs::Reference>,
    /// Refs the re-traversal must not pick up — see [`Overlay::with_dropped_references`].
    dropped_references: Vec<gix::refs::FullName>,
    meta_branches: Vec<(gix::refs::FullName, ref_metadata::Branch)>,
    branch_stack_orders: Vec<Vec<gix::refs::FullName>>,
    workspace: Option<(gix::refs::FullName, ref_metadata::Workspace)>,
}

/// Options for the graph traversals (`CommitGraph::from_head`, `CommitGraph::from_tip`).
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
    /// Also note: This is a hint and not an exact measure, and it's always possible to receive
    /// more commits than asked for — e.g. remote branches must be able to find their local
    /// branch regardless of the limit.
    pub commits_limit_hint: Option<usize>,
    /// Commits at which the remaining budget resets to `commits_limit_hint` — typically the
    /// last commits of partial segments a previous traversal returned. Think of them as
    /// refuelling stops that direct where the commit budget is spent.
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
    /// The tip of one additional, anonymous target — it joins the configured
    /// target (never overrides it) wherever targets act: integration marking,
    /// and the workspace's lower bound, a merge-base over the stack tips and
    /// all target positions where the lowest target decides (`min(targets)`).
    /// A past target position thus pulls the floor down, re-revealing the
    /// integrated span pruning would otherwise hide; with no configured
    /// target it is the sole source of integration.
    pub extra_target_commit_id: Option<gix::ObjectId>,
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
    /// Set a soft cap on how many commits each seed's walk may traverse. Building consistent,
    /// connected graphs takes precedence over the cap.
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

    /// Set [`Self::extra_target_commit_id`]. Tests use it to nominate a target
    /// without remote or metadata setup; production feeds the persisted
    /// workspace target commit through the same seeding via metadata.
    pub fn with_extra_target_commit_id(mut self, id: impl Into<gix::ObjectId>) -> Self {
        self.extra_target_commit_id = Some(id.into());
        self
    }
}

/// Lifecycle
impl crate::CommitGraph {
    /// Read the `HEAD` of `repo` and represent whatever is visible as a graph.
    ///
    /// See [`Self::from_tip()`] for details.
    pub(crate) fn from_head(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: Options,
    ) -> anyhow::Result<(crate::CommitGraph, GraphContext)> {
        let head = repo.head()?;
        // The dispatch lives in `from_tip` (which every case below delegates to):
        // a checkout inside a managed workspace — including HEAD on the workspace ref itself —
        // builds via the managed builder, everything else via the non-managed one.
        let mut is_detached = false;
        let (seed, maybe_name) = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                let mut cg = crate::CommitGraph::default();
                // The frame reads the entrypoint ref off the substrate even for a
                // commitless graph.
                cg.set_entrypoint_ref(ref_name.clone());
                // It's OK to default-initialise this here as overlays are only used when redoing
                // the traversal.
                let (_repo, meta, _entrypoint) = Overlay::default().into_parts(repo, meta);
                let wt_by_branch = {
                    // Assume linked worktrees are never unborn!
                    let mut m = BTreeMap::new();
                    m.insert(
                        ref_name.clone(),
                        vec![crate::Worktree {
                            kind: crate::WorktreeKind::Main,
                            owned_by_repo: true,
                        }],
                    );
                    m
                };
                let segment = branch_segment_from_name_and_meta(
                    Some((ref_name, None)),
                    &meta,
                    None,
                    &wt_by_branch,
                )?;
                let branch_details = segment
                    .ref_info
                    .as_ref()
                    .map(|ri| {
                        let details = crate::workspace::BranchDetails {
                            metadata: segment.metadata.as_ref().and_then(|md| match md {
                                crate::SegmentMetadata::Branch(md) => Some(md.clone()),
                                crate::SegmentMetadata::Workspace(_) => None,
                            }),
                            worktree: ri.worktree.clone(),
                            remote_walk_tip: None,
                        };
                        std::iter::once((ri.ref_name.clone(), details)).collect()
                    })
                    .unwrap_or_default();
                let ctx = GraphContext {
                    project_meta,
                    branch_details,
                    ..Default::default()
                };
                return Ok((cg, ctx));
            }
            gix::head::Kind::Detached { target, peeled } => {
                is_detached = true;
                (peeled.unwrap_or(target).attach(repo), None)
            }
            gix::head::Kind::Symbolic(existing_reference) => {
                let mut existing_reference = existing_reference.attach(repo);
                let seed = existing_reference.peel_to_id()?;
                (seed, Some(existing_reference.inner.name))
            }
        };

        let (mut cg, ctx) = Self::from_tip(seed, maybe_name, meta, project_meta, options)?;
        if is_detached && let Some(seed) = cg.seeds.iter_mut().find(|t| t.is_entrypoint) {
            // Detachment rides on the substrate's entrypoint seed — the
            // projection anonymizes the entry from it.
            seed.is_detached = true;
        }
        Ok((cg, ctx))
    }
    /// Build the workspace's substrate from the commit at `seed` (`ref_name` is assumed to
    /// point to it if given): a managed workspace is discovered on the fly from `meta`, else
    /// the non-managed builder runs.
    ///
    /// Walk rules the traversal owns:
    /// * Seeding: workspace metadata resolves into [`Seed`]s and follows the explicit-seeds
    ///   path. Explicit seeds carry exactly ONE entrypoint, no duplicates, named seeds must
    ///   resolve to their commit, detached seeds are unnamed entrypoints. Metadata seeds keep
    ///   their queue order; explicit ones normalize (integrated/target first, entrypoint last).
    /// * Flags settle only when the traversal finishes — partitions grow together.
    /// * Remote tracking branches are discovered only for refs the walk reached, and never
    ///   take commits that are already owned.
    /// * The traversal cuts short when only integrated seeds remain, but always runs long
    ///   enough to reconcile possibly disjoint branches.
    #[instrument(name = "CommitGraph::from_tip", level = "trace", skip_all, fields(seed = ?seed, ref_name), err(Debug))]
    pub(crate) fn from_tip(
        seed: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: Options,
    ) -> anyhow::Result<(crate::CommitGraph, GraphContext)> {
        let repo = seed.repo;
        Self::dispatch_tip(
            repo,
            seed.detach(),
            ref_name.into(),
            meta,
            project_meta,
            options,
            Overlay::default(),
        )
    }

    /// The tip dispatch `from_tip` and [`Workspace::redo`](crate::Workspace::redo) share:
    /// inside a managed workspace (a workspace-ref seed is the plain case, any other checkout
    /// an entrypoint split), else the non-managed builder.
    pub(crate) fn dispatch_tip(
        repo: &gix::Repository,
        seed: gix::ObjectId,
        ref_name: Option<gix::refs::FullName>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: Options,
        overlay: Overlay,
    ) -> anyhow::Result<(crate::CommitGraph, GraphContext)> {
        let is_ws_tip = ref_name
            .as_ref()
            .is_some_and(|r| but_core::is_workspace_ref_name(r.as_ref()));
        let (entrypoint, entrypoint_ref) = if is_ws_tip {
            (None, None)
        } else {
            (Some(seed), ref_name.clone())
        };
        if let Some(graph) = crate::graph_from_repository(
            repo,
            meta,
            entrypoint,
            entrypoint_ref,
            project_meta.clone(),
            options.clone(),
            overlay.clone(),
        )? {
            return Ok(graph);
        }
        // No managed workspace, or the entrypoint is outside it: the non-managed builder.
        crate::graph_from_repository_unmanaged(
            repo,
            meta,
            seed,
            ref_name,
            project_meta,
            options,
            overlay,
        )
    }

    /// Produce a graph from already resolved seeds and their traversal roles.
    ///
    /// This is useful for callers that already know the commits they want to
    /// relate, or whose seeds are not represented by durable repository refs or
    /// workspace metadata.
    ///
    /// `seeds` must contain exactly one seed with [`Seed::is_entrypoint`] set.
    pub(crate) fn from_seeds(
        repo: &gix::Repository,
        seeds: impl IntoIterator<Item = Seed>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: Options,
    ) -> anyhow::Result<(crate::CommitGraph, GraphContext)> {
        let seeds: Vec<_> = seeds.into_iter().collect();
        // Build from a CommitGraph derived from the same seeds traversal.
        crate::graph_from_repository_seeds(repo, meta, seeds, project_meta, options)
    }
}

/// Validate caller-provided seeds: exactly one entrypoint, no duplicate
/// traversal seeds or ref names, detached entrypoints unnamed, and every ref
/// name resolving to its seed's commit.
fn validate_explicit_seeds<'a>(
    repo: &OverlayRepo<'_>,
    seeds: &'a [Seed],
    entrypoint_ref_override: Option<&gix::refs::FullName>,
) -> anyhow::Result<&'a Seed> {
    let mut entrypoints = seeds.iter().filter(|seed| seed.is_entrypoint);
    let entrypoint = entrypoints
        .next()
        .context("explicit traversal seeds require exactly one entrypoint")?;
    ensure!(
        entrypoints.next().is_none(),
        "explicit traversal seeds require exactly one entrypoint"
    );

    for (idx, seed) in seeds.iter().enumerate() {
        ensure!(
            !seed.is_detached || seed.is_entrypoint,
            "explicit detached seed must also be the entrypoint"
        );
        ensure!(
            !seed.is_detached || seed.ref_name.is_none(),
            "explicit detached entrypoint seed cannot have a ref name"
        );
        ensure!(
            !seed.is_entrypoint || matches!(seed.role, SeedRole::Reachable | SeedRole::Workspace),
            "explicit entrypoint seed must be reachable or workspace"
        );

        for previous in &seeds[..idx] {
            ensure!(
                !seeds_have_same_traversal(previous, seed),
                "explicit traversal seeds contain duplicate traversal seed {seed:?}"
            );
            if let Some(ref_name) = seed
                .ref_name
                .as_ref()
                .filter(|ref_name| previous.ref_name.as_ref() == Some(*ref_name))
            {
                bail!("explicit traversal seeds contain duplicate ref name {ref_name}");
            }
        }

        if let Some(ref_name) = seed.ref_name.as_ref() {
            validate_seed_ref(repo, ref_name, seed.id, "explicit traversal seed ref")?;
        }
    }

    if !entrypoint.is_detached
        && let Some(ref_name) = entrypoint_ref_override
    {
        validate_seed_ref(
            repo,
            ref_name,
            entrypoint.id,
            "explicit traversal entrypoint ref",
        )?;
    }

    Ok(entrypoint)
}

fn validate_seed_ref(
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

/// Whether two seeds would seed the same traversal work: same commit id, role,
/// and entrypoint flag. Naming and presentation data are deliberately ignored —
/// they affect the build, but don't make enqueueing the same commit twice useful.
fn seeds_have_same_traversal(previous: &Seed, seed: &Seed) -> bool {
    previous.id == seed.id
        && seeds_have_same_role(previous, seed)
        && previous.is_entrypoint == seed.is_entrypoint
}

/// Whether two seeds have the same traversal role, for deduplication.
///
/// Named and anonymous [`SeedRole::TargetRemote`] seeds count as different:
/// a named one represents a ref (segment, target identity, sibling link), an
/// anonymous one only commit-level context. Normalization later collapses the
/// anonymous form into a named seed on the same commit.
fn seeds_have_same_role(previous: &Seed, seed: &Seed) -> bool {
    match (&previous.role, &seed.role) {
        (SeedRole::TargetRemote, SeedRole::TargetRemote) => {
            previous.ref_name.is_some() == seed.ref_name.is_some()
        }
        _ => previous.role == seed.role,
    }
}

/// Build auxiliary traversal inputs from normalized seeds.
fn assemble_initial_seeds(
    repo: &OverlayRepo<'_>,
    mut seeds: Vec<Seed>,
    project_meta: &ProjectMeta,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> InitialSeeds {
    let mut auxiliary_integrated_seed_ids = BTreeSet::new();
    if let Some(extra_target) = extra_target_commit_id {
        auxiliary_integrated_seed_ids.insert(extra_target);
        push_integrated_seed_once(&mut seeds, extra_target);
    }
    let frontload_workspace_related_seeds = has_workspace_related_seeds(&seeds);
    if frontload_workspace_related_seeds {
        auxiliary_integrated_seed_ids.extend(seeds.iter().filter_map(|seed| {
            seed.is_anonymous_integrated_target_context()
                .then_some(seed.id)
        }));
    }
    collapse_anonymous_integrated_seeds_into_named_targets(&mut seeds);
    let seeds = seeds_in_queue_order(seeds, &auxiliary_integrated_seed_ids);
    let workspace_seeds = seeds
        .iter()
        .filter(|seed| matches!(seed.role, SeedRole::Workspace))
        .map(|seed| seed.id)
        .collect();
    let workspace_ref_names = seeds
        .iter()
        .filter(|seed| matches!(seed.role, SeedRole::Workspace))
        .filter_map(|seed| seed.ref_name.clone())
        .collect();
    let include_seed_refs = !seeds
        .iter()
        .any(|seed| matches!(seed.metadata, Some(SegmentMetadata::Workspace(_))));
    let target_refs = target_refs_from_seeds(&seeds, project_meta, include_seed_refs);
    let symbolic_remote_names =
        symbolic_remote_names_from_seeds(repo, &seeds, project_meta, include_seed_refs);
    let target_local_links = target_local_links_from_seeds(repo, &seeds);

    InitialSeeds {
        seeds,
        workspace_seeds,
        workspace_ref_names,
        target_refs,
        symbolic_remote_names,
        frontload_workspace_related_seeds,
        target_local_links,
        auxiliary_integrated_seed_ids,
    }
}

/// Drop anonymous integrated target seeds whose commit a named integrated
/// target already covers — the named segment should own the commit.
fn collapse_anonymous_integrated_seeds_into_named_targets(seeds: &mut Vec<Seed>) {
    let named_integrated_target_ids = seeds
        .iter()
        .filter_map(|seed| {
            (matches!(seed.role, SeedRole::TargetRemote) && seed.ref_name.is_some())
                .then_some(seed.id)
        })
        .collect::<BTreeSet<_>>();
    seeds.retain(|seed| !seed.collapses_into_named_integrated_target(&named_integrated_target_ids));
}

/// Order validated seeds deterministically — queue order matters because the
/// first item to reach a commit owns its segment.
///
/// Role priority sets the broad shape, workspace metadata restores stack and
/// branch order where available, and stable tie-breakers make equivalent
/// inputs independent of caller order. Non-workspace traversals keep caller
/// order among equal priorities.
fn seeds_in_queue_order(
    seeds: Vec<Seed>,
    auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
) -> Vec<Seed> {
    let has_workspace_related_seeds = has_workspace_related_seeds(&seeds);
    let workspace_branch_order = workspace_branch_order_from_seeds(&seeds);
    let mut seeds: Vec<_> = seeds.into_iter().enumerate().collect();
    seeds.sort_by(|(a_idx, a), (b_idx, b)| {
        seed_queue_priority(
            a,
            has_workspace_related_seeds,
            auxiliary_integrated_seed_ids,
        )
        .cmp(&seed_queue_priority(
            b,
            has_workspace_related_seeds,
            auxiliary_integrated_seed_ids,
        ))
        .then_with(|| {
            seed_metadata_order(a, &workspace_branch_order)
                .cmp(&seed_metadata_order(b, &workspace_branch_order))
        })
        .then_with(|| {
            if has_workspace_related_seeds {
                seed_sort_name(a).cmp(&seed_sort_name(b))
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .then_with(|| {
            if has_workspace_related_seeds {
                a.id.cmp(&b.id)
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .then_with(|| a_idx.cmp(b_idx))
    });
    seeds.into_iter().map(|(_, seed)| seed).collect()
}

/// Whether seed ordering has to emulate workspace metadata traversal: the
/// relative order of workspace-related seeds decides commit ownership and how
/// virtual workspace/stack segments are minted, so their presence switches
/// sorting from "preserve caller order" to "rebuild metadata order".
fn has_workspace_related_seeds(seeds: &[Seed]) -> bool {
    seeds.iter().any(|seed| {
        matches!(
            seed.role,
            SeedRole::Workspace
                | SeedRole::TargetLocal { .. }
                | SeedRole::WorkspaceStackBranch { .. }
        ) || matches!(seed.metadata, Some(SegmentMetadata::Workspace(_)))
    })
}

/// Primary sort key for initial seeds. For workspace-related traversals,
/// recreate the metadata-derived segment creation order:
///
/// 1. A non-workspace reachable entrypoint, if there is one.
/// 2. The workspace ref, the traversal anchor.
/// 3. The integrated target ref, then its local tracking branch, so they can
///    be linked as siblings and agree on target ownership.
/// 4. Synthetic integrated targets, like extra target commits.
/// 5. Workspace stack branches (order refined later from metadata).
/// 6. Other reachable roots.
///
/// For non-workspace traversals there is no metadata order to recover:
/// integrated context first, non-entry reachable roots next, the entrypoint
/// last. Synthetic integrated seeds stay last — auxiliary limits, not roots.
fn seed_queue_priority(
    seed: &Seed,
    has_workspace_related_seeds: bool,
    auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
) -> usize {
    if has_workspace_related_seeds {
        match &seed.role {
            SeedRole::Reachable if seed.is_entrypoint => 0,
            SeedRole::Workspace => 1,
            SeedRole::TargetRemote if seed.ref_name.is_some() => 2,
            SeedRole::TargetLocal { .. } => 3,
            SeedRole::TargetRemote
                if seed.is_auxiliary_integrated_seed(auxiliary_integrated_seed_ids) =>
            {
                4
            }
            SeedRole::TargetRemote => 2,
            SeedRole::WorkspaceStackBranch { .. } => 5,
            SeedRole::Reachable => 6,
        }
    } else {
        match &seed.role {
            SeedRole::TargetRemote
                if seed.is_auxiliary_integrated_seed(auxiliary_integrated_seed_ids) =>
            {
                3
            }
            SeedRole::TargetRemote => 0,
            SeedRole::TargetLocal { .. } => 0,
            SeedRole::Reachable | SeedRole::Workspace | SeedRole::WorkspaceStackBranch { .. } => {
                if seed.is_entrypoint {
                    2
                } else {
                    1
                }
            }
        }
    }
}

/// Recover stack-branch order from workspace metadata — the only reliable way
/// for scrambled explicit input to produce the same graph and projection as
/// `from_tip()`.
///
/// Maps a branch ref to `(workspace_order, stack_order, branch_order)`:
/// workspaces sorted by their optional ref name (deterministic for
/// multi-workspace input), stacks counted among in-workspace stacks only,
/// branches by position in their stack. Refs not in the map fall back to
/// later tie-breakers; on duplicates the first metadata occurrence wins,
/// matching "first configured stack owns the branch".
fn workspace_branch_order_from_seeds(
    seeds: &[Seed],
) -> BTreeMap<gix::refs::FullName, (usize, usize, usize)> {
    let mut workspaces: Vec<_> = seeds
        .iter()
        .filter_map(|seed| match seed.metadata.as_ref() {
            Some(SegmentMetadata::Workspace(data)) => Some((seed.ref_name.as_ref(), data)),
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

/// The metadata order for a workspace stack branch seed; `None` for other
/// roles, which are governed by role priority and later tie-breakers.
fn seed_metadata_order(
    seed: &Seed,
    workspace_branch_order: &BTreeMap<gix::refs::FullName, (usize, usize, usize)>,
) -> Option<(usize, usize, usize)> {
    match &seed.role {
        SeedRole::WorkspaceStackBranch { desired_ref_name } => {
            workspace_branch_order.get(desired_ref_name).copied()
        }
        SeedRole::Reachable
        | SeedRole::Workspace
        | SeedRole::TargetRemote
        | SeedRole::TargetLocal { .. } => None,
    }
}

/// Stable name tie-breaker for workspace-related sorting: sorting by the ref
/// that will name the segment keeps caller input order irrelevant. Ignored
/// for non-workspace traversals, which preserve caller order.
fn seed_sort_name(seed: &Seed) -> Option<String> {
    match &seed.role {
        SeedRole::WorkspaceStackBranch { desired_ref_name } => {
            Some(desired_ref_name.as_bstr().to_string())
        }
        SeedRole::TargetLocal { local_ref_name } => Some(local_ref_name.as_bstr().to_string()),
        SeedRole::Reachable | SeedRole::Workspace | SeedRole::TargetRemote => {
            seed.ref_name.as_ref().map(|ref_name| ref_name.to_string())
        }
    }
}

/// Discover workspaces, targets, local tracking branches, and workspace stack
/// branch refs and turn them into initial traversal seeds.
pub(crate) fn initial_seeds_from_workspace_metadata<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    project_meta: &ProjectMeta,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> anyhow::Result<Vec<Seed>> {
    let workspaces = obtain_workspace_infos(repo, entrypoint_ref.map(|rn| rn.as_ref()), meta)?;
    let tip_ref_matches_ws_ref = workspaces
        .iter()
        .find_map(|(ws_tip, ws_rn, _)| (Some(ws_rn) == entrypoint_ref).then_some(ws_tip));

    let mut seeds = Vec::new();
    let mut workspace_metas = Vec::new();
    // The metadata's target commit only provides target context for workspaces.
    let metadata_target_commit_id = if workspaces.is_empty() {
        None
    } else {
        project_meta.target_commit_id
    };
    let mut queued_ids = Vec::new();

    match tip_ref_matches_ws_ref {
        None => {
            // We don't name the seed of the entrypoint as we want the segment
            // naming to be handled by seeds created from metadata.
            seeds.push(Seed::entrypoint(entrypoint, None));
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
        seeds.push(
            Seed::new(ws_tip)
                .with_ref_name(Some(ws_ref.clone()))
                .with_role(SeedRole::Workspace)
                .with_metadata(SegmentMetadata::Workspace(ws_meta.clone()))
                .with_is_entrypoint(Some(&ws_ref) == entrypoint_ref),
        );

        let target = if let Some((target_ref, target_ref_id, local_info)) =
            workspace_target_tip(repo, project_meta.target_ref.as_ref())?
        {
            let local_info =
                local_info.filter(|(_local_ref_name, local_tip)| !queued_ids.contains(local_tip));
            seeds.push(
                Seed::new(target_ref_id)
                    .with_ref_name(Some(target_ref))
                    .with_role(SeedRole::TargetRemote),
            );
            if let Some((local_ref_name, local_tip)) = local_info.clone() {
                seeds
                    .push(Seed::new(local_tip).with_role(SeedRole::TargetLocal { local_ref_name }));
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
        push_integrated_seed_once(&mut seeds, extra_target);
    }

    if let Some(target_commit_id) = metadata_target_commit_id {
        // Metadata may be stale — the commit might not exist (anymore). Ignore if that's the case.
        if let Err(err) = repo.find_commit(target_commit_id) {
            tracing::warn!(
                ?target_commit_id,
                ?err,
                "Ignoring stale target commit id as it didn't exist"
            );
        } else {
            push_integrated_seed_once(&mut seeds, target_commit_id);
        }
    }

    // In ad-hoc/single-branch mode the persisted branch order plays the role workspace
    // metadata plays for managed stacks: its members must start segments so the ordered
    // chain can be planned over boundaries instead of repaired after the fact. Seed them
    // like stack branches — reversed, so when several ordered refs share one tip, the
    // run's bottom-most ref (the commit owner) provides the segment name.
    if let Some(entrypoint_ref) =
        entrypoint_ref.filter(|r| r.category() == Some(gix::reference::Category::LocalBranch))
        && let Some(branch_order) = meta.branch_stack_order(entrypoint_ref.as_ref())?
    {
        for branch in branch_order.into_iter().rev() {
            if branch.category() != Some(gix::reference::Category::LocalBranch) {
                continue;
            }
            let Some(tip) = repo
                .try_find_reference(branch.as_ref())?
                .map(|mut r| r.peel_to_id())
                .transpose()?
            else {
                continue;
            };
            push_seed_once(
                &mut seeds,
                Seed::new(tip.detach()).with_role(SeedRole::WorkspaceStackBranch {
                    desired_ref_name: branch,
                }),
            );
        }
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
            push_seed_once(
                &mut seeds,
                Seed::new(segment_tip.detach()).with_role(SeedRole::WorkspaceStackBranch {
                    desired_ref_name: segment.ref_name,
                }),
            );
        }
    }

    Ok(seeds)
}

fn push_integrated_seed_once(seeds: &mut Vec<Seed>, id: gix::ObjectId) {
    let seed = Seed::new(id).with_role(SeedRole::TargetRemote);
    push_seed_once(seeds, seed);
}

fn push_seed_once(seeds: &mut Vec<Seed>, seed: Seed) {
    if !seeds
        .iter()
        .any(|existing| seeds_have_same_traversal(existing, &seed))
    {
        seeds.push(seed);
    }
}

/// Resolve a workspace target ref and, when possible, its local tracking branch
/// tip.
pub(crate) fn workspace_target_tip(
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

/// Remote target refs already represented by initial seeds, so
/// remote-tracking discovery won't queue them again. Workspace traversals
/// take these from the project metadata target ref; explicit traversals fall
/// back to named integrated seeds when `include_integrated_seed_refs` is set.
fn target_refs_from_seeds(
    seeds: &[Seed],
    project_meta: &ProjectMeta,
    include_integrated_seed_refs: bool,
) -> Vec<gix::refs::FullName> {
    let has_workspace_metadata_seed = seeds
        .iter()
        .any(|seed| matches!(seed.metadata, Some(SegmentMetadata::Workspace(_))));
    let mut target_refs: Vec<_> = seeds
        .iter()
        .filter(|seed| include_integrated_seed_refs && seed.role.is_integrated())
        .filter_map(|seed| seed.ref_name.clone())
        .chain(
            has_workspace_metadata_seed
                .then(|| project_meta.target_ref.clone())
                .flatten(),
        )
        .collect();
    target_refs.sort();
    target_refs.dedup();
    target_refs
}

/// Infer target remote/local tracking links without exposing correlation ids
/// on public seeds: a named [`SeedRole::TargetRemote`] pairs with the
/// [`SeedRole::TargetLocal`] whose ref is configured to track it. If either
/// side is absent, no sibling link is prepared up front.
fn target_local_links_from_seeds(repo: &OverlayRepo<'_>, seeds: &[Seed]) -> TargetLocalLinks {
    let remote_target_refs: Vec<_> = seeds
        .iter()
        .filter(|seed| matches!(seed.role, SeedRole::TargetRemote))
        .filter_map(|seed| seed.ref_name.clone())
        .collect();
    let local_refs: BTreeSet<_> = seeds
        .iter()
        .filter_map(|seed| match &seed.role {
            SeedRole::TargetLocal { local_ref_name } => Some(local_ref_name.clone()),
            SeedRole::Reachable
            | SeedRole::Workspace
            | SeedRole::WorkspaceStackBranch { .. }
            | SeedRole::TargetRemote => None,
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

/// Collect symbolic remote names implied by seed refs, workspace target refs,
/// workspace `push_remote` settings, and stack branch refs.
fn symbolic_remote_names_from_seeds(
    repo: &OverlayRepo<'_>,
    seeds: &[Seed],
    project_meta: &ProjectMeta,
    include_seed_refs: bool,
) -> Vec<String> {
    let remote_names = repo.remote_names();
    let refs = seeds
        .iter()
        .filter_map(|seed| {
            include_seed_refs
                .then_some(seed.ref_name.as_ref())
                .flatten()
        })
        .filter_map({
            let remote_names = &remote_names;
            move |ref_name| {
                extract_remote_name_and_short_name(ref_name.as_ref(), remote_names)
                    .map(|(remote, _short_name)| (1, remote))
            }
        });
    let workspace_metadata_names = seeds
        .iter()
        .filter_map(|seed| match seed.metadata.as_ref() {
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
    let desired_refs = seeds.iter().filter_map(|seed| match &seed.role {
        _ if !include_seed_refs => None,
        SeedRole::WorkspaceStackBranch { desired_ref_name } => {
            extract_remote_name_and_short_name(desired_ref_name.as_ref(), &remote_names)
                .map(|(remote, _short_name)| (1, remote))
        }
        SeedRole::Reachable
        | SeedRole::Workspace
        | SeedRole::TargetLocal { .. }
        | SeedRole::TargetRemote => None,
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

/// The second half of the ordering heuristic: `seeds_in_queue_order()` fixes
/// segment creation order, but some roles must also be *visited* first so
/// they own shared commits.
///
/// Synthetic integrated seeds always front-load (limits, not user roots);
/// workspace, target, and target-local seeds front-load so target ownership
/// and sibling links settle before stack branches can claim shared commits.
/// Stack branches deliberately are not front-loaded — their traversal work
/// should follow the workspace/target context.
fn queue_should_frontload_seed(
    seed: &Seed,
    frontload_workspace_related_seeds: bool,
    auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
) -> bool {
    seed.is_auxiliary_integrated_seed(auxiliary_integrated_seed_ids)
        || (frontload_workspace_related_seeds
            && matches!(
                seed.role,
                SeedRole::Workspace | SeedRole::TargetRemote | SeedRole::TargetLocal { .. }
            ))
}

/// Return the flags and limit used by a reachable seed seeking the entrypoint.
fn reachable_seed_flags_and_limit(
    seed: gix::ObjectId,
    entrypoint: gix::ObjectId,
    max_limit: Limit,
    goals: &mut Goals,
) -> (CommitFlags, Limit) {
    let limit = if seed == entrypoint {
        max_limit
    } else {
        max_limit.with_indirect_goal(entrypoint, goals)
    };
    (CommitFlags::NotInRemote, limit)
}
