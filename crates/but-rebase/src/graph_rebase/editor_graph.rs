//! An owned arena graph for rebase steps: the ROW arena's payload is the COMMIT ID
//! (`None` = tombstone) with pick options in a parallel settings table, each row bearing
//! an ORDERED PARENT ARRAY — a parent's position in the array IS its parent order, dense by
//! construction. References live in the REF table and bear positions. Children are DERIVED
//! (a reverse scan of the parent arrays), never stored. Nothing is ever removed (a removed
//! pick becomes a `None` payload, a removed reference goes dead in place), so ids are
//! stable by construction. [`Step`] and [`Pick`] are BOUNDARY VALUE types: synthesized by
//! [`EditorGraph::step_view`], decomposed by [`EditorGraph::add_row`]/[`EditorGraph::set_step`].

use std::collections::{HashMap, HashSet};

use but_core::commit::SignCommit;

use crate::graph_rebase::{
    Pick, Step,
    cherry_pick::{PickMode, TreeMergeMode},
};

/// The stable identifier of an editor-graph entry. Two namespaces, one id type: `Row` points
/// into the pick arena (its parent array is its truth), `Ref` into the reference table (a
/// position is its truth) — so a selector can address either without knowing which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EditorGraphIndex {
    /// A pick or its tombstone in the row arena.
    Row(usize),
    /// A reference (live or dead) in the ref table.
    Ref(usize),
}

impl std::fmt::Display for EditorGraphIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorGraphIndex::Row(i) => write!(f, "p{i}"),
            EditorGraphIndex::Ref(i) => write!(f, "r{i}"),
        }
    }
}

/// One incoming child edge of a pick, named POSITIONALLY as `(source pick, parent-slot)`.
/// Carry statements name edges this way, so an edge removed and re-created at the same coordinates
/// is the SAME statement — see [`Carry::Edges`].
pub(crate) type Edge = (EditorGraphIndex, usize);

/// Where a reference sits — a VIEW synthesized from the arrangement table by
/// [`EditorGraph::position_of`], not stored data: `on` is the table key of the group holding
/// the reference, `below` the member underneath it (the previous member in its group, or the
/// group's attach), `ambiguous` the per-ref record flag. Derived reads live in `positions`:
/// `ref_depth` (rank), `edges_through` (entering edges), `resolve_to_pick` (the row,
/// followed through tombstones).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefPosition {
    /// The row this reference resolves to (a pick, or its tombstone after deletion) — the
    /// commit the ref points at, reached lazily through tombstones at read time.
    pub on: EditorGraphIndex,
    /// The reference directly underneath in the physical stack (`None` = sits directly on the row).
    pub below: Option<EditorGraphIndex>,
    /// The entry into this position converged — more than one thing (edges and/or refs stacked
    /// above) met here (a merge). A creation-time signal distinct from the entering-edge count (a
    /// position can converge yet resolve to a single edge), so it is stored and PRESERVED, not
    /// re-derived.
    pub ambiguous: bool,
}

/// One reference: name, mutability, liveness, convergence flag. Deletion flips `live` and
/// RETAINS name and position — retention is load-bearing (stale selectors normalize through
/// dead refs, rebuilds carry them).
#[derive(Debug, Clone)]
pub(crate) struct RefRecord {
    /// The full reference name.
    pub refname: gix::refs::FullName,
    /// Whether the rebase may move this reference.
    pub mutable: bool,
    /// `false` once the reference is deleted; the record stays.
    pub live: bool,
    /// See [`RefPosition::ambiguous`] — kept on the record because it is a preserved
    /// creation-time signal, not table structure.
    pub ambiguous: bool,
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

/// How much of its pick's incoming edges a reference group carries — which edges ENTER
/// THROUGH the group's references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Carry {
    /// Nothing descends into this group (a root group: remote above a tip, empty top).
    None,
    /// Every edge into the pick descends through this group (a plain stack, or the shared
    /// group all merge groups converge on).
    All,
    /// Exactly these stated edges — one group of a merge. Kept sorted and deduplicated,
    /// keyed by the full `(source-pick, parent-slot)` edge: two distinct sources can feed
    /// one pick at the same slot (and one source at two slots), so both coordinates are
    /// needed. Read filtered against the pick's LIVE edges, so a stale entry is inert —
    /// and reclaims its edge by itself when surgery revives the same coordinates.
    Edges(Vec<Edge>),
}

