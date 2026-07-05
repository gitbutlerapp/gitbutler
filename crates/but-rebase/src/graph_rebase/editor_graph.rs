//! An owned arena graph for rebase steps: the NODE arena's payload is the COMMIT ID
//! (`None` = tombstone) with pick options in a parallel settings table, each node bearing
//! an ORDERED PARENT ARRAY — a parent's position in the array IS its parent order, dense by
//! construction. References live in the REF table and bear positions. Children are DERIVED
//! (a reverse scan of the parent arrays), never stored. Nothing is ever removed (a removed
//! pick becomes a `None` payload, a removed reference goes dead in place), so ids are
//! stable by construction. [`Step`] and [`Pick`] are BOUNDARY VALUE types: synthesized by
//! [`EditorGraph::step_view`], decomposed by [`EditorGraph::add_node`]/[`EditorGraph::set_step`].

use std::collections::{HashMap, HashSet};

use but_core::commit::SignCommit;

use crate::graph_rebase::{
    Pick, Step,
    cherry_pick::{PickMode, TreeMergeMode},
};

/// The stable identifier of a commit-graph entry. Two namespaces, one id type: `Node` points
/// into the pick arena (its parent array is its truth), `Ref` into the reference table (a
/// position is its truth) — so a selector can address either without knowing which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EditorGraphIndex {
    /// A pick or its tombstone in the node arena.
    Node(usize),
    /// A reference (live or dead) in the ref table.
    Ref(usize),
}

impl std::fmt::Display for EditorGraphIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorGraphIndex::Node(i) => write!(f, "n{i}"),
            EditorGraphIndex::Ref(i) => write!(f, "r{i}"),
        }
    }
}

/// One incoming child edge of a pick, named POSITIONALLY as `(source pick, parent-slot)`.
/// Chains state edges by this name, so an edge removed and re-created at the same coordinates is
/// the SAME statement — see [`ChainRec::edges`].
pub(crate) type Edge = (EditorGraphIndex, usize);

/// Where a reference sits, stored explicitly: references are POSITIONS, not topology. The
/// edges entering through it live in the reference's CHAIN (see [`EditorGraph::chain_of`]), not here.
/// Derived reads live in `positions`: `ref_depth` (rank), `edges_through` (entering edges),
/// `resolve_to_pick` (the node, followed through tombstones).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefPosition {
    /// The node this reference resolves to (a pick, or its tombstone after deletion) — the
    /// commit the ref points at, reached lazily through tombstones at read time.
    pub on: EditorGraphIndex,
    /// The reference directly underneath in the physical stack (`None` = sits directly on the node).
    /// Rank is DERIVED: a reference's depth is the length of its below-chain
    /// (`positions::ref_depth`).
    pub below: Option<EditorGraphIndex>,
    /// The entry into this position converged — more than one thing (edges and/or refs stacked
    /// above) met here (a merge). A creation-time signal distinct from `edges.len() > 1` (a position
    /// can converge yet resolve to a single edge), so it is stored and PRESERVED, not re-derived.
    pub ambiguous: bool,
}

/// One reference: name, mutability, liveness, position. Deletion flips `live` and RETAINS
/// name and position — retention is load-bearing (stale selectors normalize through dead
/// refs, rebuilds carry them).
#[derive(Debug, Clone)]
pub(crate) struct RefRecord {
    /// The full reference name.
    pub refname: gix::refs::FullName,
    /// Whether the rebase may move this reference.
    pub mutable: bool,
    /// `false` once the reference is deleted; the record stays.
    pub live: bool,
    /// The stored position, `None` until placed (creation routes connectivity through
    /// temporary ref parent arrays first, then converts them to a position at finalize).
    pub position: Option<RefPosition>,
}

