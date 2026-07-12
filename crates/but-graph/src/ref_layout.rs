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
//! commit indices (ingest maps ids, copies the rest). The projection asks the *per-ref*
//! questions, with the same query vocabulary on both sides of that boundary. A group's
//! [`GroupCarry`](crate::ref_layout::GroupCarry) answers the one further thing flat refs can't: which of the commit's
//! incoming parent entries enter *through* these refs — so when commits are inserted or removed
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
//!      │                      ├─ RefGroup<ParentIndex> + GroupCarry   ◀── THE stored truth
//!      │                      ├─ RefFacts (per name)
//!      │                      └─ queried: positioned_on · below_of · facts_of
//!      │                      │
//!      │                      ▼  ingest = id-mapping copy (ObjectId → CommitIndex)
//!      │                  RefGroup<ParentEntryId> + GroupCarry     ◀── the editor's store
//!      │                      └─ RefState (per name: mutable, live, ambiguous)
//!      │                      │
//!      └── materialize ───────┘  refs written to disk; the next walk observes them
//! ```
//!
//! *Declared* as ref chains, *observed* as `RefInfo`s, *decided* into groups, *edited* as
//! those same groups over commit indices — with positions always asked, never stored.
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

/// How much of its commit's incoming parent entries a reference group carries — which parent entries ENTER
/// THROUGH the group's references. Generic over the id space: the graph stores commit ids,
/// the rebase editor stores the same shape over its commit indices.
///
/// # The aliasing rule
///
/// `Entries` naming the commit's ENTIRE live parent-entry set resolves the same as `All`,
/// but the two are not interchangeable: `All` keeps carrying entries added later, an `Entries`
/// statement never grows. Every placement door normalizes toward `All` when the stated set
/// covers everything live — the builder's inline classification in `RefLayout::from_parts` at
/// authoring, the editor's `place` on
/// every (re)placement — so settled groups never hold a full-set `Entries` in disguise.
/// Group-merging comparisons ([`place_in_groups`], [`coalesce_groups`]) are SYNTACTIC over
/// those normalized statements: a same-(carry, attach) twin folds onto the existing
/// group's top rather than standing as an unordered position collision. What normalization
/// cannot settle — a carry copied mid-surgery, before its commit's parent entries are final — is the
/// mutation's own job to re-hang once parent entries settle, and anything that slips through is a
/// reported position collision at the editor's well-formedness checks, which compare by
/// RESOLUTION.
/// The parent entry coordinate `E` differs per side: the stored layout names parent entries positionally
/// as `(child, parent number)` ([`ParentIndex`] — positions are stable in a derived,
/// immutable record), while the rebase editor names them by STABLE PARENT-ENTRY IDENTITY
/// (`GroupCarry<ParentEntryId>`) so statements survive parent-array renumbering without rename
/// maintenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupCarry<E> {
    /// Nothing descends into this group (a root group: remote above a tip, empty top).
    None,
    /// Every parent entry into the commit descends through this group (a plain stack, or the shared
    /// group all merge groups converge on) — including parent entries added AFTER this was stated.
    All,
    /// Exactly these stated parent entries — one group of a merge. Kept sorted and deduplicated.
    /// In the stored layout an parent entry is the full `(child, parent number)` pair: two
    /// distinct sources can feed one commit at the same parent number (and one source at
    /// two parent numbers), so both coordinates are needed. Never grows to cover parent entries
    /// added later.
    Entries(Vec<E>),
}

impl<E> GroupCarry<E> {
    /// Convert the parent entry coordinates, keeping the carry intent verbatim.
    pub fn map_entries<F>(self, f: impl FnMut(E) -> F) -> GroupCarry<F> {
        match self {
            GroupCarry::None => GroupCarry::None,
            GroupCarry::All => GroupCarry::All,
            GroupCarry::Entries(entries) => {
                GroupCarry::Entries(entries.into_iter().map(f).collect())
            }
        }
    }
}

/// The STORED layout's parent entry coordinate: `(child commit, parent number)` — positional,
/// which is stable here because the stored record is derived and immutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParentIndex {
    /// The commit whose parent array holds the parent entry.
    pub child: gix::ObjectId,
    /// The position of the parent in `child`'s parent array.
    pub index: usize,
}

