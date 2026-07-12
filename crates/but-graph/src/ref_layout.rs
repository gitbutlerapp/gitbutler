//! The stored REF LAYOUT — and a field guide to how this codebase talks about refs.
//!
//! The problem in one sentence: git stores refs as a flat `name → commit` list; the
//! workspace needs *structure over them* — which refs form a stack together, in what
//! vertical order, and where branches with no commits of their own live. Ref management
//! declares that structure, places it onto the commit graph, and keeps it intact while
//! commits are rewritten underneath. This module holds the placed result: authored by the
//! build, stored on the [`CommitGraph`](crate::CommitGraph), read by the projection to
//! carve stacks, and ingested by the rebase editor as an id-mapping copy.
//!
//! # One example
//!
//! Two stacks, five refs:
//!
//! ```text
//!         M  (workspace merge commit)
//!        ╱ ╲
//!      A2   B1   ►feature-b
//!      A1   ╱
//!       ╲  ╱
//!       base     ►main
//! ```
//!
//! Refs on disk: `feature-a → A2`, `review-a → A2`, `feature-b → B1`, `fresh → base`,
//! `main → base`. The flat store cannot express what a user plainly sees: that `review-a`
//! is an *empty branch stacked on* `feature-a` (two names for A2), that `fresh` is its
//! *own empty stack* (just another name for base), and that `main` is *not a stack at all*.
//! Workspace metadata declares the missing structure, one ref chain per stack:
//! `[review-a, feature-a]`, `[feature-b]`, `[fresh]`.
//!
//! # The store and the queries
//!
//! The layout stores decisions only — [`RefGroup`](crate::ref_layout::RefGroup)s per commit plus per-reference
//! [`RefFacts`](crate::ref_layout::RefFacts) — and *nothing derived*:
//!
//! ```text
//! on A2:  RefGroup { members: [feature-a, review-a], carry: All }
//! on B1:  RefGroup { members: [feature-b],           carry: All }
//! ```
//!
//! Per-reference positions are *questions*, never a second filing system:
//! [`RefLayout::positioned_on`](crate::ref_layout::RefLayout::positioned_on), [`RefLayout::below_of`](crate::ref_layout::RefLayout::below_of), [`RefLayout::facts_of`](crate::ref_layout::RefLayout::facts_of),
//! [`RefLayout::placements`](crate::ref_layout::RefLayout::placements). Think shelves vs questions at the desk: a group is a shelf
//! (the stack of refs on one commit, bottom→top); "where is this ref?" is answered by
//! looking at the shelves. Nothing derived is stored, so nothing derived can go stale.
//!
//! The split serves the two consumers' opposite questions. The editor asks the *shelf*
//! question — mutations move whole stacks of refs between commits — so groups are the
//! stored truth, and the editor's mutation store is the *same* [`RefGroup`](crate::ref_layout::RefGroup) type over its
//! pick handles (ingest maps ids, copies the rest). The projection asks the *per-ref*
//! questions, with the same query vocabulary on both sides of that boundary. A group's
//! [`GroupCarry`](crate::ref_layout::GroupCarry) answers the one further thing flat refs can't: which of the commit's
//! incoming edges enter *through* these refs — so when commits are inserted or removed
//! around A2, the editor knows those refs guard the A-side of the merge, not B's.
//!
//! # Where the commit-less chains rest
//!
//! The chain `[fresh]` has nothing to place — its branches simply *rest on* an existing
//! commit. [`RefLayout::empty_chain_anchors`](crate::ref_layout::RefLayout::empty_chain_anchors) lists that resting commit per such chain,
//! and [`EmptyChainAnchor::joins_owning_chain`](crate::ref_layout::EmptyChainAnchor::joins_owning_chain) distinguishes the two possibilities:
//! the commit is already displayed inside another chain's run (the empties splice into
//! that stack), or no run displays it — then only this chain makes the commit part of the
//! workspace at all, and the frame counts the anchor as a workspace tip of its own.
//! Chains *with* commits are placed as runs and need no anchor.
//!
//! # The lifecycle
//!
//! ```text
//!  metadata ──declares──▶ RefChain        (one stack's ordered branch list)
//!      ▲                      │
//!      │                      ▼  the build plans from chains + observed refs
//!  the walk ──observes──▶ RefInfo         (a ref seen at a commit)
//!      ▲                      │
//!      │                      ▼  decisions, stored on the CommitGraph
//!      │                  RefLayout
//!      │                      ├─ RefGroup<ObjectId> + GroupCarry   ◀── THE stored truth
//!      │                      ├─ RefFacts (per name)
//!      │                      └─ queried: positioned_on · below_of · facts_of
//!      │                      │
//!      │                      ▼  ingest = id-mapping copy (ObjectId → PickIndex)
//!      │                  RefGroup<PickIndex> + GroupCarry     ◀── the editor's store
//!      │                      └─ RefState (per name: mutable, live, ambiguous)
//!      │                      │
//!      └── materialize ───────┘  refs written to disk; the next walk observes them
//! ```
//!
//! *Declared* as ref chains, *observed* as `RefInfo`s, *decided* into groups, *edited* as
//! those same groups over pick handles — with positions always asked, never stored.
//!
//! # Why it is the way it is
//!
//! - **Names are the member identity**: a rebase rewrites every commit id under a ref,
//!   but the ref is still `feature-a`. Names never churn while commits do.
//! - **Stored fields are decisions, not caches.** Each was checked to be non-derivable:
//!   `RefFacts::ambiguous` records a convergence visible only while deriving;
//!   `RefFacts::reachable` records which segments the entrypoint's descent actually entered —
//!   two layouts can look identical and still differ here; `joins_owning_chain` is the plan's
//!   run-ownership call, and re-deriving it would need the workspace bound, which is computed
//!   FROM these anchors.
//! - **Remote-tracking links stay out**: they are disk-derived enrichment, not placement
//!   decisions.