/// One group of references standing on a stored key: an ordered bottom→top run of members
/// sharing one [`Carry`]. Order and stacking are LIST STRUCTURE — a member's below is the
/// previous member, its rank its index plus the height of the attach walk underneath
/// (`positions::ref_depth`).
#[derive(Debug, Clone)]
pub(crate) struct RefGroup {
    /// The reference entries, bottom→top: `members[0]` sits on `attach` (or the key's pick).
    pub members: Vec<EditorGraphIndex>,
    /// How much of the pick's edges this group carries.
    pub carry: Carry,
    /// The reference this group's bottom member sits on — a member of ANOTHER group
    /// resolving to the same pick (`None` = directly on the pick). Group boundaries are
    /// carry boundaries: a branch point or a carry change starts a new group.
    pub attach: Option<EditorGraphIndex>,
}

/// The editor's graph: a [`but_graph::CommitGraph`] arena where PICKS carry ordered
/// parent slots, plus a table of [`RefRecord`]s where REFERENCES carry explicit positions —
/// the arena is the truth for commits, positions the truth for refs, with no overlap.
/// References are edgeless: native creation authors their positions straight from the
/// placement ledger.
#[derive(Debug, Clone, Default)]
pub(crate) struct EditorGraph {
    /// THE arena: `EditorGraphIndex::Row(i)` IS `RowIdx` `i`. Commit ids are the payload
    /// (tombstoning flags a row in place, the row id survives every rewrite), parent
    /// slots are the ordered structure.
    arena: but_graph::CommitGraph,
    /// Each row's pick options, parallel to the arena.
    settings: Vec<PickSettings>,
    refs: Vec<RefRecord>,
    /// THE position store: per STORED (unresolved) key, the reference groups standing on it.
    /// A reference's position (its `on`, its below, its rank, its entering edges) is all
    /// list structure here — authored by [`Self::set_position`]/[`Self::join_group_of`],
    /// carried by [`Self::rekey_position`], renamed by [`Self::rename_edges`], read via
    /// [`Self::position_of`] and `positions::edges_through`.
    arrangement: HashMap<EditorGraphIndex, Vec<RefGroup>>,
}

impl EditorGraph {
    /// Adopt `arena` wholesale as THE arena — full commit payloads (flags, refs, generation)
    /// survive, which the write-through put-back depends on. The caller must have normalized
    /// every parent slot to PRESENT (the editor's slot invariant) and follows up with
    /// per-row settings via [`Self::set_step`].
    pub(crate) fn adopt(arena: but_graph::CommitGraph) -> Self {
        let settings = vec![PickSettings::default(); arena.row_count()];
        Self {
            arena,
            settings,
            refs: Vec::new(),
            arrangement: HashMap::new(),
        }
    }

    /// THE arena, read-only — the write-through seam projects it after a rebase.
    pub(crate) fn arena(&self) -> &but_graph::CommitGraph {
        &self.arena
    }

    /// Add `step` to the row arena and return its stable id. References do not belong here —
    /// use [`Self::add_reference`].
    pub(crate) fn add_row(&mut self, step: Step) -> EditorGraphIndex {
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
        let i = self.arena.add_row(id);
        self.settings.push(settings);
        debug_assert_eq!(
            self.settings.len(),
            self.arena.row_count(),
            "settings table fell out of step with the arena"
        );
        EditorGraphIndex::Row(i)
    }

