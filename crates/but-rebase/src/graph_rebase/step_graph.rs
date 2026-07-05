//! An owned arena graph for rebase steps: the NODE arena's payload is the COMMIT ID
//! (`None` = tombstone) with pick options in a parallel settings table, each node bearing
//! an ORDERED PARENT ARRAY — a parent's position in the array IS its parent order, dense by
//! construction. References live in the REF table and bear positions. Children are DERIVED
//! (a reverse scan of the parent arrays), never stored. Nothing is ever removed (a removed
//! pick becomes a `None` payload, a removed reference goes dead in place), so ids are
//! stable by construction. [`Step`] and [`Pick`] are BOUNDARY VALUE types: synthesized by
//! [`StepGraph::step_view`], decomposed by [`StepGraph::add_node`]/[`StepGraph::set_step`].

use std::collections::{HashMap, HashSet};

use but_core::commit::SignCommit;

use crate::graph_rebase::{
    Pick, Step,
    cherry_pick::{PickMode, TreeMergeMode},
};

/// The stable identifier of a step-graph entry. Two namespaces, one id type: `Node` points
/// into the pick arena (its parent array is its truth), `Ref` into the reference table (a
/// position is its truth) — so a selector can address either without knowing which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum StepGraphIndex {
    /// A pick or its tombstone in the node arena.
    Node(usize),
    /// A reference (live or dead) in the ref table.
    Ref(usize),
}

impl std::fmt::Display for StepGraphIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepGraphIndex::Node(i) => write!(f, "n{i}"),
            StepGraphIndex::Ref(i) => write!(f, "r{i}"),
        }
    }
}

/// One incoming child leg of a pick, named POSITIONALLY as `(source pick, parent-slot)`.
/// Lanes state legs by this name so a leg removed and re-created at the same coordinates is
/// the SAME statement — see [`LaneRec::legs`].
pub(crate) type Leg = (StepGraphIndex, usize);

/// Where a reference sits, stored explicitly: references are POSITIONS, not topology. The
/// approach legs live in the reference's LANE (see [`StepGraph::lane_of`]), not here.
/// Derived reads live in `positions`: `ref_depth` (rank), `ref_approach` (legs),
/// `resolve_to_pick` (anchor through tombstones).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefPosition {
    /// The node this reference resolves to (a pick, or its tombstone after deletion) — the
    /// commit the ref points at, reached lazily through tombstones at read time.
    pub anchor: StepGraphIndex,
    /// The reference directly underneath in the physical stack (`None` = sits on the anchor).
    /// Rank is DERIVED: a reference's depth is the length of its below-chain
    /// (`positions::ref_depth`).
    pub below: Option<StepGraphIndex>,
    /// The entry into this position converged — more than one thing (legs and/or refs stacked
    /// above) met here (a merge). A creation-time signal distinct from `approach.len() > 1` (a position
    /// can converge yet resolve to a single leg), so it is stored and PRESERVED, not re-derived.
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

/// How much of its anchor's incoming legs a lane carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LaneCarry {
    /// Nothing descends into this lane (a root chain: remote above a tip, empty top).
    None,
    /// Every leg into the anchor descends through this lane (a plain chain, or a shared
    /// chain all merge lanes converge on).
    All,
    /// This lane carries exactly the legs its [`LaneRec::legs`] statement names — one lane
    /// of a merge.
    Count(usize),
}

/// One lane above a stored anchor: the references sharing an approach at one position.
/// Membership only — order among members stays the below-chain's job.
#[derive(Debug, Clone)]
pub(crate) struct LaneRec {
    /// The reference nodes in this lane, unordered (order by below-chain depth to read).
    pub members: Vec<StepGraphIndex>,
    /// How much of the anchor's legs this lane carries.
    pub carry: LaneCarry,
    /// The legs this lane STATES it carries (`Count` lanes only). Keyed by the full
    /// `(source-pick, parent-slot)` leg: two distinct sources can feed one anchor at the
    /// same slot (and one source at two slots), so both coordinates are needed. Read
    /// filtered against the anchor's LIVE legs, so a stale entry is inert — and reclaims
    /// its leg by itself when surgery revives the same coordinates.
    pub legs: Vec<Leg>,
}