/// How much of its commit's incoming edges a reference group carries — which edges ENTER
/// THROUGH the group's references. Generic over the id space: the graph stores commit ids,
/// the rebase editor stores the same shape over its pick handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupCarry<Id> {
    /// Nothing descends into this group (a root group: remote above a tip, empty top).
    None,
    /// Every edge into the commit descends through this group (a plain stack, or the shared
    /// group all merge groups converge on).
    All,
    /// Exactly these stated edges — one group of a merge. Kept sorted and deduplicated,
    /// keyed by the full `(child, parent number)` edge: two distinct sources can feed one
    /// commit at the same parent number (and one source at two parent numbers), so both coordinates are
    /// needed.
    Edges(Vec<(Id, usize)>),
}

/// One group of references standing on a commit: an ordered bottom→top run of member NAMES
/// sharing one [`GroupCarry`] — THE shared shape of the display↔rebase boundary. The builder
/// authors it, the projection derives its row view from it, and the rebase editor ingests
/// it as an id-mapping copy. Order and stacking are LIST STRUCTURE: a member's below is the
/// previous member, its rank its index plus the height of the attach walk underneath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefGroup<Id> {
    /// The reference names, bottom→top: `members[0]` sits on `attach` (or the commit).
    pub members: Vec<gix::refs::FullName>,
    /// How much of the commit's edges this group carries.
    pub carry: GroupCarry<Id>,
    /// The reference this group's bottom member sits on — a member of ANOTHER group on the
    /// same commit (`None` = directly on the commit). Group boundaries are carry
    /// boundaries: a branch point or a carry change starts a new group.
    pub attach: Option<gix::refs::FullName>,
}

/// The graph-side facts of one reference — everything the projection needs beyond the
/// structural groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefFacts {
    /// Whether the entrypoint reaches this ref's segment — mutability before the editor's
    /// category gates.
    pub reachable: bool,
    /// Whether this ref NAMES its segment — the disambiguation winner — rather than riding
    /// passively on a commit.
    pub names_segment: bool,
    /// For naming refs: whether the named segment owns no commits.
    pub names_empty_segment: bool,
    /// Whether several edges converge right above the ref — a preserved creation-time
    /// signal, never re-derived.
    pub ambiguous: bool,
}

/// Where one empty chain rests — a declared chain with no commits of its own only exists
/// as branches sitting on some commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyChainAnchor {
    /// The commit the chain's branches sit on.
    pub commit: gix::ObjectId,
    /// Whether that commit is already shown inside another chain's run — the empties then splice
    /// into that stack. When `false`, only this chain's wiring makes the commit part of
    /// the workspace at all, so the frame counts it as a workspace tip of its own.
    pub joins_owning_chain: bool,
}