/// Everything a [`Pick`] carries except the commit id — the id is the arena payload itself,
/// so the options live beside it. Stale for tombstones (never read; a revival overwrites).
#[derive(Debug, Clone)]
pub(crate) struct PickSettings {
    pub preserved_parents: Option<Vec<gix::ObjectId>>,
    pub pick_mode: PickMode,
    pub sign_commit: SignCommit,
    pub exclude_from_tracking: bool,
    pub conflictable: bool,
    pub tree_merge_mode: TreeMergeMode,
    pub mutable: bool,
}

impl PickSettings {
    fn split(pick: Pick) -> (gix::ObjectId, Self) {
        let Pick {
            id,
            preserved_parents,
            pick_mode,
            sign_commit,
            exclude_from_tracking,
            conflictable,
            tree_merge_mode,
            mutable,
        } = pick;
        (
            id,
            Self {
                preserved_parents,
                pick_mode,
                sign_commit,
                exclude_from_tracking,
                conflictable,
                tree_merge_mode,
                mutable,
            },
        )
    }

    fn pick(&self, id: gix::ObjectId) -> Pick {
        Pick {
            id,
            preserved_parents: self.preserved_parents.clone(),
            pick_mode: self.pick_mode,
            sign_commit: self.sign_commit,
            exclude_from_tracking: self.exclude_from_tracking,
            conflictable: self.conflictable,
            tree_merge_mode: self.tree_merge_mode,
            mutable: self.mutable,
        }
    }
}

impl Default for PickSettings {
    fn default() -> Self {
        let (_, settings) = Self::split(Pick::new_pick(gix::ObjectId::null(gix::hash::Kind::Sha1)));
        settings
    }
}

/// How much of its node's incoming edges a chain carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChainCarry {
    /// Nothing descends into this chain (a root chain: remote above a tip, empty top).
    None,
    /// Every edge into the node descends through this chain (a plain chain, or a shared
    /// chain all merge chains converge on).
    All,
    /// This chain carries exactly the edges its [`ChainRec::edges`] statement names — one chain
    /// of a merge.
    Edges,
}

/// One chain above a stored node: the references sharing the same entering edges at one position.
/// Membership only — order among members stays the below-chain's job.
#[derive(Debug, Clone)]
pub(crate) struct ChainRec {
    /// The reference nodes in this chain, unordered (order by below-chain depth to read).
    pub members: Vec<EditorGraphIndex>,
    /// How much of the node's edges this chain carries.
    pub carry: ChainCarry,
    /// The edges this chain STATES it carries (`Edges` chains only). Keyed by the full
    /// `(source-pick, parent-slot)` edge: two distinct sources can feed one node at the
    /// same slot (and one source at two slots), so both coordinates are needed. Read
    /// filtered against the node's LIVE edges, so a stale entry is inert — and reclaims
    /// its edge by itself when surgery revives the same coordinates.
    pub edges: Vec<Edge>,
}

/// The editor's commit graph: a [`but_graph::CommitGraph`] arena where PICKS carry ordered
/// parent slots, plus a table of [`RefRecord`]s where REFERENCES carry explicit positions —
/// the arena is the truth for commits, positions the truth for refs, with no overlap.
/// References are edgeless: native creation authors their positions straight from the
/// placement ledger.
#[derive(Debug, Clone, Default)]
pub(crate) struct EditorGraph {
    /// THE arena: `EditorGraphIndex::Node(i)` IS `CommitIdx` `i`. Commit ids are the payload
    /// (tombstoning flags a node in place, the node id survives every rewrite), parent
    /// slots are the ordered structure.
    arena: but_graph::CommitGraph,
    /// Each node's pick options, parallel to the arena.
    settings: Vec<PickSettings>,
    refs: Vec<RefRecord>,
    /// THE entering-edge store: chain membership per STORED (unresolved) `on` value. Which edges
    /// descend into a reference's position lives here and only here — authored by
    /// [`Self::set_position`]/[`Self::join_chain_of`], carried by [`Self::rekey_position`],
    /// renamed by [`Self::rename_edges`], read via `positions::edges_through`.
    chains: HashMap<EditorGraphIndex, Vec<ChainRec>>,
}