/// One group of references standing on a commit: an ordered bottom→top run of member NAMES
/// sharing one [`GroupCarry`] — THE shared shape of the display↔rebase boundary. The builder
/// authors it, the projection derives its row view from it, and the rebase editor ingests
/// it as an id-mapping copy. Order and stacking are LIST STRUCTURE: a member's below is the
/// previous member, its rank its index plus the height of the attach walk underneath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefGroup<E> {
    /// The reference names, bottom→top: `members[0]` sits on `attach` (or the commit).
    pub members: Vec<gix::refs::FullName>,
    /// How much of the commit's parent entries this group carries.
    pub carry: GroupCarry<E>,
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
    /// Whether several parent entries converge right above the ref — a preserved creation-time
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

/// The workspace commit and its AMENDED parent list — the real parents in their
/// authoritative (real) order, with one entry woven in per empty chain at that chain's
/// sort position. Empty branches never exist as actual parents (git cannot repeat a
/// parent, and merge semantics drop a parent contained in another), so their amended
/// entries — duplicates over one base included — are their representation and carry
/// their ordering among the lanes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendedWsParents {
    /// The managed workspace commit.
    pub commit: gix::ObjectId,
    /// Its parents in chain order.
    pub parents: Vec<gix::ObjectId>,
}

/// The insertion-ordered position store: per key, the ordered reference groups standing
/// on it. ONE implementation of the name-keyed table on both sides of the display↔rebase
/// boundary: the builder authors it over commit ids and [`ParentIndex`]s, and the rebase
/// editor mutates the very same structure over its commit indices and stable parent entry ids — so
/// placement, extraction, and splice rules cannot drift between the sides. Sites keep
/// insertion order (the builder's first-namer order, the editor's ingest order), a site
/// vanishes with its last group, and equality is order-sensitive like the rest of the
/// layout record.
///
/// Members are identified BY NAME — one name, one reference, is the table's contract.
/// Lookup is a linear scan by design: R, the refs in a workspace, is human-scale, and
/// the callers that loop it document their own tripwires.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RefGroups<K, E> {
    sites: Vec<Site<K, E>>,
}

/// One site: a key and the reference groups standing on it. Internal shape only — the
/// public surface is the queries and [`RefGroups::iter`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Site<K, E> {
    on: K,
    groups: Vec<RefGroup<E>>,
}

impl<K, E> Default for RefGroups<K, E> {
    fn default() -> Self {
        Self { sites: Vec::new() }
    }
}