/// The rebase step graph: a [`but_graph::CommitGraph`] arena where PICKS carry ordered
/// parent slots, plus a table of [`RefRecord`]s where REFERENCES carry explicit positions —
/// the CommitGraph is the truth for commits, positions the truth for refs, with no overlap.
/// References are edgeless: native creation authors their positions straight from the
/// placement ledger.
#[derive(Debug, Clone, Default)]
pub(crate) struct StepGraph {
    /// THE arena: `StepGraphIndex::Node(i)` IS `CommitIdx` `i`. Commit ids are the payload
    /// (tombstoning flags a node in place, the node id survives every rewrite), parent
    /// slots are the ordered structure.
    arena: but_graph::CommitGraph,
    /// Each node's pick options, parallel to the arena.
    settings: Vec<PickSettings>,
    refs: Vec<RefRecord>,
    /// THE approach store: lane membership per STORED (unresolved) anchor value. Which legs
    /// descend into a reference's position lives here and only here — authored by
    /// [`Self::set_position`]/[`Self::join_lane_of`], carried by [`Self::rekey_position`],
    /// renamed by [`Self::rename_legs`], read via `positions::ref_approach`.
    lanes: HashMap<StepGraphIndex, Vec<LaneRec>>,
}

impl StepGraph {
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
            lanes: HashMap::new(),
        }
    }

    /// THE arena, read-only — the write-through seam projects it after a rebase.
    pub(crate) fn arena(&self) -> &but_graph::CommitGraph {
        &self.arena
    }

    /// Add `step` to the node arena and return its stable id. References do not belong here —
    /// use [`Self::add_reference`].
    pub(crate) fn add_node(&mut self, step: Step) -> StepGraphIndex {
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
        StepGraphIndex::Node(i)
    }

    /// Replace the node payload at `node` with `step` — a pick decomposes into id and
    /// settings, [`Step::None`] tombstones the payload (settings go stale, not cleared).
    pub(crate) fn set_step(&mut self, node: StepGraphIndex, step: Step) {
        let StepGraphIndex::Node(i) = node else {
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
    pub(crate) fn commit_id(&self, node: StepGraphIndex) -> Option<gix::ObjectId> {
        match node {
            StepGraphIndex::Node(i) => self.arena.node_payload(i),
            StepGraphIndex::Ref(_) => None,
        }
    }

    /// Rewrite the commit id of the pick at `node` IN PLACE — THE rebase write: the node id,
    /// its parent array, its settings, and every position naming it all survive unchanged.
    pub(crate) fn set_commit_id(&mut self, node: StepGraphIndex, id: gix::ObjectId) {
        let StepGraphIndex::Node(i) = node else {
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
        node: StepGraphIndex,
        parents: Option<Vec<gix::ObjectId>>,
    ) {
        let StepGraphIndex::Node(i) = node else {
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
    ) -> StepGraphIndex {
        self.refs.push(RefRecord {
            refname,
            mutable,
            live: true,
            position: None,
        });
        StepGraphIndex::Ref(self.refs.len() - 1)
    }

    /// The reference payload at `node` — `Some` iff it names a live (non-deleted) reference.
    pub(crate) fn reference(&self, node: StepGraphIndex) -> Option<(&gix::refs::FullName, bool)> {
        let StepGraphIndex::Ref(i) = node else {
            return None;
        };
        let record = self.refs.get(i)?;
        record.live.then_some((&record.refname, record.mutable))
    }

    /// `true` iff `node` is a live reference.
    pub(crate) fn is_reference(&self, node: StepGraphIndex) -> bool {
        self.reference(node).is_some()
    }

    /// `true` iff `node` is a pick — `false` for tombstones and references.
    pub(crate) fn is_pick(&self, node: StepGraphIndex) -> bool {
        self.commit_id(node).is_some()
    }

    /// All live references, ascending by id.
    pub(crate) fn references(
        &self,
    ) -> impl Iterator<Item = (StepGraphIndex, &gix::refs::FullName, bool)> + '_ {
        self.refs.iter().enumerate().filter_map(|(i, record)| {
            record
                .live
                .then_some((StepGraphIndex::Ref(i), &record.refname, record.mutable))
        })
    }

    /// All reference ids — live AND dead — ascending. Dead references still carry their
    /// retained name and position (see [`RefRecord`]).
    pub(crate) fn ref_indices(&self) -> impl Iterator<Item = StepGraphIndex> + '_ {
        (0..self.refs.len()).map(StepGraphIndex::Ref)
    }

    /// The full record of the reference at `node`, including dead ones — rebuilds need the
    /// retained payload.
    pub(crate) fn reference_record(&self, node: StepGraphIndex) -> Option<&RefRecord> {
        match node {
            StepGraphIndex::Ref(i) => self.refs.get(i),
            StepGraphIndex::Node(_) => None,
        }
    }

    /// Rename (or resurrect) the reference at `node` in place; its position is untouched.
    pub(crate) fn set_reference(
        &mut self,
        node: StepGraphIndex,
        refname: gix::refs::FullName,
        mutable: bool,
    ) {
        let StepGraphIndex::Ref(i) = node else {
            panic!("BUG: only references can be renamed");
        };
        let record = &mut self.refs[i];
        record.refname = refname;
        record.mutable = mutable;
        record.live = true;
    }

    /// Delete the reference at `node`: it goes dead in place, RETAINING its name and
    /// position so stale selectors keep normalizing and rebuilds keep carrying it.
    pub(crate) fn tombstone_reference(&mut self, node: StepGraphIndex) {
        let StepGraphIndex::Ref(i) = node else {
            panic!("BUG: only references can be tombstoned");
        };
        self.refs[i].live = false;
    }

    /// The step at `node` as an owned view — the read for whole-step consumers, synthesized
    /// from the payload: id plus settings make a `Step::Pick`, a `None` id a `Step::None`,
    /// a reference entry `Step::Reference` while live and `Step::None` once dead.
    pub(crate) fn step_view(&self, node: StepGraphIndex) -> Step {
        match node {
            StepGraphIndex::Node(i) => match self.arena.node_payload(i) {
                Some(id) => Step::Pick(self.settings[i].pick(id)),
                None => Step::None,
            },
            StepGraphIndex::Ref(i) => {
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
    pub(crate) fn position_of(&self, node: StepGraphIndex) -> Option<RefPosition> {
        match node {
            StepGraphIndex::Ref(i) => self.refs.get(i)?.position.clone(),
            StepGraphIndex::Node(_) => None,
        }
    }

    fn position_slot(&mut self, node: StepGraphIndex) -> &mut Option<RefPosition> {
        match node {
            StepGraphIndex::Ref(i) => &mut self.refs[i].position,
            StepGraphIndex::Node(_) => panic!("BUG: only references hold positions"),
        }
    }

    /// Author a FRESH position for `node`: `approach` is the lane intent, classified against
    /// `anchor`'s CURRENT legs — empty opens (or joins) the root lane, the whole live set the
    /// shared `All` lane, any other set a `Count` lane stating exactly those legs. Only
    /// correct when the anchor's legs are already complete — never use to re-place an
    /// existing position wholesale.
    pub(crate) fn set_position(
        &mut self,
        node: StepGraphIndex,
        anchor: StepGraphIndex,
        approach: &[(StepGraphIndex, usize)],
        ambiguous: bool,
        below: Option<StepGraphIndex>,
    ) {
        let live = match crate::graph_rebase::positions::resolve_to_pick(self, anchor) {
            Some(pick) => crate::graph_rebase::positions::legs_into_pick(self, pick),
            None => Vec::new(),
        };
        let (carry, legs) = if approach.is_empty() {
            (LaneCarry::None, Vec::new())
        } else {
            let approach_set: HashSet<_> = approach.iter().copied().collect();
            let live_set: HashSet<_> = live.iter().copied().collect();
            if approach_set == live_set {
                (LaneCarry::All, Vec::new())
            } else {
                let mut legs = approach.to_vec();
                legs.sort_unstable();
                legs.dedup();
                (LaneCarry::Count(legs.len()), legs)
            }
        };
        if let Some(previous) = self.position_of(node) {
            self.lane_remove(node, previous.anchor);
        }
        self.lane_insert(node, anchor, carry, legs);
        *self.position_slot(node) = Some(RefPosition {
            anchor,
            ambiguous,
            below,
        });
    }

    /// Join `node` into the lane CONTAINING `mate` — direct membership, not legs-equality —
    /// sitting on `below`, copying the mate's anchor and ambiguity.
    pub(crate) fn join_lane_of(
        &mut self,
        node: StepGraphIndex,
        mate: StepGraphIndex,
        below: Option<StepGraphIndex>,
    ) {
        let Some(m) = self.position_of(mate) else {
            return;
        };
        if let Some(previous) = self.position_of(node) {
            self.lane_remove(node, previous.anchor);
        }
        let joined = self
            .lanes
            .entry(m.anchor)
            .or_default()
            .iter_mut()
            .find(|lane| lane.members.contains(&mate))
            .map(|lane| lane.members.push(node))
            .is_some();
        debug_assert!(joined, "positioned mate {mate} must own a lane");
        if !joined {
            self.lane_insert(node, m.anchor, LaneCarry::All, Vec::new());
        }
        *self.position_slot(node) = Some(RefPosition {
            anchor: m.anchor,
            ambiguous: m.ambiguous,
            below,
        });
    }

    /// Re-key `node`'s position onto `new_anchor`, carrying its CURRENT lane record — the
    /// carry and legs as maintained through edge surgery. Below and ambiguity are preserved.
    pub(crate) fn rekey_position(&mut self, node: StepGraphIndex, new_anchor: StepGraphIndex) {
        let Some(stored) = self.position_of(node) else {
            return;
        };
        if stored.anchor == new_anchor {
            return;
        }
        let lane_data = self.lanes.get(&stored.anchor).and_then(|lanes| {
            lanes
                .iter()
                .find(|lane| lane.members.contains(&node))
                .map(|lane| (lane.carry.clone(), lane.legs.clone()))
        });
        self.lane_remove(node, stored.anchor);
        match lane_data {
            Some((carry, legs)) => self.lane_insert(node, new_anchor, carry, legs),
            None => {
                debug_assert!(false, "positioned node {node} must own a lane");
                self.lane_insert(node, new_anchor, LaneCarry::All, Vec::new());
            }
        }
        if let Some(a) = self.position_slot(node).as_mut() {
            a.anchor = new_anchor;
        }
    }

    /// Re-hang `node` onto `below` — an adjacency statement only; anchor and lane
    /// membership are untouched.
    pub(crate) fn set_below(&mut self, node: StepGraphIndex, below: Option<StepGraphIndex>) {
        if let Some(stored) = self.position_slot(node).as_mut() {
            stored.below = below;
        }
    }

    /// Point a DEAD reference's retained position at `anchor` — the bare retention pointer
    /// stale selectors normalize through. Lane membership, below, and ambiguity are dropped;
    /// live references re-anchor via the arrangement machinery instead.
    pub(crate) fn set_retained_anchor(&mut self, node: StepGraphIndex, anchor: StepGraphIndex) {
        debug_assert!(
            !self.is_reference(node) && matches!(node, StepGraphIndex::Ref(_)),
            "retained anchors belong to dead references"
        );
        if let Some(stored) = self.position_of(node) {
            self.lane_remove(node, stored.anchor);
        }
        *self.position_slot(node) = Some(RefPosition {
            anchor,
            below: None,
            ambiguous: false,
        });
    }

    fn lane_remove(&mut self, node: StepGraphIndex, key: StepGraphIndex) {
        let Some(lanes) = self.lanes.get_mut(&key) else {
            return;
        };
        for lane in lanes.iter_mut() {
            lane.members.retain(|&member| member != node);
        }
        lanes.retain(|lane| !lane.members.is_empty());
        if lanes.is_empty() {
            self.lanes.remove(&key);
        }
    }

    fn lane_insert(
        &mut self,
        node: StepGraphIndex,
        key: StepGraphIndex,
        carry: LaneCarry,
        legs: Vec<(StepGraphIndex, usize)>,
    ) {
        let lanes = self.lanes.entry(key).or_default();
        let existing = lanes.iter_mut().find(|lane| match carry {
            // `Count` lanes are identified by their stated legs (same legs => same lane).
            LaneCarry::Count(_) => matches!(lane.carry, LaneCarry::Count(_)) && lane.legs == legs,
            // One `None` and one `All` lane per key.
            _ => lane.carry == carry,
        });
        match existing {
            Some(lane) => lane.members.push(node),
            None => lanes.push(LaneRec {
                members: vec![node],
                carry,
                legs,
            }),
        }
    }

    /// The lane table: every positioned reference's approach statement, keyed by the STORED
    /// (unresolved) anchor value.
    pub(crate) fn lane_table(&self) -> &HashMap<StepGraphIndex, Vec<LaneRec>> {
        &self.lanes
    }

    /// The lane containing the reference at `node`, if it holds a position.
    pub(crate) fn lane_of(&self, node: StepGraphIndex) -> Option<&LaneRec> {
        let stored = match node {
            StepGraphIndex::Ref(i) => self.refs.get(i)?.position.as_ref()?,
            StepGraphIndex::Node(_) => return None,
        };
        self.lanes
            .get(&stored.anchor)?
            .iter()
            .find(|lane| lane.members.contains(&node))
    }

    /// All positioned references — live AND dead — ascending by id.
    pub(crate) fn positioned_refs(
        &self,
    ) -> impl Iterator<Item = (StepGraphIndex, RefPosition)> + '_ {
        self.refs
            .iter()
            .enumerate()
            .filter_map(|(i, record)| record.position.clone().map(|p| (StepGraphIndex::Ref(i), p)))
    }

    /// The leg `old` is now called `new` — its slot renumbered (or re-sourced onto another
    /// pick) by surgery: every lane that carried `old` carries `new` instead.
    pub(crate) fn rename_leg(&mut self, old: Leg, new: Leg) {
        self.rename_legs(&[(old, new)]);
    }

    /// Apply several leg renames SIMULTANEOUSLY: every lane leg is matched against the
    /// pre-rename names once, so shifting slots in a renumber can't collide mid-flight.
    pub(crate) fn rename_legs(&mut self, renames: &[(Leg, Leg)]) {
        for lanes in self.lanes.values_mut() {
            for lane in lanes.iter_mut() {
                let mut changed = false;
                for leg in lane.legs.iter_mut() {
                    if let Some((_, new)) = renames.iter().find(|(old, _)| old == leg) {
                        *leg = *new;
                        changed = true;
                    }
                }
                if changed {
                    lane.legs.sort_unstable();
                    lane.legs.dedup();
                    if let LaneCarry::Count(_) = lane.carry {
                        lane.carry = LaneCarry::Count(lane.legs.len());
                    }
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
    pub(crate) fn parents(&self, node: StepGraphIndex) -> Vec<StepGraphIndex> {
        match node {
            StepGraphIndex::Node(i) => self
                .arena
                .parent_indices(i)
                .into_iter()
                .map(StepGraphIndex::Node)
                .collect(),
            StepGraphIndex::Ref(_) => Vec::new(),
        }
    }

    /// Rewrite `child`'s parent array through `f` — the single seam every parent mutation
    /// flows through into the arena's slot write.
    fn update_parents<R>(
        &mut self,
        child: StepGraphIndex,
        f: impl FnOnce(&mut Vec<StepGraphIndex>) -> R,
    ) -> R {
        let StepGraphIndex::Node(i) = child else {
            panic!("references are edgeless — no parent array");
        };
        let mut parents: Vec<StepGraphIndex> = self
            .arena
            .parent_indices(i)
            .into_iter()
            .map(StepGraphIndex::Node)
            .collect();
        let result = f(&mut parents);
        let targets = parents
            .into_iter()
            .map(|parent| match parent {
                StepGraphIndex::Node(j) => j,
                StepGraphIndex::Ref(_) => {
                    panic!("references are edgeless — they cannot be parents")
                }
            })
            .collect();
        self.arena.set_parents(i, targets);
        result
    }

    /// How many parent slots `node` has.
    pub(crate) fn parent_count(&self, node: StepGraphIndex) -> usize {
        self.parents(node).len()
    }

    /// Every parent-array entry naming `node`, as `(child, slot)` legs, sorted — the derived
    /// children read.
    pub(crate) fn incoming_legs(&self, node: StepGraphIndex) -> Vec<Leg> {
        let mut legs = Vec::new();
        for i in 0..self.arena.node_count() {
            let child = StepGraphIndex::Node(i);
            for (slot, parent) in self.arena.parent_indices(i).into_iter().enumerate() {
                if StepGraphIndex::Node(parent) == node {
                    legs.push((child, slot));
                }
            }
        }
        legs.sort_unstable();
        legs
    }

    /// Append `parent` as `child`'s last parent slot; returns the slot.
    pub(crate) fn push_parent(&mut self, child: StepGraphIndex, parent: StepGraphIndex) -> usize {
        self.update_parents(child, |parents| {
            parents.push(parent);
            parents.len() - 1
        })
    }

    /// Insert `parent` at `slot` of `child` (clamped to the array end); later slots shift up
    /// with their statements. Returns the slot actually used.
    pub(crate) fn insert_parent(
        &mut self,
        child: StepGraphIndex,
        slot: usize,
        parent: StepGraphIndex,
    ) -> usize {
        let len = self.parent_count(child);
        let slot = slot.min(len);
        let renames: Vec<_> = (slot..len).map(|s| ((child, s), (child, s + 1))).collect();
        self.rename_legs(&renames);
        self.update_parents(child, |parents| parents.insert(slot, parent));
        slot
    }

    /// Remove `child`'s parent at `slot`, returning it; later slots shift down with their
    /// statements, and statements naming the removed slot are dropped.
    pub(crate) fn remove_parent(
        &mut self,
        child: StepGraphIndex,
        slot: usize,
    ) -> Option<StepGraphIndex> {
        let len = self.parent_count(child);
        if slot >= len {
            return None;
        }
        let target = self.update_parents(child, |parents| parents.remove(slot));
        self.retain_legs(|&leg| leg != (child, slot));
        let renames: Vec<_> = (slot + 1..len)
            .map(|s| ((child, s), (child, s - 1)))
            .collect();
        self.rename_legs(&renames);
        Some(target)
    }

    /// Re-point `child`'s parent at `slot` onto `new_parent`. The slot — and so the
    /// statement name — is untouched: chains stated on the leg follow it to its new target.
    pub(crate) fn replace_parent(
        &mut self,
        child: StepGraphIndex,
        slot: usize,
        new_parent: StepGraphIndex,
    ) {
        self.update_parents(child, |parents| match parents.get_mut(slot) {
            Some(entry) => *entry = new_parent,
            None => debug_assert!(false, "replace_parent: {child} has no slot {slot}"),
        });
    }

    /// Move `from`'s whole parent array onto `to` (which must have none); statements follow
    /// slot-for-slot.
    pub(crate) fn transplant_parents(&mut self, from: StepGraphIndex, to: StepGraphIndex) {
        debug_assert_eq!(
            self.parent_count(to),
            0,
            "transplant target {to} already has parents"
        );
        let parents = self.update_parents(from, std::mem::take);
        let renames: Vec<_> = (0..parents.len()).map(|s| ((from, s), (to, s))).collect();
        self.update_parents(to, |slot| *slot = parents);
        self.rename_legs(&renames);
    }

    /// Re-target every parent-array entry naming `from` onto `to`, slots preserved —
    /// statement names are `(source, slot)`, so they stay valid untouched.
    pub(crate) fn redirect_children(&mut self, from: StepGraphIndex, to: StepGraphIndex) {
        for i in 0..self.arena.node_count() {
            let child = StepGraphIndex::Node(i);
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

    /// Empty `child`'s parent array, returning it. Lane statements naming the drained slots
    /// are DELIBERATELY untouched: the caller re-states the orphaned names onto their new
    /// carrier itself (the below-insert path renames them onto the segment's parent-most).
    pub(crate) fn drain_parents(&mut self, child: StepGraphIndex) -> Vec<StepGraphIndex> {
        self.update_parents(child, std::mem::take)
    }

    /// Drop every lane statement `keep` rejects, keeping `Count` carries consistent.
    fn retain_legs(&mut self, keep: impl Fn(&Leg) -> bool) {
        for lanes in self.lanes.values_mut() {
            for lane in lanes.iter_mut() {
                let before = lane.legs.len();
                lane.legs.retain(&keep);
                if lane.legs.len() != before
                    && let LaneCarry::Count(_) = lane.carry
                {
                    lane.carry = LaneCarry::Count(lane.legs.len());
                }
            }
        }
    }

    /// All node-arena ids (picks and tombstones), ascending. References are NOT included —
    /// see [`Self::references`] and [`Self::ref_indices`].
    pub(crate) fn node_indices(&self) -> impl Iterator<Item = StepGraphIndex> + '_ {
        (0..self.arena.node_count()).map(StepGraphIndex::Node)
    }

    /// The ARENA nodes no parent array names — the child-less tips, ascending. References
    /// never appear here: they are edgeless by construction, and the consumers
    /// (head discovery) want picks and tombstones only.
    pub(crate) fn tips(&self) -> impl Iterator<Item = StepGraphIndex> + '_ {
        let referenced: HashSet<StepGraphIndex> = (0..self.arena.node_count())
            .flat_map(|i| self.arena.parent_indices(i))
            .map(StepGraphIndex::Node)
            .collect();
        (0..self.arena.node_count())
            .map(StepGraphIndex::Node)
            .filter(move |node| !referenced.contains(node))
    }
}
