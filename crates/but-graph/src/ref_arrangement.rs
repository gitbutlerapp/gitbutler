//! The metadata-driven REF PLACEMENT table: which references sit on which commit, grouped
//! and ordered, one chain per metadata stack list.
//!
//! Authored by the builder's chain plan and stored on the [`CommitGraph`](crate::CommitGraph),
//! so the placement decisions survive the build instead of dying with it. The segment builder's
//! chain-structure pass consumes the table directly — the parity oracle next to `chain_plan`
//! keeps proving it round-trips the plan — and adoption by the rebase editor and the
//! projection follows in later stages.
//!
//! Remote-tracking references stay OUT of the table: they are disk-derived enrichment, not
//! placement decisions.

use std::collections::HashMap;

/// How a reference group lands relative to the commit it anchors on.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GroupPlacement {
    /// The group's commit is inside another chain: the empties splice into that chain.
    Dependent,
    /// The group anchors its own chain from the workspace (shared base or integrated anchor).
    OwnChain,
    /// Another chain owns the (non-integrated) commit: the refs stay passive on it.
    Passive,
    /// The group is outside the workspace or co-located with a managed merge commit — nothing
    /// is created. Kept so group ordinals stay aligned between plan and build.
    Skipped,
}

/// The group member that NAMES the anchor commit's segment.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GroupNamer {
    /// The naming reference.
    pub name: gix::refs::FullName,
    /// The metadata-order override: this namer displaced a build-time name belonging to the
    /// group, whose remote link moves to its floated empty segment instead.
    pub clear_remote: bool,
}

/// One same-commit group of references anchored on a commit.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ArrangedGroup {
    /// The member naming the anchor commit's segment, when the group names it at all.
    pub namer: Option<GroupNamer>,
    /// The members that become empty segments spliced above the anchor, in metadata order.
    pub empties: Vec<gix::refs::FullName>,
    /// How the group lands.
    pub placement: GroupPlacement,
}

/// One metadata stack list's groups, in metadata order (top → bottom). Each anchor is
/// `(commit, index into RefArrangement::at_commit[commit])` — the index keeps chains
/// apart when several chains anchor groups on the same commit.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Chain {
    /// The chain's anchors in metadata order.
    pub anchors: Vec<(gix::ObjectId, usize)>,
}

/// The ref placement table.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RefArrangement {
    /// The groups anchored on each commit, in chain-threading order.
    pub at_commit: HashMap<gix::ObjectId, Vec<ArrangedGroup>>,
    /// One chain per metadata stack list, in metadata order.
    pub chains: Vec<Chain>,
    /// Commits whose build-time name is suppressed (sorted): a shared base stays anonymous
    /// while every chain's branches float above it as their own chain.
    pub demoted: Vec<gix::ObjectId>,
    /// The DERIVED editor-grade layout over the full ref universe (see [`RefPositions`]).
    /// `None` until the assembler authors it from the finished segment graph.
    pub positions: Option<RefPositions>,
}

/// EVERY reference the workspace surfaces — chain names, empties, floats, remote and target
/// names, passive commit refs — with its resolved position over the commit graph, in segment
/// order. Authored from the FINISHED segment graph (and retiring with it once a commit-graph
/// native derivation exists); consumed by the rebase editor, which translates it 1:1 into its
/// reference table instead of re-deriving positions from segment topology.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RefPositions {
    /// The references in segment order — the order fixes the editor's reference table (and
    /// with it render sibling order).
    pub refs: Vec<PositionedRef>,
    /// The managed entrypoint commit and its resolved CHAIN slots, one per chain top→bottom —
    /// empty chains over one base yield duplicate entries the real commit does not have.
    /// `None` without a managed entrypoint commit.
    pub ws_chain_slots: Option<(gix::ObjectId, Vec<gix::ObjectId>)>,
    /// Ordinals (into [`Self::refs`]) of the entrypoint's ref — the editor's HEAD checkouts.
    pub head_refs: Vec<usize>,
    /// Commits reachable from the entrypoint (sorted) — the editor's mutable commits.
    pub reachable_commits: Vec<gix::ObjectId>,
}

/// One reference of [`RefPositions`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PositionedRef {
    /// The reference name.
    pub name: gix::refs::FullName,
    /// Whether the entrypoint reaches this ref's segment — mutability before the editor's
    /// category gates (remote-category refs are never mutable).
    pub reachable: bool,
    /// Where the ref sits. `None` for unborn refs, which keep no stored position.
    pub position: Option<RefPosition>,
}

/// A reference's resolved position over the commit graph.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RefPosition {
    /// The commit the ref sits on.
    pub on: gix::ObjectId,
    /// The ordinal (into [`RefPositions::refs`]) of the next ref BELOW this one on the same
    /// commit run, when any.
    pub below: Option<usize>,
    /// The edges entering the ref from above, sorted: `(child commit, parent slot)` — the
    /// child reaches its parent slot's commit through this ref.
    pub entering: Vec<(gix::ObjectId, usize)>,
    /// Whether several edges converge right above the ref.
    pub ambiguous: bool,
}