/// The workspace commit and its materialized parent list — one parent per ref chain,
/// so empty chains over one base yield duplicate entries the real commit does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedWsParents {
    /// The managed workspace commit.
    pub commit: gix::ObjectId,
    /// Its parents in chain order.
    pub parents: Vec<gix::ObjectId>,
}

/// The ref layout: the stored reference groups per commit plus per-reference facts and the
/// workspace anchors. Everything here is something the build DECIDED; nothing is a value
/// you could recompute from the other fields — those are answered by the queries
/// ([`Self::positioned_on`], [`Self::below_of`], [`Self::facts_of`], [`Self::placements`]),
/// so a stored copy can never go stale or disagree. The two fields that look derivable
/// aren't: [`Self::reachable_commits`] follows the plan's segment wiring (empty chains
/// included), not the arena's edges, and [`Self::head_refs`] records the naming decision,
/// not the refs at a commit.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RefLayout {
    /// THE stored structure: per commit, the ordered reference groups standing on it —
    /// the exact shape the rebase editor stores over its own handles. Commits appear in
    /// first-namer order (the order their first ref holds in [`Self::facts`]).
    pub groups: Vec<(gix::ObjectId, Vec<RefGroup<gix::ObjectId>>)>,
    /// Per-reference facts, in reference-table order — the order fixes the editor's
    /// reference table (and with it render sibling order). Unborn refs (no position) are
    /// here too, absent from [`Self::groups`].
    pub facts: Vec<(gix::refs::FullName, RefFacts)>,
    /// See [`MaterializedWsParents`]; `None` without a managed entrypoint commit.
    pub materialized_ws_parents: Option<MaterializedWsParents>,
    /// The entrypoint's ref names — the editor's HEAD checkouts.
    pub head_refs: Vec<gix::refs::FullName>,
    /// Commits reachable from the entrypoint (sorted) — the editor's mutable commits.
    pub reachable_commits: Vec<gix::ObjectId>,
    /// Where the empty chains rest: for each declared chain with no commits of its own,
    /// the commit its branches sit on, in metadata order.
    pub empty_chain_anchors: Vec<EmptyChainAnchor>,
}

/// One reference's authoring input to [`RefLayout::from_parts`]: its facts, and — when
/// positioned — its placement.
pub(crate) struct RefPart {
    pub name: gix::refs::FullName,
    pub facts: RefFacts,
    pub placement: Option<Placement>,
}

/// A positioned reference's placement: the commit it stands on, the NAME of the reference
/// below it, and its entering edges (sorted).
pub(crate) struct Placement {
    pub on: gix::ObjectId,
    pub below: Option<gix::refs::FullName>,
    pub entering: Vec<(gix::ObjectId, usize)>,
}

impl RefLayout {
    /// Author a layout from the builder's parts: per reference (table order) its facts and
    /// resolved placement (`on`, the NAME below it, its entering edges) — groups authored
    /// directly, the position view derived from them. `incoming` answers a commit's full
    /// connected incoming edge set, `(child, parent number)` sorted — the [`GroupCarry::All`]
    /// classification and expansion oracle.
    pub(crate) fn from_parts(
        parts: Vec<RefPart>,
        incoming: &dyn Fn(gix::ObjectId) -> Vec<(gix::ObjectId, usize)>,
    ) -> Self {
        // Group placements exactly like the editor's place(): in table order, a ref joins
        // the group whose top member is its below and whose carry equals its own — else it
        // starts a fresh group attached there. Coalescing keeps groups canonical maximal
        // same-carry runs.
        //
        // One name = one reference is the name-keyed contract; the builder authors each
        // name at most once.
        let mut groups: Vec<(gix::ObjectId, Vec<RefGroup<gix::ObjectId>>)> = Vec::new();
        let mut grouped = std::collections::HashSet::new();
        for part in &parts {
            let (name, placement) = (&part.name, &part.placement);
            debug_assert!(
                grouped.insert(name),
                "one name = one reference: '{name}' was authored twice"
            );
            let Some(Placement {
                on,
                below,
                entering,
            }) = placement
            else {
                continue;
            };
            let carry = if entering.is_empty() {
                GroupCarry::None
            } else if *entering == incoming(*on) {
                GroupCarry::All
            } else {
                GroupCarry::Edges(entering.clone())
            };
            let attach = below.clone();
            let at = match groups.iter().position(|(id, _)| id == on) {
                Some(at) => at,
                None => {
                    groups.push((*on, Vec::new()));
                    groups.len() - 1
                }
            };
            place_in_groups(&mut groups[at].1, name.clone(), carry, attach);
        }
        RefLayout {
            groups,
            facts: parts
                .into_iter()
                .map(|part| (part.name, part.facts))
                .collect(),
            ..Default::default()
        }
    }