impl EditorGraph {
    /// Adopt `arena` wholesale as THE arena — full commit payloads (flags, refs, generation)
    /// survive, which the write-through put-back depends on. The caller must have normalized
    /// every parent slot to PRESENT (the editor's slot invariant) and follows up with
    /// per-node settings via [`Self::set_step`].
    pub(crate) fn adopt(arena: but_graph::CommitGraph) -> Self {
        let settings = vec![PickSettings::default(); arena.node_count()];
        Self {
            arena,
            settings,
            refs: Vec::new(),
            chains: HashMap::new(),
        }
    }

    /// THE arena, read-only — the write-through seam projects it after a rebase.
    pub(crate) fn arena(&self) -> &but_graph::CommitGraph {
        &self.arena
    }

    /// Add `step` to the node arena and return its stable id. References do not belong here —
    /// use [`Self::add_reference`].
    pub(crate) fn add_node(&mut self, step: Step) -> EditorGraphIndex {
        let (id, settings) = match step {
            Step::Pick(pick) => {
                let (id, settings) = PickSettings::split(pick);
                (Some(id), settings)
            }
            Step::None => (None, PickSettings::default()),
            Step::Reference { .. } => {
                panic!("references go through add_reference, not the step arena")
            }
        };
        let i = self.arena.add_node(id);
        self.settings.push(settings);
        debug_assert_eq!(
            self.settings.len(),
            self.arena.node_count(),
            "settings table fell out of step with the arena"
        );
        EditorGraphIndex::Node(i)
    }

    /// Replace the node payload at `node` with `step` — a pick decomposes into id and
    /// settings, [`Step::None`] tombstones the payload (settings go stale, not cleared).
    pub(crate) fn set_step(&mut self, node: EditorGraphIndex, step: Step) {
        let EditorGraphIndex::Node(i) = node else {
            panic!("BUG: references live in the ref table, not the step arena");
        };
        match step {
            Step::Pick(pick) => {
                let (id, settings) = PickSettings::split(pick);
                self.arena.set_node_id(i, Some(id));
                self.settings[i] = settings;
            }
            Step::None => self.arena.set_node_id(i, None),
            Step::Reference { .. } => {
                panic!("BUG: references live in the ref table, not the step arena")
            }
        }
    }

    /// The commit id of the pick at `node` — `None` for tombstones and references. THE fast
    /// payload read; whole-step consumers use [`Self::step_view`].
    pub(crate) fn commit_id(&self, node: EditorGraphIndex) -> Option<gix::ObjectId> {
        match node {
            EditorGraphIndex::Node(i) => self.arena.node_payload(i),
            EditorGraphIndex::Ref(_) => None,
        }
    }

    /// Rewrite the commit id of the pick at `node` IN PLACE — THE rebase write: the node id,
    /// its parent array, its settings, and every position naming it all survive unchanged.
    pub(crate) fn set_commit_id(&mut self, node: EditorGraphIndex, id: gix::ObjectId) {
        let EditorGraphIndex::Node(i) = node else {
            panic!("BUG: only picks carry commit ids");
        };
        debug_assert!(
            self.arena.node_payload(i).is_some(),
            "tombstones have no commit id"
        );
        self.arena.set_commit_id(i, id);
    }

    /// Overwrite the preserved parents of the pick at `node` (see
    /// [`Pick::preserved_parents`]).
    pub(crate) fn set_preserved_parents(
        &mut self,
        node: EditorGraphIndex,
        parents: Option<Vec<gix::ObjectId>>,
    ) {
        let EditorGraphIndex::Node(i) = node else {
            panic!("BUG: only picks carry preserved parents");
        };
        debug_assert!(
            self.arena.node_payload(i).is_some(),
            "tombstones carry no pick options"
        );
        self.settings[i].preserved_parents = parents;
    }

