//! The seed: what a traversal may start from, and the rules a seed set must obey.
//!
//! A [`Seed`] is a resolved commit with a [`SeedRole`] deciding its initial flags and
//! pacing; the roles and their goals are explained in the parent module's "How the walk
//! decides to stop". Validation guards caller-provided seed sets; the push helpers keep
//! seed lists free of duplicate traversals.

use anyhow::{Context as _, bail, ensure};

use std::collections::BTreeSet;

use crate::SegmentMetadata;
use crate::walk::overlay::OverlayRepo;

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
    pub(super) fn is_anonymous_integrated_target_context(&self) -> bool {
        matches!(self.role, SeedRole::TargetRemote) && self.ref_name.is_none()
    }

    /// Whether this anonymous integrated target was derived by normalization
    /// (from metadata or `extra_target_commit_id`) rather than passed by the
    /// caller. Such seeds are limits/context and get ordered and deduplicated
    /// as auxiliary work, not as user-visible roots.
    pub(super) fn is_auxiliary_integrated_seed(
        &self,
        auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
    ) -> bool {
        self.is_anonymous_integrated_target_context()
            && auxiliary_integrated_seed_ids.contains(&self.id)
    }

    /// Whether a named target ref already covers this anonymous target's
    /// commit. Keeping both would let the anonymous seed own the commit and
    /// leave the named ref as a duplicate empty segment.
    pub(super) fn collapses_into_named_integrated_target(
        &self,
        named_integrated_target_ids: &BTreeSet<gix::ObjectId>,
    ) -> bool {
        self.is_anonymous_integrated_target_context()
            && named_integrated_target_ids.contains(&self.id)
    }
}

/// The role a resolved traversal seed plays when constructing a graph.
///
/// Roles decide the initial [`CommitFlags`](crate::CommitFlags) and `Limit` goals used by the
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
    /// This seed marks all commits it traverses with [`CommitFlags::NotInRemote`](crate::CommitFlags::NotInRemote).
    #[default]
    Reachable,
    /// The workspace ref itself, paired with workspace metadata on [`Seed`].
    ///
    /// This marks commits as in-workspace with [`CommitFlags::InWorkspace`](crate::CommitFlags::InWorkspace).
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
    /// This seed receives [`CommitFlags::Integrated`](crate::CommitFlags::Integrated) and an indirect goal for
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
        /// The local is a PROVEN strict ancestor of the target: its convergence
        /// point is its own tip, a fact that needs no walk. Such a seed is
        /// carried as data — readers may resolve the ref to [`Seed::id`] — but
        /// it is never queued and never pairs goals: walking to it would drag
        /// the traversal as far below the base as the local is stale, and
        /// nothing above the base reads what that walk finds.
        behind_target: bool,
    },
}

/// Access
impl SeedRole {
    /// Whether this role represents integrated history.
    pub fn is_integrated(&self) -> bool {
        matches!(self, SeedRole::TargetRemote)
    }
}

/// Validate caller-provided seeds: exactly one entrypoint, no duplicate
/// traversal seeds or ref names, detached entrypoints unnamed, and every ref
/// name resolving to its seed's commit.
pub(super) fn validate_explicit_seeds<'a>(
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

pub(super) fn push_integrated_seed_once(seeds: &mut Vec<Seed>, id: gix::ObjectId) {
    let seed = Seed::new(id).with_role(SeedRole::TargetRemote);
    push_seed_once(seeds, seed);
}

pub(super) fn push_seed_once(seeds: &mut Vec<Seed>, seed: Seed) {
    if !seeds
        .iter()
        .any(|existing| seeds_have_same_traversal(existing, &seed))
    {
        seeds.push(seed);
    }
}