    /// The facts of the reference `name`, if the layout knows it.
    pub fn facts_of(&self, name: &gix::refs::FullNameRef) -> Option<&RefFacts> {
        self.facts
            .iter()
            .find(|(n, _)| n.as_ref() == name)
            .map(|(_, facts)| facts)
    }

    /// The reference directly underneath `name` in its group's physical stack — the
    /// previous member, or the group's attach for a bottom member; `None` when it sits
    /// directly on its commit (or holds no placement at all).
    pub fn below_of(&self, name: &gix::refs::FullNameRef) -> Option<&gix::refs::FullName> {
        self.groups.iter().find_map(|(_, commit_groups)| {
            commit_groups.iter().find_map(|group| {
                let i = group.members.iter().position(|m| m.as_ref() == name)?;
                match i {
                    0 => group.attach.as_ref(),
                    _ => Some(&group.members[i - 1]),
                }
            })
        })
    }

    /// The commit the reference `name` stands on — `None` for unborn refs (and unknown
    /// names), which keep no stored placement.
    pub fn positioned_on(&self, name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        self.placements()
            .find(|(n, _)| n.as_ref() == name)
            .map(|(_, on)| on)
    }

    /// Every positioned reference with the commit it stands on, in group order (per commit
    /// bottom→top). One entry per name — the layout's name-keyed contract.
    pub fn placements(&self) -> impl Iterator<Item = (&gix::refs::FullName, gix::ObjectId)> {
        self.groups.iter().flat_map(|(on, commit_groups)| {
            commit_groups
                .iter()
                .flat_map(move |group| group.members.iter().map(move |name| (name, *on)))
        })
    }

    /// [`Self::placements`] narrowed to segment-NAMING references (facts say
    /// [`RefFacts::names_segment`]) — the placements that are structure, never riders.
    pub fn segment_naming_placements(
        &self,
    ) -> impl Iterator<Item = (&gix::refs::FullName, gix::ObjectId)> {
        let naming: std::collections::HashSet<&gix::refs::FullNameRef> = self
            .facts
            .iter()
            .filter(|(_, facts)| facts.names_segment)
            .map(|(name, _)| name.as_ref())
            .collect();
        self.placements()
            .filter(move |(name, _)| naming.contains(name.as_ref()))
    }
}

/// Place `name` into `groups`: onto `attach`'s group when it lands on a group top with
/// the same carry, as a fresh group otherwise — then re-coalesce. THE placement
/// algorithm of the name-keyed store, shared by the builder's authoring and the rebase
/// editor's mutations (one implementation on both sides of the boundary).
pub fn place_in_groups<Id: PartialEq>(
    groups: &mut Vec<RefGroup<Id>>,
    name: gix::refs::FullName,
    carry: GroupCarry<Id>,
    attach: Option<gix::refs::FullName>,
) {
    let joined = attach.as_ref().and_then(|b| {
        groups
            .iter()
            .position(|group| group.members.last() == Some(b) && group.carry == carry)
    });
    match joined {
        Some(g) => groups[g].members.push(name),
        None => groups.push(RefGroup {
            members: vec![name],
            carry,
            attach,
        }),
    }
    coalesce_groups(groups);
}

/// Merge groups that stand contiguously with the same carry: a group attached to the TOP
/// member of another group with an equal carry is its continuation — group boundaries stay
/// canonical maximal same-carry runs.
pub fn coalesce_groups<Id: PartialEq>(groups: &mut Vec<RefGroup<Id>>) {
    loop {
        let merge = groups.iter().enumerate().find_map(|(upper_idx, upper)| {
            let b = upper.attach.as_ref()?;
            groups.iter().enumerate().find_map(|(lower_idx, lower)| {
                (lower_idx != upper_idx
                    && lower.members.last() == Some(b)
                    && lower.carry == upper.carry)
                    .then_some((lower_idx, upper_idx))
            })
        });
        let Some((lower_idx, upper_idx)) = merge else {
            break;
        };
        let upper = groups.remove(upper_idx);
        let lower_idx = lower_idx - usize::from(upper_idx < lower_idx);
        groups[lower_idx].members.extend(upper.members);
    }
}