impl<K: Copy + PartialEq, E: PartialEq + Clone> RefGroups<K, E> {
    /// The sites in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (K, &[RefGroup<E>])> {
        self.sites
            .iter()
            .map(|site| (site.on, site.groups.as_slice()))
    }

    /// Every group, mutably — for whole-table rewrites (carry re-statements, the debug
    /// anonymizer). Structure stays the caller's responsibility.
    pub fn groups_mut(&mut self) -> impl Iterator<Item = &mut RefGroup<E>> {
        self.sites
            .iter_mut()
            .flat_map(|site| site.groups.iter_mut())
    }

    /// The groups standing on `key`, if any reference does.
    pub fn groups_at(&self, key: K) -> Option<&[RefGroup<E>]> {
        self.sites
            .iter()
            .find(|site| site.on == key)
            .map(|site| site.groups.as_slice())
    }

    /// Author `name` at `key`: find-or-append the row, then run THE placement algorithm
    /// ([`place_in_groups`]). The builder's door; each name is authored at most once.
    pub fn author(
        &mut self,
        key: K,
        name: gix::refs::FullName,
        carry: GroupCarry<E>,
        attach: Option<gix::refs::FullName>,
    ) {
        place_in_groups(self.site_mut(key), name, carry, attach);
    }

    /// Where `name` sits: `(key, group index, member index)`.
    pub fn locate(&self, name: &gix::refs::FullNameRef) -> Option<(K, usize, usize)> {
        self.sites.iter().find_map(|site| {
            site.groups.iter().enumerate().find_map(|(g, group)| {
                group
                    .members
                    .iter()
                    .position(|m| m.as_ref() == name)
                    .map(|i| (site.on, g, i))
            })
        })
    }

    /// The key `name` stands on — `None` for unpositioned (unborn) names.
    pub fn positioned_on(&self, name: &gix::refs::FullNameRef) -> Option<K> {
        self.locate(name).map(|(key, ..)| key)
    }

    /// The name directly underneath `name` in its group's physical stack — the previous
    /// member, or the group's attach for a bottom member; `None` when it sits directly on
    /// its key (or holds no position at all).
    pub fn below_of(&self, name: &gix::refs::FullNameRef) -> Option<&gix::refs::FullName> {
        let (key, g, i) = self.locate(name)?;
        let group = &self.groups_at(key)?[g];
        if i > 0 {
            Some(&group.members[i - 1])
        } else {
            group.attach.as_ref()
        }
    }

    /// The carry of the group holding `name`, if it holds a position.
    pub fn carry_of(&self, name: &gix::refs::FullNameRef) -> Option<&GroupCarry<E>> {
        let (key, g, _) = self.locate(name)?;
        Some(&self.groups_at(key)?[g].carry)
    }

    /// Every positioned name with the key it stands on, in group order (per key
    /// bottom→top). One entry per name — the name-keyed contract.
    pub fn placements(&self) -> impl Iterator<Item = (&gix::refs::FullName, K)> {
        self.sites.iter().flat_map(|site| {
            site.groups
                .iter()
                .flat_map(move |group| group.members.iter().map(move |name| (name, site.on)))
        })
    }

    /// Take `name` out of the table, its dependents RIDING: members stacked above it in
    /// its group split off as their own group attached to `name` (they keep sitting on it,
    /// wherever it goes next), and groups attached to `name` stay attached. Returns the
    /// name it vacated — what sat directly below — for the caller to re-place against;
    /// `None` also when `name` held no position.
    pub fn extract(&mut self, name: &gix::refs::FullNameRef) -> Option<gix::refs::FullName> {
        let (key, g, i) = self.locate(name)?;
        let name = name.to_owned();
        let site = self
            .sites
            .iter()
            .position(|site| site.on == key)
            .expect("just located");
        let groups = &mut self.sites[site].groups;
        let vacated = if i > 0 {
            Some(groups[g].members[i - 1].clone())
        } else {
            groups[g].attach.clone()
        };
        let above = groups[g].members.split_off(i + 1);
        groups[g].members.pop();
        if !above.is_empty() {
            let carry = groups[g].carry.clone();
            groups.push(RefGroup {
                members: above,
                carry,
                attach: Some(name),
            });
        }
        if groups[g].members.is_empty() {
            groups.remove(g);
        }
        if groups.is_empty() {
            self.sites.remove(site);
        }
        vacated
    }

    /// Splice `name` out of its stack while staying at its key: dependents heal past it
    /// (members above close the gap, attached groups re-hang onto what it vacated), and
    /// the name lands back as its own group — same carry — on what it sat on.
    pub fn splice(&mut self, name: &gix::refs::FullNameRef) {
        let Some((key, g, i)) = self.locate(name) else {
            return;
        };
        let name = name.to_owned();
        let site = self
            .sites
            .iter()
            .position(|site| site.on == key)
            .expect("just located");
        let groups = &mut self.sites[site].groups;
        let below = if i > 0 {
            Some(groups[g].members[i - 1].clone())
        } else {
            groups[g].attach.clone()
        };
        groups[g].members.remove(i);
        let carry = groups[g].carry.clone();
        if groups[g].members.is_empty() {
            groups.remove(g);
        }
        for group in self.groups_mut() {
            if group.attach.as_ref() == Some(&name) {
                group.attach = below.clone();
            }
        }
        self.site_mut(key).push(RefGroup {
            members: vec![name],
            carry,
            attach: below,
        });
        self.coalesce(key);
    }

    /// Put the (extracted or fresh) `name` into the table at `key` via [`place_in_groups`].
    /// `vacated` is what [`Self::extract`] returned: when the name slides UNDER one of its
    /// own riders — a group still attached to it that contains the new `attach` — that
    /// rider takes the vacated spot instead, or the attach chain would close on itself.
    pub fn place(
        &mut self,
        name: gix::refs::FullName,
        key: K,
        carry: GroupCarry<E>,
        attach: Option<gix::refs::FullName>,
        vacated: Option<gix::refs::FullName>,
    ) {
        if let (Some(b), Some(site)) = (
            attach.as_ref(),
            self.sites.iter_mut().find(|site| site.on == key),
        ) {
            for group in site.groups.iter_mut() {
                if group.attach.as_ref() == Some(&name) && group.members.contains(b) {
                    group.attach = vacated.clone();
                }
            }
        }
        place_in_groups(self.site_mut(key), name, carry, attach);
    }

    /// Re-run group coalescing at `key` (see [`coalesce_groups`]).
    pub fn coalesce(&mut self, key: K) {
        if let Some(site) = self.sites.iter_mut().find(|site| site.on == key) {
            coalesce_groups(&mut site.groups);
        }
    }

    /// Adopt `groups` wholesale as `key`'s reference groups — ingest's id-mapped copy.
    pub fn insert_groups(&mut self, key: K, groups: Vec<RefGroup<E>>) {
        self.site_mut(key).extend(groups);
    }

    /// Rename `old` to `new` everywhere it appears — members and attaches.
    pub fn rename(&mut self, old: &gix::refs::FullNameRef, new: &gix::refs::FullName) {
        for group in self.groups_mut() {
            for member in group.members.iter_mut() {
                if member.as_ref() == old {
                    *member = new.clone();
                }
            }
            if group.attach.as_ref().is_some_and(|a| a.as_ref() == old) {
                group.attach = Some(new.clone());
            }
        }
    }

    fn site_mut(&mut self, key: K) -> &mut Vec<RefGroup<E>> {
        let at = match self.sites.iter().position(|site| site.on == key) {
            Some(at) => at,
            None => {
                self.sites.push(Site {
                    on: key,
                    groups: Vec::new(),
                });
                self.sites.len() - 1
            }
        };
        &mut self.sites[at].groups
    }
}