    /// Add a reference and return its stable id.
    pub(crate) fn add_reference(
        &mut self,
        refname: gix::refs::FullName,
        mutable: bool,
    ) -> EditorGraphIndex {
        self.refs.push(RefRecord {
            refname,
            mutable,
            live: true,
            position: None,
        });
        EditorGraphIndex::Ref(self.refs.len() - 1)
    }

    /// The reference payload at `node` — `Some` iff it names a live (non-deleted) reference.
    pub(crate) fn reference(&self, node: EditorGraphIndex) -> Option<(&gix::refs::FullName, bool)> {
        let EditorGraphIndex::Ref(i) = node else {
            return None;
        };
        let record = self.refs.get(i)?;
        record.live.then_some((&record.refname, record.mutable))
    }

    /// `true` iff `node` is a live reference.
    pub(crate) fn is_reference(&self, node: EditorGraphIndex) -> bool {
        self.reference(node).is_some()
    }

    /// `true` iff `node` is a pick — `false` for tombstones and references.
    pub(crate) fn is_pick(&self, node: EditorGraphIndex) -> bool {
        self.commit_id(node).is_some()
    }

    /// All live references, ascending by id.
    pub(crate) fn references(
        &self,
    ) -> impl Iterator<Item = (EditorGraphIndex, &gix::refs::FullName, bool)> + '_ {
        self.refs.iter().enumerate().filter_map(|(i, record)| {
            record
                .live
                .then_some((EditorGraphIndex::Ref(i), &record.refname, record.mutable))
        })
    }

    /// All reference ids — live AND dead — ascending. Dead references still carry their
    /// retained name and position (see [`RefRecord`]).
    pub(crate) fn ref_indices(&self) -> impl Iterator<Item = EditorGraphIndex> + '_ {
        (0..self.refs.len()).map(EditorGraphIndex::Ref)
    }

    /// The full record of the reference at `node`, including dead ones — rebuilds need the
    /// retained payload.
    pub(crate) fn reference_record(&self, node: EditorGraphIndex) -> Option<&RefRecord> {
        match node {
            EditorGraphIndex::Ref(i) => self.refs.get(i),
            EditorGraphIndex::Node(_) => None,
        }
    }

    /// Rename (or resurrect) the reference at `node` in place; its position is untouched.
    pub(crate) fn set_reference(
        &mut self,
        node: EditorGraphIndex,
        refname: gix::refs::FullName,
        mutable: bool,
    ) {
        let EditorGraphIndex::Ref(i) = node else {
            panic!("BUG: only references can be renamed");
        };
        let record = &mut self.refs[i];
        record.refname = refname;
        record.mutable = mutable;
        record.live = true;
    }

    /// Delete the reference at `node`: it goes dead in place, RETAINING its name and
    /// position so stale selectors keep normalizing and rebuilds keep carrying it.
    pub(crate) fn tombstone_reference(&mut self, node: EditorGraphIndex) {
        let EditorGraphIndex::Ref(i) = node else {
            panic!("BUG: only references can be tombstoned");
        };
        self.refs[i].live = false;
    }

    /// The step at `node` as an owned view — the read for whole-step consumers, synthesized
    /// from the payload: id plus settings make a `Step::Pick`, a `None` id a `Step::None`,
    /// a reference entry `Step::Reference` while live and `Step::None` once dead.
    pub(crate) fn step_view(&self, node: EditorGraphIndex) -> Step {
        match node {
            EditorGraphIndex::Node(i) => match self.arena.node_payload(i) {
                Some(id) => Step::Pick(self.settings[i].pick(id)),
                None => Step::None,
            },
            EditorGraphIndex::Ref(i) => {
                let record = &self.refs[i];
                if record.live {
                    Step::Reference {
                        refname: record.refname.clone(),
                        mutable: record.mutable,
                    }
                } else {
                    Step::None
                }
            }
        }
    }

    /// The stored position of the reference at `node`, live or dead.
    pub(crate) fn position_of(&self, node: EditorGraphIndex) -> Option<RefPosition> {
        match node {
            EditorGraphIndex::Ref(i) => self.refs.get(i)?.position.clone(),
            EditorGraphIndex::Node(_) => None,
        }
    }

    fn position_slot(&mut self, node: EditorGraphIndex) -> &mut Option<RefPosition> {
        match node {
            EditorGraphIndex::Ref(i) => &mut self.refs[i].position,
            EditorGraphIndex::Node(_) => panic!("BUG: only references hold positions"),
        }
    }

    /// Author a FRESH position for `node`: `entering` is the chain intent — the edges meant to
    /// enter through it — classified against
    /// `on`'s CURRENT edges — empty opens (or joins) the root chain, the whole live set the
    /// shared `All` chain, any other set a `Count` chain stating exactly those edges. Only
    /// correct when the node's edges are already complete — never use to re-place an
    /// existing position wholesale.
    pub(crate) fn set_position(
        &mut self,
        node: EditorGraphIndex,
        on: EditorGraphIndex,
        entering: &[Edge],
        ambiguous: bool,
        below: Option<EditorGraphIndex>,
    ) {
        let live = match crate::graph_rebase::positions::resolve_to_pick(self, on) {
            Some(pick) => crate::graph_rebase::positions::edges_into(self, pick),
            None => Vec::new(),
        };
        let (carry, edges) = if entering.is_empty() {
            (ChainCarry::None, Vec::new())
        } else {
            let edge_set: HashSet<_> = entering.iter().copied().collect();
            let live_set: HashSet<_> = live.iter().copied().collect();
            if edge_set == live_set {
                (ChainCarry::All, Vec::new())
            } else {
                let mut edges = entering.to_vec();
                edges.sort_unstable();
                edges.dedup();
                (ChainCarry::Edges, edges)
            }
        };
        if let Some(previous) = self.position_of(node) {
            self.chain_remove(node, previous.on);
        }
        self.chain_insert(node, on, carry, edges);
        *self.position_slot(node) = Some(RefPosition {
            on,
            ambiguous,
            below,
        });
    }

    /// Join `node` into the chain CONTAINING `mate` — direct membership, not edges-equality —
    /// sitting on `below`, copying the mate's `on` and ambiguity.
    pub(crate) fn join_chain_of(
        &mut self,
        node: EditorGraphIndex,
        mate: EditorGraphIndex,
        below: Option<EditorGraphIndex>,
    ) {
        let Some(m) = self.position_of(mate) else {
            return;
        };
        if let Some(previous) = self.position_of(node) {
            self.chain_remove(node, previous.on);
        }
        let joined = self
            .chains
            .entry(m.on)
            .or_default()
            .iter_mut()
            .find(|chain| chain.members.contains(&mate))
            .map(|chain| chain.members.push(node))
            .is_some();
        debug_assert!(joined, "positioned mate {mate} must own a chain");
        if !joined {
            self.chain_insert(node, m.on, ChainCarry::All, Vec::new());
        }
        *self.position_slot(node) = Some(RefPosition {
            on: m.on,
            ambiguous: m.ambiguous,
            below,
        });
    }

    /// Re-key `node`'s position onto `onto`, carrying its CURRENT chain record — the
    /// carry and edges as maintained through edge surgery. Below and ambiguity are preserved.
    pub(crate) fn rekey_position(&mut self, node: EditorGraphIndex, onto: EditorGraphIndex) {
        let Some(stored) = self.position_of(node) else {
            return;
        };
        if stored.on == onto {
            return;
        }
        let chain_data = self.chains.get(&stored.on).and_then(|chains| {
            chains
                .iter()
                .find(|chain| chain.members.contains(&node))
                .map(|chain| (chain.carry.clone(), chain.edges.clone()))
        });
        self.chain_remove(node, stored.on);
        match chain_data {
            Some((carry, edges)) => self.chain_insert(node, onto, carry, edges),
            None => {
                debug_assert!(false, "positioned node {node} must own a chain");
                self.chain_insert(node, onto, ChainCarry::All, Vec::new());
            }
        }
        if let Some(a) = self.position_slot(node).as_mut() {
            a.on = onto;
        }
    }

    /// Re-hang `node` onto `below` — an adjacency statement only; `on` and chain
    /// membership are untouched.
    pub(crate) fn set_below(&mut self, node: EditorGraphIndex, below: Option<EditorGraphIndex>) {
        if let Some(stored) = self.position_slot(node).as_mut() {
            stored.below = below;
        }
    }

    /// Point a DEAD reference's retained position at `on` — the bare retention pointer
    /// stale selectors normalize through. Chain membership, below, and ambiguity are dropped;
    /// live references re-place via the arrangement machinery instead.
    pub(crate) fn set_retained_position(&mut self, node: EditorGraphIndex, on: EditorGraphIndex) {
        debug_assert!(
            !self.is_reference(node) && matches!(node, EditorGraphIndex::Ref(_)),
            "retained positions belong to dead references"
        );
        if let Some(stored) = self.position_of(node) {
            self.chain_remove(node, stored.on);
        }
        *self.position_slot(node) = Some(RefPosition {
            on,
            below: None,
            ambiguous: false,
        });
    }

    fn chain_remove(&mut self, node: EditorGraphIndex, key: EditorGraphIndex) {
        let Some(chains) = self.chains.get_mut(&key) else {
            return;
        };
        for chain in chains.iter_mut() {
            chain.members.retain(|&member| member != node);
        }
        chains.retain(|chain| !chain.members.is_empty());
        if chains.is_empty() {
            self.chains.remove(&key);
        }
    }

    fn chain_insert(
        &mut self,
        node: EditorGraphIndex,
        key: EditorGraphIndex,
        carry: ChainCarry,
        edges: Vec<(EditorGraphIndex, usize)>,
    ) {
        let chains = self.chains.entry(key).or_default();
        let existing = chains.iter_mut().find(|chain| {
            // `Edges` chains are identified by their stated edges (same edges => same chain);
            // one `None` and one `All` chain per key.
            chain.carry == carry && (carry != ChainCarry::Edges || chain.edges == edges)
        });
        match existing {
            Some(chain) => chain.members.push(node),
            None => chains.push(ChainRec {
                members: vec![node],
                carry,
                edges,
            }),
        }
    }

    /// The chain containing the reference at `node`, if it holds a position.
    pub(crate) fn chain_of(&self, node: EditorGraphIndex) -> Option<&ChainRec> {
        let stored = match node {
            EditorGraphIndex::Ref(i) => self.refs.get(i)?.position.as_ref()?,
            EditorGraphIndex::Node(_) => return None,
        };
        self.chains
            .get(&stored.on)?
            .iter()
            .find(|chain| chain.members.contains(&node))
    }

    /// All positioned references — live AND dead — ascending by id.
    pub(crate) fn positioned_refs(
        &self,
    ) -> impl Iterator<Item = (EditorGraphIndex, RefPosition)> + '_ {
        self.refs.iter().enumerate().filter_map(|(i, record)| {
            record
                .position
                .clone()
                .map(|p| (EditorGraphIndex::Ref(i), p))
        })
    }

    /// Apply several edge renames SIMULTANEOUSLY: every chain edge is matched against the
    /// pre-rename names once, so shifting slots in a renumber can't collide mid-flight.
    /// Each renamed edge is `(old, new)` — a slot renumbered or re-sourced onto another pick.
    pub(crate) fn rename_edges(&mut self, renames: &[(Edge, Edge)]) {
        for chains in self.chains.values_mut() {
            for chain in chains.iter_mut() {
                let mut changed = false;
                for edge in chain.edges.iter_mut() {
                    if let Some((_, new)) = renames.iter().find(|(old, _)| old == edge) {
                        *edge = *new;
                        changed = true;
                    }
                }
                if changed {
                    chain.edges.sort_unstable();
                    chain.edges.dedup();
                }
            }
        }
    }

    // --- The parent arrays ---
    //
    // A node's parents ARE an ordered array: the slot is the parent order, dense by
    // construction. Statement names `(child, slot)` are live coordinates — mutators shift
    // and rename statements along with the slots they move. A removed slot's statements are
    // dropped — revival is an explicit op-level re-statement, never a store-level
    // coincidence.

    /// The ordered parents of `node` — slot position is the parent order. References are
    /// edgeless by construction.
    pub(crate) fn parents(&self, node: EditorGraphIndex) -> Vec<EditorGraphIndex> {
        match node {
            EditorGraphIndex::Node(i) => self
                .arena
                .parent_indices(i)
                .into_iter()
                .map(EditorGraphIndex::Node)
                .collect(),
            EditorGraphIndex::Ref(_) => Vec::new(),
        }
    }

    /// Rewrite `child`'s parent array through `f` — the single seam every parent mutation
    /// flows through into the arena's slot write.
    fn update_parents<R>(
        &mut self,
        child: EditorGraphIndex,
        f: impl FnOnce(&mut Vec<EditorGraphIndex>) -> R,
    ) -> R {
        let EditorGraphIndex::Node(i) = child else {
            panic!("references are edgeless — no parent array");
        };
        let mut parents: Vec<EditorGraphIndex> = self
            .arena
            .parent_indices(i)
            .into_iter()
            .map(EditorGraphIndex::Node)
            .collect();
        let result = f(&mut parents);
        let targets = parents
            .into_iter()
            .map(|parent| match parent {
                EditorGraphIndex::Node(j) => j,
                EditorGraphIndex::Ref(_) => {
                    panic!("references are edgeless — they cannot be parents")
                }
            })
            .collect();
        self.arena.set_parents(i, targets);
        result
    }

    /// How many parent slots `node` has.
    pub(crate) fn parent_count(&self, node: EditorGraphIndex) -> usize {
        self.parents(node).len()
    }

    /// Every parent-array entry naming `node`, as `(child, slot)` edges, sorted — the derived
    /// children read.
    pub(crate) fn incoming_edges(&self, node: EditorGraphIndex) -> Vec<Edge> {
        let mut edges = Vec::new();
        for i in 0..self.arena.node_count() {
            let child = EditorGraphIndex::Node(i);
            for (slot, parent) in self.arena.parent_indices(i).into_iter().enumerate() {
                if EditorGraphIndex::Node(parent) == node {
                    edges.push((child, slot));
                }
            }
        }
        edges.sort_unstable();
        edges
    }

    /// Append `parent` as `child`'s last parent slot; returns the slot.
    pub(crate) fn push_parent(
        &mut self,
        child: EditorGraphIndex,
        parent: EditorGraphIndex,
    ) -> usize {
        self.update_parents(child, |parents| {
            parents.push(parent);
            parents.len() - 1
        })
    }

    /// Insert `parent` at `slot` of `child` (clamped to the array end); later slots shift up
    /// with their statements. Returns the slot actually used.
    pub(crate) fn insert_parent(
        &mut self,
        child: EditorGraphIndex,
        slot: usize,
        parent: EditorGraphIndex,
    ) -> usize {
        let len = self.parent_count(child);
        let slot = slot.min(len);
        let renames: Vec<_> = (slot..len).map(|s| ((child, s), (child, s + 1))).collect();
        self.rename_edges(&renames);
        self.update_parents(child, |parents| parents.insert(slot, parent));
        slot
    }

    /// Remove `child`'s parent at `slot`, returning it; later slots shift down with their
    /// statements, and statements naming the removed slot are dropped.
    pub(crate) fn remove_parent(
        &mut self,
        child: EditorGraphIndex,
        slot: usize,
    ) -> Option<EditorGraphIndex> {
        let len = self.parent_count(child);
        if slot >= len {
            return None;
        }
        let target = self.update_parents(child, |parents| parents.remove(slot));
        self.retain_edges(|&edge| edge != (child, slot));
        let renames: Vec<_> = (slot + 1..len)
            .map(|s| ((child, s), (child, s - 1)))
            .collect();
        self.rename_edges(&renames);
        Some(target)
    }

    /// Re-point `child`'s parent at `slot` onto `new_parent`. The slot — and so the
    /// statement name — is untouched: chains stated on the edge follow it to its new target.
    pub(crate) fn replace_parent(
        &mut self,
        child: EditorGraphIndex,
        slot: usize,
        new_parent: EditorGraphIndex,
    ) {
        self.update_parents(child, |parents| match parents.get_mut(slot) {
            Some(entry) => *entry = new_parent,
            None => debug_assert!(false, "replace_parent: {child} has no slot {slot}"),
        });
    }

    /// Move `from`'s whole parent array onto `to` (which must have none); statements follow
    /// slot-for-slot.
    pub(crate) fn transplant_parents(&mut self, from: EditorGraphIndex, to: EditorGraphIndex) {
        debug_assert_eq!(
            self.parent_count(to),
            0,
            "transplant target {to} already has parents"
        );
        let parents = self.update_parents(from, std::mem::take);
        let renames: Vec<_> = (0..parents.len()).map(|s| ((from, s), (to, s))).collect();
        self.update_parents(to, |slot| *slot = parents);
        self.rename_edges(&renames);
    }

    /// Re-target every parent-array entry naming `from` onto `to`, slots preserved —
    /// statement names are `(source, slot)`, so they stay valid untouched.
    pub(crate) fn redirect_children(&mut self, from: EditorGraphIndex, to: EditorGraphIndex) {
        for i in 0..self.arena.node_count() {
            let child = EditorGraphIndex::Node(i);
            if !self.parents(child).contains(&from) {
                continue;
            }
            self.update_parents(child, |parents| {
                for parent in parents.iter_mut() {
                    if *parent == from {
                        *parent = to;
                    }
                }
            });
        }
    }

    /// Empty `child`'s parent array, returning it. Chain statements naming the drained slots
    /// are DELIBERATELY untouched: the caller re-states the orphaned names onto their new
    /// carrier itself (the below-insert path renames them onto the segment's parent-most).
    pub(crate) fn drain_parents(&mut self, child: EditorGraphIndex) -> Vec<EditorGraphIndex> {
        self.update_parents(child, std::mem::take)
    }

    /// Drop every chain statement `keep` rejects.
    fn retain_edges(&mut self, keep: impl Fn(&Edge) -> bool) {
        for chains in self.chains.values_mut() {
            for chain in chains.iter_mut() {
                chain.edges.retain(&keep);
            }
        }
    }

    /// All node-arena ids (picks and tombstones), ascending. References are NOT included —
    /// see [`Self::references`] and [`Self::ref_indices`].
    pub(crate) fn node_indices(&self) -> impl Iterator<Item = EditorGraphIndex> + '_ {
        (0..self.arena.node_count()).map(EditorGraphIndex::Node)
    }

    /// The ARENA nodes no parent array names — the child-less tips, ascending. References
    /// never appear here: they are edgeless by construction, and the consumers
    /// (head discovery) want picks and tombstones only.
    pub(crate) fn tips(&self) -> impl Iterator<Item = EditorGraphIndex> + '_ {
        let referenced: HashSet<EditorGraphIndex> = (0..self.arena.node_count())
            .flat_map(|i| self.arena.parent_indices(i))
            .map(EditorGraphIndex::Node)
            .collect();
        (0..self.arena.node_count())
            .map(EditorGraphIndex::Node)
            .filter(move |node| !referenced.contains(node))
    }
}