    /// Replace the row payload at `entry` with `step` — a pick decomposes into id and
    /// settings, [`Step::None`] tombstones the payload (settings go stale, not cleared).
    pub(crate) fn set_step(&mut self, entry: EditorGraphIndex, step: Step) {
        let EditorGraphIndex::Row(i) = entry else {
            panic!("BUG: references live in the ref table, not the step arena");
        };
        match step {
            Step::Pick(pick) => {
                let (id, settings) = PickSettings::split(pick);
                self.arena.set_row_id(i, Some(id));
                self.settings[i] = settings;
            }
            Step::None => self.arena.set_row_id(i, None),
            Step::Reference { .. } => {
                panic!("BUG: references live in the ref table, not the step arena")
            }
        }
    }

    /// The commit id of the pick at `entry` — `None` for tombstones and references. THE fast
    /// payload read; whole-step consumers use [`Self::step_view`].
    pub(crate) fn commit_id(&self, entry: EditorGraphIndex) -> Option<gix::ObjectId> {
        match entry {
            EditorGraphIndex::Row(i) => self.arena.row_payload(i),
            EditorGraphIndex::Ref(_) => None,
        }
    }

    /// Rewrite the commit id of the pick at `entry` IN PLACE — THE rebase write: the row id,
    /// its parent array, its settings, and every position naming it all survive unchanged.
    pub(crate) fn set_commit_id(&mut self, entry: EditorGraphIndex, id: gix::ObjectId) {
        let EditorGraphIndex::Row(i) = entry else {
            panic!("BUG: only picks carry commit ids");
        };
        debug_assert!(
            self.arena.row_payload(i).is_some(),
            "tombstones have no commit id"
        );
        self.arena.set_commit_id(i, id);
    }