/// The ref layout: the stored reference groups per commit plus per-reference facts and the
/// workspace anchors. Everything here is something the build DECIDED; nothing is a value
/// you could recompute from the other fields — those are answered by the queries
/// ([`Self::positioned_on`], [`Self::below_of`], [`Self::facts_of`], [`Self::placements`]),
/// so a stored copy can never go stale or disagree. The two fields that look derivable
/// aren't: [`Self::reachable_commits`] follows the plan's segment wiring (empty chains
/// included), not the arena's parent entries, and [`Self::head_refs`] records the naming decision,
/// not the refs at a commit.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RefLayout {
    /// THE stored structure: per commit, the ordered reference groups standing on it —
    /// the exact shape the rebase editor stores over its own indices. Commits appear in
    /// first-namer order (the order their first ref holds in [`Self::facts`]).
    pub groups: RefGroups<gix::ObjectId, ParentIndex>,
    /// Per-reference facts, in reference-table order — the order fixes the editor's
    /// reference table (and with it render sibling order). Unborn refs (no position) are
    /// here too, absent from [`Self::groups`].
    pub facts: Vec<(gix::refs::FullName, RefFacts)>,
    /// See [`AmendedWsParents`]; `None` without a managed entrypoint commit.
    pub amended_ws_parents: Option<AmendedWsParents>,
    /// The entrypoint's ref names — the editor's HEAD checkouts.
    pub head_refs: Vec<gix::refs::FullName>,
    /// Commits reachable from the entrypoint (sorted) — the editor's mutable commits.
    pub reachable_commits: Vec<gix::ObjectId>,
    /// Where the empty chains rest: for each declared chain with no commits of its own,
    /// the commit its branches sit on, in metadata order.
    pub empty_chain_anchors: Vec<EmptyChainAnchor>,
    /// The declared in-workspace stack partition, in metadata (workspace) order: each stack's
    /// branch names in declared tip→base order, empty branches included. This is
    /// `ws_meta`'s in-workspace stack list reduced to what the projection's claiming needs and
    /// relocated onto the layout, giving the IN-WORKSPACE ref→stack partition one home the
    /// projection reads instead of re-consulting metadata. Inactive/out-of-workspace stacks
    /// are deliberately excluded — the projection still reads those from `ws_meta` directly: the
    /// stack-id adoption in `projection::derive`'s `materialize_stacks` and the leftover-identity
    /// adoption in `projection::partition`'s `derive_partition` both scan the unfiltered
    /// `ws_meta.stacks`. So membership truth is split by
    /// design. It mirrors the in-workspace
    /// [`WorkspaceStack`](but_core::ref_metadata::WorkspaceStack)s WITHOUT cross-stack dedup
    /// (a branch listed in two stacks appears in both) — within-stack duplicates stay the
    /// claim's concern, exactly as when it read the metadata directly.
    pub stacks: Vec<DeclaredStack>,
}

