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
//!
//! ## How the walk decides to stop
//!
//! Two different questions end a traversal, and they must not be confused:
//!
//! **Convergence** is the correctness question: have the seeds met? Every non-entrypoint
//! seed is given a GOAL — a flag bit for the commit it must connect to (its pair: the
//! entrypoint for branches and the target, the local for a remote) — and a tip with an
//! outstanding goal walks with unlimited gas until it steps on territory carrying that
//! bit. While any goal is outstanding, goalless tips ride along free so the flags they
//! propagate can be found. Once every queued tip is inside target territory with its
//! goals met, the walk is converged and the queue is exhausted — unless the entrypoint
//! itself is integrated, in which case stopping would show an empty view.
//!
//! **Pagination** is the display question: how much history below the floor — the
//! workspace↔target merge-base — should be materialized? That is
//! [`Options::commits_limit_hint`](crate::walk::Options::commits_limit_hint): a per-tip budget, split across merge parents,
//! refilled at [`Options::commits_limit_recharge_location`](crate::walk::Options::commits_limit_recharge_location), and handed off at
//! re-encounters (a tip lends its remaining BUDGET, never its goals, to the
//! continuations of the cone it proved reachable). The budget never buys convergence
//! and goals never buy display depth. [`Options::hard_limit`](crate::walk::Options::hard_limit) caps total queuing as a
//! runaway backstop.
//!
//! Two deliberate exceptions complete the picture. A target's LOCAL that is proven a
//! strict ancestor of the target (a generation-cutoff ancestry check, one commit-graph
//! walk) is recorded on its [`Seed`](crate::walk::Seed) as `behind_target` and never queued at all: its
//! convergence point is its own tip, a fact that needs no walk, and walking to it would
//! drag the traversal as far below the base as the local is stale. And an EXPLICIT
//! [`Options::extra_target_commit_id`](crate::walk::Options::extra_target_commit_id) is the opposite request — extend the view down
//! to this old target — so the entrypoint seeks it as a goal, unless a zero budget
//! (tips only) outranks it.

use std::collections::BTreeMap;

use but_core::{
    RefMetadata,
    ref_metadata::{self, ProjectMeta},
};
use gix::prelude::{ObjectIdExt, ReferenceExt};
use tracing::instrument;

use crate::workspace::GraphContext;

pub(crate) mod utils;
use utils::*;

pub(crate) mod assemble;
pub use assemble::seeds_from_workspace_metadata;
mod seed;
pub use seed::{Seed, SeedRole};

pub(crate) mod types;

mod remotes;

pub(crate) mod overlay;
pub(crate) mod walker;

pub(crate) type Entrypoint = Option<(gix::ObjectId, Option<gix::refs::FullName>)>;

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
    /// Extra reachable tips the caller resolved from linked-worktree `HEAD`s.
    ///
    /// The graph learns WHICH worktree checks out a ref by reading the repository, but not
    /// which worktrees the caller considers active — archived ones are the caller's state,
    /// not Git's. So the caller decides membership and passes the tips in here.
    ///
    /// A tip with a ref name is re-resolved through the (possibly overlaid) ref store on
    /// every traversal, so redone traversals see moved refs; the recorded commit id is only
    /// a fallback for detached worktrees. A ref that no longer resolves is skipped rather
    /// than resurrected from its stale tip. They seed last and are skipped when another
    /// seed already covers their commit.
    pub worktree_tips: Vec<WorktreeTip>,
}

/// A linked-worktree `HEAD` to include as an extra traversal tip, see
/// [`Options::worktree_tips`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeTip {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    ///
    /// Traversal does not consume it; it travels with the graph for consumers that need to
    /// identify the linked worktree.
    pub name: bstr::BString,
    /// The branch the worktree has checked out, if its `HEAD` is symbolic.
    pub ref_name: Option<gix::refs::FullName>,
    /// The peeled `HEAD` commit at caller resolution time.
    pub id: gix::ObjectId,
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
        // The dispatch lives in `from_tip`, which the detached and symbolic cases below
        // delegate to (an unborn `HEAD` has no commit to walk and returns its graph from the
        // arm itself): a checkout inside a managed workspace — including HEAD on the workspace
        // ref itself — builds via the managed builder, everything else via the non-managed one.
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
                let segment =
                    resolve_ref_and_meta(Some((ref_name, None)), &meta, None, &wt_by_branch)?;
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

    /// The tip dispatch `from_tip` and [`Workspace::rederive_with`](crate::Workspace::rederive_with) share:
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