    /// Overwrite the preserved parents of the pick at `entry` (see
    /// [`Pick::preserved_parents`]).
    pub(crate) fn set_preserved_parents(
        &mut self,
        entry: EditorGraphIndex,
        parents: Option<Vec<gix::ObjectId>>,
    ) {
        let EditorGraphIndex::Row(i) = entry else {
            panic!("BUG: only picks carry preserved parents");
        };
        debug_assert!(
            self.arena.row_payload(i).is_some(),
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
            ambiguous: false,
        });
        EditorGraphIndex::Ref(self.refs.len() - 1)
    }

    /// The reference payload at `entry` — `Some` iff it names a live (non-deleted) reference.
    pub(crate) fn reference(
        &self,
        entry: EditorGraphIndex,
    ) -> Option<(&gix::refs::FullName, bool)> {
        let EditorGraphIndex::Ref(i) = entry else {
            return None;
        };
        let record = self.refs.get(i)?;
        record.live.then_some((&record.refname, record.mutable))
    }

    /// `true` iff `entry` is a live reference.
    pub(crate) fn is_reference(&self, entry: EditorGraphIndex) -> bool {
        self.reference(entry).is_some()
    }

    /// `true` iff `entry` is a pick — `false` for tombstones and references.
    pub(crate) fn is_pick(&self, entry: EditorGraphIndex) -> bool {
        self.commit_id(entry).is_some()
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

    /// The full record of the reference at `entry`, including dead ones — rebuilds need the
    /// retained payload.
    pub(crate) fn reference_record(&self, entry: EditorGraphIndex) -> Option<&RefRecord> {
        match entry {
            EditorGraphIndex::Ref(i) => self.refs.get(i),
            EditorGraphIndex::Row(_) => None,
        }
    }

    /// Rename (or resurrect) the reference at `entry` in place; its position is untouched.
    pub(crate) fn set_reference(
        &mut self,
        entry: EditorGraphIndex,
        refname: gix::refs::FullName,
        mutable: bool,
    ) {
        let EditorGraphIndex::Ref(i) = entry else {
            panic!("BUG: only references can be renamed");
        };
        let record = &mut self.refs[i];
        record.refname = refname;
        record.mutable = mutable;
        record.live = true;
    }

    /// Delete the reference at `entry`: it goes dead in place, RETAINING its name and
    /// position so stale selectors keep normalizing and rebuilds keep carrying it.
    pub(crate) fn tombstone_reference(&mut self, entry: EditorGraphIndex) {
        let EditorGraphIndex::Ref(i) = entry else {
            panic!("BUG: only references can be tombstoned");
        };
        self.refs[i].live = false;
    }

    /// The step at `entry` as an owned view — the read for whole-step consumers, synthesized
    /// from the payload: id plus settings make a `Step::Pick`, a `None` id a `Step::None`,
    /// a reference entry `Step::Reference` while live and `Step::None` once dead.
    pub(crate) fn step_view(&self, entry: EditorGraphIndex) -> Step {
        match entry {
            EditorGraphIndex::Row(i) => match self.arena.row_payload(i) {
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

    /// The position of the reference at `entry`, live or dead — synthesized from the
    /// arrangement table (see [`RefPosition`]). `None` until placed.
    pub(crate) fn position_of(&self, entry: EditorGraphIndex) -> Option<RefPosition> {
        let (key, g, i) = self.locate(entry)?;
        let group = &self.arrangement[&key][g];
        let below = if i > 0 {
            Some(group.members[i - 1])
        } else {
            group.attach
        };
        let ambiguous = match entry {
            EditorGraphIndex::Ref(r) => self.refs[r].ambiguous,
            EditorGraphIndex::Row(_) => false,
        };
        Some(RefPosition {
            on: key,
            below,
            ambiguous,
        })
    }

    /// Where `entry` sits in the arrangement table: `(key, group index, member index)`.
    fn locate(&self, entry: EditorGraphIndex) -> Option<(EditorGraphIndex, usize, usize)> {
        self.arrangement.iter().find_map(|(&key, groups)| {
            groups.iter().enumerate().find_map(|(g, group)| {
                group
                    .members
                    .iter()
                    .position(|&m| m == entry)
                    .map(|i| (key, g, i))
            })
        })
    }

    fn set_ambiguous(&mut self, entry: EditorGraphIndex, ambiguous: bool) {
        let EditorGraphIndex::Ref(i) = entry else {
            panic!("BUG: only references hold positions");
        };
        self.refs[i].ambiguous = ambiguous;
    }

    /// Classify a position's entering-edge intent against `on`'s CURRENT edges: empty is a
    /// root, the whole live set the shared `All` carry, any other set an `Edges` carry
    /// stating exactly those edges.
    fn classify(&self, on: EditorGraphIndex, entering: &[Edge]) -> Carry {
        if entering.is_empty() {
            return Carry::None;
        }
        let live = match crate::graph_rebase::positions::resolve_to_pick(self, on) {
            Some(pick) => crate::graph_rebase::positions::edges_into(self, pick),
            None => Vec::new(),
        };
        let edge_set: HashSet<_> = entering.iter().copied().collect();
        let live_set: HashSet<_> = live.iter().copied().collect();
        if edge_set == live_set {
            Carry::All
        } else {
            let mut edges = entering.to_vec();
            edges.sort_unstable();
            edges.dedup();
            Carry::Edges(edges)
        }
    }

    /// Take `entry` out of the table, its dependents RIDING: members stacked above it in its
    /// group split off as their own group attached to `entry` (they keep sitting on it,
    /// wherever it goes next), and groups attached to `entry` stay attached. The caller
    /// re-places the entry immediately.
    fn extract(&mut self, entry: EditorGraphIndex) {
        let Some((key, g, i)) = self.locate(entry) else {
            return;
        };
        let groups = self.arrangement.get_mut(&key).expect("just located");
        let above = groups[g].members.split_off(i + 1);
        groups[g].members.pop();
        if !above.is_empty() {
            let carry = groups[g].carry.clone();
            groups.push(RefGroup {
                members: above,
                carry,
                attach: Some(entry),
            });
        }
        if groups[g].members.is_empty() {
            groups.remove(g);
        }
        if groups.is_empty() {
            self.arrangement.remove(&key);
        }
    }

    /// Put the (extracted or fresh) `entry` into the table at `key`: onto `attach`'s group
    /// when it lands on a group top with the same carry, as a fresh group otherwise.
    fn place(
        &mut self,
        entry: EditorGraphIndex,
        key: EditorGraphIndex,
        carry: Carry,
        attach: Option<EditorGraphIndex>,
    ) {
        let groups = self.arrangement.entry(key).or_default();
        let joined = attach.and_then(|b| {
            groups
                .iter()
                .position(|group| group.members.last() == Some(&b) && group.carry == carry)
        });
        match joined {
            Some(g) => groups[g].members.push(entry),
            None => groups.push(RefGroup {
                members: vec![entry],
                carry,
                attach,
            }),
        }
        self.normalize(key);
    }

    /// Merge groups that stand contiguously with the same carry: a group attached to the TOP
    /// member of another group at the same key with an equal carry is its continuation —
    /// group boundaries stay canonical maximal same-carry runs.
    fn normalize(&mut self, key: EditorGraphIndex) {
        let Some(groups) = self.arrangement.get_mut(&key) else {
            return;
        };
        loop {
            let merge = groups.iter().enumerate().find_map(|(upper_idx, upper)| {
                let b = upper.attach?;
                groups.iter().enumerate().find_map(|(lower_idx, lower)| {
                    (lower_idx != upper_idx
                        && lower.members.last() == Some(&b)
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

    /// Splice `entry` out of the physical stack, dependents HEALING past it: members above it
    /// in its group close the gap, and groups attached to it re-hang onto what it sat on.
    /// The entry itself stays placed as a single-member BRANCH group at the same spot (same
    /// key, attached to its old below, carry copied) — the retained position a deletion
    /// leaves behind.
    pub(crate) fn splice(&mut self, entry: EditorGraphIndex) {
        let Some((key, g, i)) = self.locate(entry) else {
            return;
        };
        let groups = self.arrangement.get_mut(&key).expect("just located");
        let below = if i > 0 {
            Some(groups[g].members[i - 1])
        } else {
            groups[g].attach
        };
        groups[g].members.remove(i);
        let carry = groups[g].carry.clone();
        if groups[g].members.is_empty() {
            groups.remove(g);
        }
        for groups in self.arrangement.values_mut() {
            for group in groups.iter_mut() {
                if group.attach == Some(entry) {
                    group.attach = below;
                }
            }
        }
        self.arrangement.entry(key).or_default().push(RefGroup {
            members: vec![entry],
            carry,
            attach: below,
        });
        self.normalize(key);
    }

    /// Author a position for `entry`: `entering` is the carry intent — the edges meant to
    /// enter through it — classified against `on`'s CURRENT edges (see [`Self::classify`]).
    /// Members stacked on the entry ride along; only correct when the entry's edges are
    /// already complete.
    pub(crate) fn set_position(
        &mut self,
        entry: EditorGraphIndex,
        on: EditorGraphIndex,
        entering: &[Edge],
        ambiguous: bool,
        below: Option<EditorGraphIndex>,
    ) {
        let carry = self.classify(on, entering);
        self.extract(entry);
        self.place(entry, on, carry, below);
        self.set_ambiguous(entry, ambiguous);
    }

    /// Join `entry` into the group of `mate` — copying the mate's key, carry, and ambiguity —
    /// sitting on `below`.
    pub(crate) fn join_group_of(
        &mut self,
        entry: EditorGraphIndex,
        mate: EditorGraphIndex,
        below: Option<EditorGraphIndex>,
    ) {
        let Some((key, g, _)) = self.locate(mate) else {
            return;
        };
        let carry = self.arrangement[&key][g].carry.clone();
        let ambiguous = match mate {
            EditorGraphIndex::Ref(i) => self.refs[i].ambiguous,
            EditorGraphIndex::Row(_) => false,
        };
        self.extract(entry);
        self.place(entry, key, carry, below);
        self.set_ambiguous(entry, ambiguous);
    }

    /// Re-key `entry`'s position onto `onto`, carrying its CURRENT carry — as maintained
    /// through edge surgery. Below and ambiguity are preserved; members stacked on the entry
    /// ride along.
    pub(crate) fn rekey_position(&mut self, entry: EditorGraphIndex, onto: EditorGraphIndex) {
        let Some(stored) = self.position_of(entry) else {
            return;
        };
        if stored.on == onto {
            return;
        }
        let carry = self.carry_of(entry).cloned().unwrap_or(Carry::All);
        self.extract(entry);
        self.place(entry, onto, carry, stored.below);
    }

    /// Re-hang `entry` onto `below` — an adjacency statement only; its key, carry, and
    /// ambiguity are untouched, and members stacked on the entry ride along.
    pub(crate) fn set_below(&mut self, entry: EditorGraphIndex, below: Option<EditorGraphIndex>) {
        let Some(stored) = self.position_of(entry) else {
            return;
        };
        if stored.below == below {
            return;
        }
        let carry = self.carry_of(entry).cloned().unwrap_or(Carry::All);
        self.extract(entry);
        self.place(entry, stored.on, carry, below);
    }

    /// Point a DEAD reference's retained position at `on` — the bare retention pointer
    /// stale selectors normalize through. Its old spot heals (dependents re-hang past it),
    /// and it lands as a bare root; live references re-place via the arrangement machinery
    /// instead.
    pub(crate) fn set_retained_position(&mut self, entry: EditorGraphIndex, on: EditorGraphIndex) {
        debug_assert!(
            !self.is_reference(entry) && matches!(entry, EditorGraphIndex::Ref(_)),
            "retained positions belong to dead references"
        );
        self.splice(entry);
        self.extract(entry);
        self.place(entry, on, Carry::None, None);
        self.set_ambiguous(entry, false);
    }

    /// The carry of the group holding the reference at `entry`, if it holds a position.
    pub(crate) fn carry_of(&self, entry: EditorGraphIndex) -> Option<&Carry> {
        let (key, g, _) = self.locate(entry)?;
        Some(&self.arrangement[&key][g].carry)
    }

    /// All positioned references — live AND dead — ascending by id.
    pub(crate) fn positioned_refs(
        &self,
    ) -> impl Iterator<Item = (EditorGraphIndex, RefPosition)> + '_ {
        (0..self.refs.len()).filter_map(|i| {
            let entry = EditorGraphIndex::Ref(i);
            self.position_of(entry).map(|p| (entry, p))
        })
    }

    /// Apply several edge renames SIMULTANEOUSLY: every stated carry edge is matched against
    /// the pre-rename names once, so shifting slots in a renumber can't collide mid-flight.
    /// Each renamed edge is `(old, new)` — a slot renumbered or re-sourced onto another pick.
    pub(crate) fn rename_edges(&mut self, renames: &[(Edge, Edge)]) {
        for groups in self.arrangement.values_mut() {
            for group in groups.iter_mut() {
                let Carry::Edges(edges) = &mut group.carry else {
                    continue;
                };
                let mut changed = false;
                for edge in edges.iter_mut() {
                    if let Some((_, new)) = renames.iter().find(|(old, _)| old == edge) {
                        *edge = *new;
                        changed = true;
                    }
                }
                if changed {
                    edges.sort_unstable();
                    edges.dedup();
                }
            }
        }
    }

    // --- The parent arrays ---
    //
    // An entry's parents ARE an ordered array: the slot is the parent order, dense by
    // construction. Statement names `(child, slot)` are live coordinates — mutators shift
    // and rename statements along with the slots they move. A removed slot's statements are
    // dropped — revival is an explicit op-level re-statement, never a store-level
    // coincidence.

    /// The ordered parents of `entry` — slot position is the parent order. References are
    /// edgeless by construction.
    pub(crate) fn parents(&self, entry: EditorGraphIndex) -> Vec<EditorGraphIndex> {
        match entry {
            EditorGraphIndex::Row(i) => self
                .arena
                .parent_indices(i)
                .into_iter()
                .map(EditorGraphIndex::Row)
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
        let EditorGraphIndex::Row(i) = child else {
            panic!("references are edgeless — no parent array");
        };
        let mut parents: Vec<EditorGraphIndex> = self
            .arena
            .parent_indices(i)
            .into_iter()
            .map(EditorGraphIndex::Row)
            .collect();
        let result = f(&mut parents);
        let targets = parents
            .into_iter()
            .map(|parent| match parent {
                EditorGraphIndex::Row(j) => j,
                EditorGraphIndex::Ref(_) => {
                    panic!("references are edgeless — they cannot be parents")
                }
            })
            .collect();
        self.arena.set_parents(i, targets);
        result
    }

    /// How many parent slots `entry` has.
    pub(crate) fn parent_count(&self, entry: EditorGraphIndex) -> usize {
        self.parents(entry).len()
    }

    /// Every parent-array slot naming `entry`, as `(child, slot)` edges, sorted — the derived
    /// children read.
    pub(crate) fn incoming_edges(&self, entry: EditorGraphIndex) -> Vec<Edge> {
        let mut edges = Vec::new();
        for i in 0..self.arena.row_count() {
            let child = EditorGraphIndex::Row(i);
            for (slot, parent) in self.arena.parent_indices(i).into_iter().enumerate() {
                if EditorGraphIndex::Row(parent) == entry {
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
    /// statement name — is untouched: groups stated on the edge follow it to its new target.
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
        for i in 0..self.arena.row_count() {
            let child = EditorGraphIndex::Row(i);
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

    /// Empty `child`'s parent array, returning it. Group statements naming the drained slots
    /// are DELIBERATELY untouched: the caller re-states the orphaned names onto their new
    /// carrier itself (the below-insert path renames them onto the segment's parent-most).
    pub(crate) fn drain_parents(&mut self, child: EditorGraphIndex) -> Vec<EditorGraphIndex> {
        self.update_parents(child, std::mem::take)
    }

    /// Drop every stated carry edge `keep` rejects.
    fn retain_edges(&mut self, keep: impl Fn(&Edge) -> bool) {
        for groups in self.arrangement.values_mut() {
            for group in groups.iter_mut() {
                if let Carry::Edges(edges) = &mut group.carry {
                    edges.retain(&keep);
                }
            }
        }
    }

    /// All row-arena ids (picks and tombstones), ascending. References are NOT included —
    /// see [`Self::references`] and [`Self::ref_indices`].
    pub(crate) fn row_ids(&self) -> impl Iterator<Item = EditorGraphIndex> + '_ {
        (0..self.arena.row_count()).map(EditorGraphIndex::Row)
    }

    /// The arena rows no parent array names — the child-less tips, ascending. References
    /// never appear here: they are edgeless by construction, and the consumers
    /// (head discovery) want picks and tombstones only.
    pub(crate) fn tips(&self) -> impl Iterator<Item = EditorGraphIndex> + '_ {
        let referenced: HashSet<EditorGraphIndex> = (0..self.arena.row_count())
            .flat_map(|i| self.arena.parent_indices(i))
            .map(EditorGraphIndex::Row)
            .collect();
        (0..self.arena.row_count())
            .map(EditorGraphIndex::Row)
            .filter(move |entry| !referenced.contains(entry))
    }
}