/// One in-workspace stack's declared partition: its branch names in declared tip→base
/// order (empties included). Identity is the stack's position in the declared partition —
/// the persisted metadata id never enters the layout; the projection stamps it onto the
/// produced lane at the boundary, straight from the metadata. See [`RefLayout::stacks`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredStack {
    /// The stack's branch names, tip→base, exactly as the metadata lists them.
    pub branches: Vec<gix::refs::FullName>,
    /// The declared branch parents as `(child_idx, parent_idx)` into [`branches`](Self::branches),
    /// implied chain adjacency resolved — the DAG declaration
    /// ([`WorkspaceStackBranch::parents`](but_core::ref_metadata::WorkspaceStackBranch::parents))
    /// reduced for the projection. A chain is `(i, i + 1)` pairs.
    pub branch_parents: Vec<(usize, usize)>,
    /// Whether any branch DECLARED its parents explicitly. Read at BUILD time only, where it
    /// guards the implementation-ref normalization — a declared DAG cannot carry the legacy
    /// shape that normalization exists for. Nothing in the projection routes on it; the
    /// engine reads the parent entries themselves.
    pub declares_dag: bool,
}

/// One reference's authoring input to [`RefLayout::from_parts`]: its facts, and — when
/// positioned — its placement.
pub(crate) struct RefPart {
    pub name: gix::refs::FullName,
    pub facts: RefFacts,
    pub placement: Option<Placement>,
}

/// A positioned reference's placement: the commit it stands on, the NAME of the reference
/// below it, and its entering parent entries (sorted).
pub(crate) struct Placement {
    pub on: gix::ObjectId,
    pub below: Option<gix::refs::FullName>,
    pub entering: Vec<ParentIndex>,
}

impl RefLayout {
    /// THE PER-BRANCH GATE, one verdict: does this name shape a stack at all? `false` in three
    /// cases — a name absent from the layout (no facts, so it does not exist here), a name in
    /// the `refs/heads/gitbutler/*` implementation namespace, and a naming loser (where several
    /// refs sit on one commit only one names the segment; a rider appears nowhere, and whoever
    /// won the naming keeps the territory). `true` exactly when the name NAMES its position's
    /// segment — an empty one included.
    pub fn shapes_stack(&self, name: &gix::refs::FullNameRef) -> bool {
        !in_gitbutler_namespace(name) && self.facts_of(name).is_some_and(|f| f.names_segment)
    }

    /// Author a layout from the builder's parts: per reference (table order) its facts and
    /// resolved placement (`on`, the NAME below it, its entering parent entries) — groups authored
    /// directly, the position view derived from them. `incoming` answers a commit's full
    /// connected incoming parent entry set, `(child, parent number)` sorted — the [`GroupCarry::All`]
    /// classification and expansion oracle.
    pub(crate) fn from_parts(
        parts: Vec<RefPart>,
        incoming: &dyn Fn(gix::ObjectId) -> Vec<ParentIndex>,
    ) -> Self {
        // Group placements exactly like the editor's place(): in table order, a ref joins
        // the group whose top member is its below and whose carry equals its own — else it
        // starts a fresh group attached there. Coalescing keeps groups canonical maximal
        // same-carry runs.
        //
        // One name = one reference is the name-keyed contract; the builder authors each
        // name at most once.
        let mut groups: RefGroups<gix::ObjectId, ParentIndex> = RefGroups::default();
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
                GroupCarry::Entries(entering.clone())
            };
            let attach = below.clone();
            groups.author(*on, name.clone(), carry, attach);
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
        self.groups.below_of(name)
    }

    /// The commit the reference `name` stands on — `None` for unborn refs (and unknown
    /// names), which keep no stored placement.
    pub fn positioned_on(&self, name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        self.groups.positioned_on(name)
    }

    /// Every positioned reference with the commit it stands on, in group order (per commit
    /// bottom→top). One entry per name — the layout's name-keyed contract.
    pub fn placements(&self) -> impl Iterator<Item = (&gix::refs::FullName, gix::ObjectId)> {
        self.groups.placements()
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
pub fn place_in_groups<E: PartialEq>(
    groups: &mut Vec<RefGroup<E>>,
    name: gix::refs::FullName,
    carry: GroupCarry<E>,
    attach: Option<gix::refs::FullName>,
) {
    let joined = attach
        .as_ref()
        .and_then(|b| {
            groups
                .iter()
                .position(|group| group.members.last() == Some(b) && group.carry == carry)
        })
        // A group already standing at this exact (carry, attach) key is this placement's
        // home: a parallel twin would put two members at one rank — the unordered position
        // collision — so the newcomer lands on the twin's top instead. Placement is the
        // only door to the table, which makes the collision unconstructable rather than
        // merely checked. `None` carries are exempt: they hold no rank (nothing enters
        // through them), so independent root groups — a dead ref's retained position, an
        // empty top — legitimately coexist and must not be folded together.
        .or_else(|| {
            (!matches!(carry, GroupCarry::None))
                .then(|| {
                    groups
                        .iter()
                        .position(|group| group.carry == carry && group.attach == attach)
                })
                .flatten()
        });
    match joined {
        Some(g) => {
            // Insertion re-links dependents: pushing onto the group's top puts the
            // newcomer between the old top and anything standing ON that top, so a
            // same-carry group attached to the old top re-hangs onto the newcomer —
            // otherwise its anchor becomes a mid-group member and two references
            // share one rank (the collision the twin rule exists to prevent).
            // Different-carry attachers keep their anchor: they occupy a different
            // rank key and are legal parallels.
            let old_top = groups[g].members.last().cloned();
            groups[g].members.push(name.clone());
            if let Some(old_top) = old_top {
                let to_rehang: Vec<usize> = groups
                    .iter()
                    .enumerate()
                    .filter(|(i, group)| {
                        *i != g
                            && group.carry == groups[g].carry
                            && group.attach.as_ref() == Some(&old_top)
                    })
                    .map(|(i, _)| i)
                    .collect();
                for i in to_rehang {
                    groups[i].attach = Some(name.clone());
                }
            }
        }
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
pub fn coalesce_groups<E: PartialEq>(groups: &mut Vec<RefGroup<E>>) {
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
        // The merge is an insertion too: the old lower top gains members on it, so
        // any OTHER same-carry group anchored to that top re-hangs onto the merged
        // chain's new top — else its anchor is a mid-group member and two references
        // share one rank. (Two same-carry groups on one anchor are exactly the
        // unordered-fork case; re-hanging serializes them, and the next loop pass
        // folds the re-hung group in as well.)
        let old_top = groups[lower_idx].members.last().cloned();
        groups[lower_idx].members.extend(upper.members);
        let new_top = groups[lower_idx].members.last().cloned();
        if let (Some(old_top), Some(new_top)) = (old_top, new_top) {
            let to_rehang: Vec<usize> = groups
                .iter()
                .enumerate()
                .filter(|(i, group)| {
                    *i != lower_idx
                        && group.carry == groups[lower_idx].carry
                        && group.attach.as_ref() == Some(&old_top)
                })
                .map(|(i, _)| i)
                .collect();
            for i in to_rehang {
                groups[i].attach = Some(new_top.clone());
            }
        }
    }
}

/// The `refs/heads/gitbutler/` namespace: refs GitButler creates for itself (the
/// workspace ref, targets) rather than the user's branches. Centralized here so the
/// rule has one home for every consumer (the partition engine, the display prunes, keep-lists).
pub fn in_gitbutler_namespace(name: &gix::refs::FullNameRef) -> bool {
    name.as_bstr().starts_with(b"refs/heads/gitbutler/")
}
