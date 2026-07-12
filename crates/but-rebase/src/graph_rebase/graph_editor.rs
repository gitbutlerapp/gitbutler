//! An owned arena graph for rebase steps. Each arena node holds a commit id (`None` =
//! tombstone) with its pick options in a parallel settings table, and an ordered parent
//! array — a parent's position in the array is its parent order, with no gaps. References
//! live in the ref table and hold positions. Children are derived from the parent arrays
//! (a maintained reverse index, never independent truth). Nothing is ever removed (a
//! removed pick becomes a `None` payload, a removed reference goes dead in place), so ids
//! stay stable. [`Step`] and [`Pick`] exist only at the API edge: [`GraphEditor::step_view`]
//! builds them, [`GraphEditor::add_node`]/[`GraphEditor::set_step`] take them apart.

use std::collections::{HashMap, HashSet};

use but_core::commit::SignCommit;

use crate::graph_rebase::{
    Pick, Step,
    cherry_pick::{PickMode, TreeMergeMode},
};

/// The stable identifier of an editor-graph entry. Two namespaces, one id type: `Pick` points
/// into the pick arena (its parent array is its truth), `Ref` into the reference table (a
/// position is its truth) — so a selector can address either without knowing which.
///
/// This is the ADDRESSING type. Namespace-specific operations take [`PickIndex`] or
/// [`RefIndex`] instead, so "a reference in a parent array" is unrepresentable rather than
/// a runtime check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EditorIndex {
    /// A pick or its tombstone in the pick arena.
    Pick(usize),
    /// A reference (live or dead) in the ref table.
    Ref(usize),
}

impl EditorIndex {
    /// The pick-arena handle, when this addresses a pick or tombstone.
    pub(crate) fn as_pick(self) -> Option<PickIndex> {
        match self {
            EditorIndex::Pick(i) => Some(PickIndex(i)),
            EditorIndex::Ref(_) => None,
        }
    }

    /// The ref-table handle, when this addresses a reference.
    pub(crate) fn as_ref(self) -> Option<RefIndex> {
        match self {
            EditorIndex::Ref(i) => Some(RefIndex(i)),
            EditorIndex::Pick(_) => None,
        }
    }
}

impl std::fmt::Display for EditorIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorIndex::Pick(i) => write!(f, "p{i}"),
            EditorIndex::Ref(i) => write!(f, "r{i}"),
        }
    }
}

/// A node in the pick arena — a pick or its tombstone. Nodes are the ONLY entities that
/// carry edges: parent arrays connect nodes, never references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct PickIndex(pub(crate) usize);

impl From<PickIndex> for EditorIndex {
    fn from(n: PickIndex) -> Self {
        EditorIndex::Pick(n.0)
    }
}

impl std::fmt::Display for PickIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "p{}", self.0)
    }
}

/// An entry in the reference table, live or dead. References are edgeless — a position is
/// their truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RefIndex(pub(crate) usize);

impl From<RefIndex> for EditorIndex {
    fn from(r: RefIndex) -> Self {
        EditorIndex::Ref(r.0)
    }
}

impl std::fmt::Display for RefIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{}", self.0)
    }
}

/// One incoming child edge of a pick, named POSITIONALLY as `(source node, parent number)`.
/// GroupCarry statements name edges this way, so an edge removed and re-created at the same coordinates
/// is the SAME statement — see [`GroupCarry::Edges`].
pub(crate) type Edge = (PickIndex, usize);

/// One reference's editor state: name, mutability, liveness, convergence flag. Deletion
/// flips `live` but keeps the name and position — that matters: selectors taken before the
/// deletion still resolve through the dead record, and rebuilds copy it forward. Where a
/// reference SITS is not
/// state but list structure in the layout table, read via [`GraphEditor::positioned_on`],
/// [`GraphEditor::below_of`] and the derived queries in `positions` (`ref_depth` for rank,
/// `edges_through` for entering edges, `resolve_to_pick` for the pick through tombstones).
#[derive(Debug, Clone)]
pub(crate) struct RefState {
    /// The full reference name.
    pub refname: gix::refs::FullName,
    /// Whether the rebase may move this reference.
    pub mutable: bool,
    /// `false` once the reference is deleted; the record stays.
    pub live: bool,
    /// More than one thing (edges and/or stacked refs) converged here — a merge. A
    /// creation-time signal distinct from the entering-edge count (a position can converge
    /// yet resolve to a single edge), so it is preserved here, never re-derived.
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

/// The editor's carry: [`but_graph::ref_layout::GroupCarry`] over pick handles — the same
/// shape the display side stores, with commit ids mapped to pick handles at creation.
/// Listed edges are always read against the pick's live edges, so an entry whose edge no
/// longer exists is harmless — and comes back on its own if a later mutation re-creates
/// the edge at the same `(child, parent number)`.
pub(crate) type GroupCarry = but_graph::ref_layout::GroupCarry<PickIndex>;

/// The editor's reference group: [`but_graph::ref_layout::RefGroup`] over pick handles —
/// an ordered bottom→top run of member NAMES sharing one [`GroupCarry`]. Order and stacking
/// are implied by the list: the member below is the previous entry, and a member's rank is
/// its index plus the height of whatever the group is attached to (`positions::ref_depth`).
pub(crate) type RefGroup = but_graph::ref_layout::RefGroup<PickIndex>;

/// The editor's graph: a [`but_graph::CommitGraph`] arena where PICKS carry ordered
/// parent arrays, plus a table of [`RefState`]s where REFERENCES carry explicit positions —
/// the arena is the truth for commits, positions the truth for refs, with no overlap.
/// References are edgeless: creation authors their positions straight from the stored
/// [`RefLayout`](but_graph::ref_layout::RefLayout).
#[derive(Debug, Clone, Default)]
pub(crate) struct GraphEditor {
    /// THE arena: `EditorIndex::Pick(i)` IS the commit-graph node index `i`. Commit ids are the payload
    /// (tombstoning flags a node in place, the node id survives every rewrite); the parent
    /// arrays are the ordered structure.
    arena: but_graph::CommitGraph,
    /// Each node's pick options, parallel to the arena.
    settings: Vec<PickSettings>,
    refs: Vec<RefState>,
    /// Each record's index by CURRENT name — names are unique across live and dead records
    /// (re-creating a deleted name resurrects its record), which is why the layout table
    /// can identify members by name alone.
    by_name: HashMap<gix::refs::FullName, RefIndex>,
    /// THE position store — the editor-space counterpart of the stored
    /// [`RefLayout`](but_graph::ref_layout::RefLayout) it is ingested from: per STORED
    /// (unresolved) key, the reference groups standing on it. A reference's position (its
    /// `on`, its below, its rank, its entering edges) is all list structure here, read via
    /// [`Self::position_of`] and `positions::edges_through`.
    layout: HashMap<PickIndex, Vec<RefGroup>>,
    /// The derived children index, parallel to the arena: `children[p]` holds every
    /// `(child, parent number)` edge naming node `p`, sorted. Maintained through
    /// [`Self::update_parents`] — the single seam every parent mutation flows through —
    /// so [`Self::incoming_edges`] is a lookup, not an arena scan.
    children: Vec<Vec<Edge>>,
}

impl GraphEditor {
    /// Adopt `arena` as the editor's arena. Full commit payloads (flags, refs, generation)
    /// survive, which handing the arena back out after a rebase depends on. Every parent
    /// edge must already point at a node in the graph — the editor's standing requirement —
    /// and the caller follows up with per-node settings via [`Self::set_step`].
    pub(crate) fn adopt(arena: but_graph::CommitGraph) -> Self {
        let settings = vec![PickSettings::default(); arena.node_count()];
        let mut children: Vec<Vec<Edge>> = vec![Vec::new(); arena.node_count()];
        for i in 0..arena.node_count() {
            for (parent_number, parent) in arena.parent_indices(i).into_iter().enumerate() {
                children[parent].push((PickIndex(i), parent_number));
            }
        }
        Self {
            arena,
            settings,
            refs: Vec::new(),
            by_name: HashMap::new(),
            layout: HashMap::new(),
            children,
        }
    }

    /// THE arena, read-only — the write-through seam projects it after a rebase.
    pub(crate) fn arena(&self) -> &but_graph::CommitGraph {
        &self.arena
    }

    /// Add `step` to the node arena and return its stable id. References do not belong here —
    /// use [`Self::add_reference`].
    pub(crate) fn add_node(&mut self, pick: Option<Pick>) -> PickIndex {
        let (id, settings) = match pick {
            Some(pick) => {
                let (id, settings) = PickSettings::split(pick);
                (Some(id), settings)
            }
            None => (None, PickSettings::default()),
        };
        let i = self.arena.add_node(id);
        self.settings.push(settings);
        self.children.push(Vec::new());
        debug_assert_eq!(
            self.settings.len(),
            self.arena.node_count(),
            "settings table fell out of step with the arena"
        );
        PickIndex(i)
    }

    /// Replace the node payload at `entry` — a pick decomposes into id and settings,
    /// `None` tombstones the payload (settings go stale, not cleared).
    pub(crate) fn set_step(&mut self, entry: PickIndex, pick: Option<Pick>) {
        match pick {
            Some(pick) => {
                let (id, settings) = PickSettings::split(pick);
                self.arena.set_node_id(entry.0, Some(id));
                self.settings[entry.0] = settings;
            }
            None => self.arena.set_node_id(entry.0, None),
        }
    }

    /// The step at `entry` as an owned view, synthesized from the payload: id plus settings
    /// make a `Step::Pick`, a `None` id (or a dead reference) a `Step::None`.
    pub(crate) fn step_view(&self, entry: EditorIndex) -> Step {
        match entry {
            EditorIndex::Pick(i) => match self.arena.node_payload(i) {
                Some(id) => Step::Pick(self.settings[i].pick(id)),
                None => Step::None,
            },
            EditorIndex::Ref(i) => {
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

    /// The commit id of the pick at `entry` — `None` for tombstones and references. The
    /// cheap way to read just the id; use [`Self::step_view`] for the whole step.
    pub(crate) fn commit_id(&self, entry: impl Into<EditorIndex>) -> Option<gix::ObjectId> {
        let entry = entry.into();
        match entry {
            EditorIndex::Pick(i) => self.arena.node_payload(i),
            EditorIndex::Ref(_) => None,
        }
    }

    /// Rewrite the commit id of the pick at `entry` IN PLACE — THE rebase write: the node id,
    /// its parent array, its settings, and every position naming it all survive unchanged.
    pub(crate) fn set_commit_id(&mut self, entry: PickIndex, id: gix::ObjectId) {
        debug_assert!(
            self.arena.node_payload(entry.0).is_some(),
            "tombstones have no commit id"
        );
        self.arena.set_commit_id(entry.0, id);
    }

    /// Overwrite the preserved parents of the pick at `entry` (see
    /// [`Pick::preserved_parents`]).
    pub(crate) fn set_preserved_parents(
        &mut self,
        entry: PickIndex,
        parents: Option<Vec<gix::ObjectId>>,
    ) {
        debug_assert!(
            self.arena.node_payload(entry.0).is_some(),
            "tombstones carry no pick options"
        );
        self.settings[entry.0].preserved_parents = parents;
    }

    /// `true` iff `entry` is a pick — `false` for tombstones and references.
    pub(crate) fn is_pick(&self, entry: impl Into<EditorIndex>) -> bool {
        self.commit_id(entry).is_some()
    }

    /// All node-arena ids (picks and tombstones), ascending — the type says
    /// references are not here; see [`Self::references`] and [`Self::ref_indices`].
    pub(crate) fn node_ids(&self) -> impl Iterator<Item = PickIndex> + '_ {
        (0..self.arena.node_count()).map(PickIndex)
    }

    /// The nodes that no other node lists as a parent — the childless tips, ascending.
    /// Callers (head discovery) want picks and tombstones only; references can't appear
    /// here by type.
    pub(crate) fn tips(&self) -> impl Iterator<Item = PickIndex> + '_ {
        (0..self.arena.node_count())
            .filter(|&i| self.children[i].is_empty())
            .map(PickIndex)
    }

    /// Add a reference and return its stable id. Names are identity: adding a name that
    /// already has a record — a re-created deleted ref, say — RESURRECTS that record in
    /// place (its retained position stands until the caller re-places it).
    pub(crate) fn add_reference(
        &mut self,
        refname: gix::refs::FullName,
        mutable: bool,
    ) -> RefIndex {
        if let Some(&existing) = self.by_name.get(&refname) {
            let record = &mut self.refs[existing.0];
            record.mutable = mutable;
            record.live = true;
            return existing;
        }
        let entry = RefIndex(self.refs.len());
        self.by_name.insert(refname.clone(), entry);
        self.refs.push(RefState {
            refname,
            mutable,
            live: true,
            ambiguous: false,
        });
        entry
    }

    /// The record registered under `name`, live or dead.
    pub(crate) fn entry_of(&self, name: &gix::refs::FullNameRef) -> Option<RefIndex> {
        self.by_name.get(name).copied()
    }

    /// The CURRENT name of the record at `entry`, live or dead.
    fn name_of(&self, entry: RefIndex) -> &gix::refs::FullName {
        &self.refs[entry.0].refname
    }

    /// The reference payload at `entry` — `Some` iff it names a live (non-deleted) reference.
    pub(crate) fn reference(&self, entry: EditorIndex) -> Option<(&gix::refs::FullName, bool)> {
        let EditorIndex::Ref(i) = entry else {
            return None;
        };
        let record = self.refs.get(i)?;
        record.live.then_some((&record.refname, record.mutable))
    }

    /// `true` iff `entry` is a live reference.
    pub(crate) fn is_reference(&self, entry: impl Into<EditorIndex>) -> bool {
        self.reference(entry.into()).is_some()
    }

    /// All live references, ascending by id.
    pub(crate) fn references(
        &self,
    ) -> impl Iterator<Item = (RefIndex, &gix::refs::FullName, bool)> + '_ {
        self.refs.iter().enumerate().filter_map(|(i, record)| {
            record
                .live
                .then_some((RefIndex(i), &record.refname, record.mutable))
        })
    }

    /// All reference ids — live AND dead — ascending. Dead references still carry their
    /// retained name and position (see [`RefState`]).
    pub(crate) fn ref_indices(&self) -> impl Iterator<Item = RefIndex> + '_ {
        (0..self.refs.len()).map(RefIndex)
    }

    /// The full record of the reference at `entry`, including dead ones — rebuilds need the
    /// retained payload.
    pub(crate) fn state_of(&self, entry: EditorIndex) -> Option<&RefState> {
        match entry {
            EditorIndex::Ref(i) => self.refs.get(i),
            EditorIndex::Pick(_) => None,
        }
    }

    /// Rename (or resurrect) the reference at `entry` in place; its position is untouched —
    /// the layout table's member entry renames with it.
    pub(crate) fn set_reference(
        &mut self,
        entry: RefIndex,
        refname: gix::refs::FullName,
        mutable: bool,
    ) {
        let old_name = self.refs[entry.0].refname.clone();
        if old_name != refname {
            debug_assert!(
                !self.by_name.contains_key(&refname),
                "BUG: renaming {old_name} onto a name that already has a record: {refname}"
            );
            self.by_name.remove(&old_name);
            self.by_name.insert(refname.clone(), entry);
            for groups in self.layout.values_mut() {
                for group in groups.iter_mut() {
                    for member in group.members.iter_mut() {
                        if *member == old_name {
                            *member = refname.clone();
                        }
                    }
                    if group.attach.as_ref() == Some(&old_name) {
                        group.attach = Some(refname.clone());
                    }
                }
            }
        }
        let record = &mut self.refs[entry.0];
        record.refname = refname;
        record.mutable = mutable;
        record.live = true;
    }

    /// Delete the reference at `entry`: it goes dead in place, retaining name and position
    /// (see [`RefState`]).
    pub(crate) fn tombstone_reference(&mut self, entry: RefIndex) {
        self.refs[entry.0].live = false;
    }

    /// The stored key the reference at `entry` stands on (a pick, or its tombstone after
    /// deletion), live or dead — `None` until placed.
    pub(crate) fn positioned_on(&self, entry: impl Into<EditorIndex>) -> Option<PickIndex> {
        let entry = entry.into().as_ref()?;
        self.locate(entry).map(|(key, ..)| key)
    }

    /// The reference directly underneath `entry` in the physical stack — `None` when it
    /// sits directly on its pick (or holds no position at all; see [`Self::is_positioned`]).
    pub(crate) fn below_of(&self, entry: impl Into<EditorIndex>) -> Option<RefIndex> {
        let entry = entry.into().as_ref()?;
        let (key, g, i) = self.locate(entry)?;
        let group = &self.layout[&key][g];
        let below = if i > 0 {
            Some(&group.members[i - 1])
        } else {
            group.attach.as_ref()
        };
        below.and_then(|name| self.by_name.get(name.as_ref()).copied())
    }

    /// Whether the reference at `entry` holds a position — only unborn refs don't.
    pub(crate) fn is_positioned(&self, entry: impl Into<EditorIndex>) -> bool {
        entry
            .into()
            .as_ref()
            .is_some_and(|entry| self.locate(entry).is_some())
    }

    /// The preserved convergence flag of the reference at `entry` (see [`RefState`]).
    pub(crate) fn ambiguous_of(&self, entry: RefIndex) -> bool {
        self.refs[entry.0].ambiguous
    }

    /// Author a position for `entry`: `entering` is the carry intent — the edges meant to
    /// enter through it — classified against `on`'s CURRENT edges (see [`Self::classify`]).
    /// Members stacked on the entry ride along; only correct when the entry's edges are
    /// already complete.
    pub(crate) fn set_position(
        &mut self,
        entry: RefIndex,
        on: PickIndex,
        entering: &[Edge],
        ambiguous: bool,
        below: Option<RefIndex>,
    ) {
        let carry = self.classify(on, entering);
        self.extract(entry);
        self.place(entry, on, carry, below);
        self.set_ambiguous(entry, ambiguous);
    }

    /// Splice `entry` out of the physical stack, dependents HEALING past it: members above it
    /// in its group close the gap, and groups attached to it re-hang onto what it sat on.
    /// The entry itself stays placed as a single-member BRANCH group at the same spot (same
    /// key, attached to its old below, carry copied) — the retained position a deletion
    /// leaves behind.
    pub(crate) fn splice(&mut self, entry: RefIndex) {
        let Some((key, g, i)) = self.locate(entry) else {
            return;
        };
        let name = self.name_of(entry).clone();
        let groups = self.layout.get_mut(&key).expect("just located");
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
        for groups in self.layout.values_mut() {
            for group in groups.iter_mut() {
                if group.attach.as_ref() == Some(&name) {
                    group.attach = below.clone();
                }
            }
        }
        self.layout.entry(key).or_default().push(RefGroup {
            members: vec![name],
            carry,
            attach: below,
        });
        self.coalesce_groups(key);
    }

    /// Join `entry` into the group of `mate` — copying the mate's key, carry, and ambiguity —
    /// sitting on `below`.
    pub(crate) fn join_group_of(
        &mut self,
        entry: RefIndex,
        mate: RefIndex,
        below: Option<RefIndex>,
    ) {
        let Some((key, g, _)) = self.locate(mate) else {
            return;
        };
        let carry = self.layout[&key][g].carry.clone();
        let ambiguous = self.refs[mate.0].ambiguous;
        self.extract(entry);
        self.place(entry, key, carry, below);
        self.set_ambiguous(entry, ambiguous);
    }

    /// Re-key `entry`'s position onto `onto`, carrying its CURRENT carry — as maintained
    /// through edge surgery. Below and ambiguity are preserved; members stacked on the entry
    /// ride along.
    pub(crate) fn rekey_position(&mut self, entry: RefIndex, onto: PickIndex) {
        let Some(on) = self.positioned_on(entry) else {
            return;
        };
        if on == onto {
            return;
        }
        let below = self.below_of(entry);
        let carry = self.carry_of(entry).cloned().unwrap_or(GroupCarry::All);
        self.extract(entry);
        self.place(entry, onto, carry, below);
    }

    /// Re-hang `entry` onto `below` — an adjacency statement only; its key, carry, and
    /// ambiguity are untouched, and members stacked on the entry ride along.
    pub(crate) fn set_below(&mut self, entry: RefIndex, below: Option<RefIndex>) {
        let Some(on) = self.positioned_on(entry) else {
            return;
        };
        if self.below_of(entry) == below {
            return;
        }
        let carry = self.carry_of(entry).cloned().unwrap_or(GroupCarry::All);
        self.extract(entry);
        self.place(entry, on, carry, below);
    }

    /// Point a dead reference's kept position at `on` — the bare pointer that stale
    /// selectors resolve through. Its old spot heals (dependents re-hang past it),
    /// and it lands as a bare root; live references re-place via the layout machinery
    /// instead.
    pub(crate) fn set_retained_position(&mut self, entry: RefIndex, on: PickIndex) {
        debug_assert!(
            !self.is_reference(entry),
            "retained positions belong to dead references"
        );
        self.splice(entry);
        self.extract(entry);
        self.place(entry, on, GroupCarry::None, None);
        self.set_ambiguous(entry, false);
    }

    /// The carry of the group holding the reference at `entry`, if it holds a position.
    pub(crate) fn carry_of(&self, entry: impl Into<EditorIndex>) -> Option<&GroupCarry> {
        let entry = entry.into().as_ref()?;
        let (key, g, _) = self.locate(entry)?;
        Some(&self.layout[&key][g].carry)
    }

    /// All positioned references — live AND dead — ascending by id.
    pub(crate) fn positioned_refs(&self) -> impl Iterator<Item = RefIndex> + '_ {
        (0..self.refs.len())
            .map(RefIndex)
            .filter(|&entry| self.locate(entry).is_some())
    }

    /// Where `entry` sits in the layout table: `(key, group index, member index)`.
    /// Membership is by NAME — the record's current name is the table identity.
    fn locate(&self, entry: RefIndex) -> Option<(PickIndex, usize, usize)> {
        let name = self.name_of(entry);
        self.layout.iter().find_map(|(&key, groups)| {
            groups.iter().enumerate().find_map(|(g, group)| {
                group
                    .members
                    .iter()
                    .position(|m| m == name)
                    .map(|i| (key, g, i))
            })
        })
    }

    /// Classify a position's entering-edge intent against `on`'s CURRENT edges: empty is a
    /// root, the whole live set the shared `All` carry, any other set an `Edges` carry
    /// stating exactly those edges.
    fn classify(&self, on: PickIndex, entering: &[Edge]) -> GroupCarry {
        if entering.is_empty() {
            return GroupCarry::None;
        }
        let live = match crate::graph_rebase::positions::resolve_to_pick(self, on) {
            Some(pick) => crate::graph_rebase::positions::edges_into(self, pick),
            None => Vec::new(),
        };
        let edge_set: HashSet<_> = entering.iter().copied().collect();
        let live_set: HashSet<_> = live.iter().copied().collect();
        if edge_set == live_set {
            GroupCarry::All
        } else {
            let mut edges = entering.to_vec();
            edges.sort_unstable();
            edges.dedup();
            GroupCarry::Edges(edges)
        }
    }

    /// Take `entry` out of the table, its dependents RIDING: members stacked above it in its
    /// group split off as their own group attached to `entry` (they keep sitting on it,
    /// wherever it goes next), and groups attached to `entry` stay attached. The caller
    /// re-places the entry immediately.
    fn extract(&mut self, entry: RefIndex) {
        let Some((key, g, i)) = self.locate(entry) else {
            return;
        };
        let name = self.name_of(entry).clone();
        let groups = self.layout.get_mut(&key).expect("just located");
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
            self.layout.remove(&key);
        }
    }

    /// Put the (extracted or fresh) `entry` into the table at `key` via the shared
    /// [`but_graph::ref_layout::place_in_groups`] — the same algorithm the builder authors with.
    fn place(
        &mut self,
        entry: RefIndex,
        key: PickIndex,
        carry: GroupCarry,
        attach: Option<RefIndex>,
    ) {
        let name = self.name_of(entry).clone();
        let attach = attach.map(|b| self.name_of(b).clone());
        but_graph::ref_layout::place_in_groups(
            self.layout.entry(key).or_default(),
            name,
            carry,
            attach,
        );
    }

    fn coalesce_groups(&mut self, key: PickIndex) {
        if let Some(groups) = self.layout.get_mut(&key) {
            but_graph::ref_layout::coalesce_groups(groups);
        }
    }

    fn set_ambiguous(&mut self, entry: RefIndex, ambiguous: bool) {
        self.refs[entry.0].ambiguous = ambiguous;
    }

    /// Overwrite whether the rebase may move the reference at `entry`.
    pub(crate) fn set_ref_mutable(&mut self, entry: RefIndex, mutable: bool) {
        self.refs[entry.0].mutable = mutable;
    }

    /// The references carrying the edge `(child, parent number)` into `parent`: each carrying
    /// group answers with its TOP member — the below-walk covers the rest.
    pub(crate) fn edge_carriers(
        &self,
        parent: PickIndex,
        child: PickIndex,
        parent_number: usize,
    ) -> impl Iterator<Item = RefIndex> + '_ {
        self.layout
            .get(&parent)
            .into_iter()
            .flatten()
            .filter(move |group| match &group.carry {
                GroupCarry::None => false,
                GroupCarry::All => true,
                GroupCarry::Edges(edges) => edges.contains(&(child, parent_number)),
            })
            .filter_map(|group| group.members.last())
            .filter_map(|name| self.by_name.get(name).copied())
    }

    pub(crate) fn set_ref_ambiguous(&mut self, entry: RefIndex, ambiguous: bool) {
        self.set_ambiguous(entry, ambiguous);
    }

    /// Adopt `groups` wholesale as `key`'s reference groups — creation's id-mapped copy of
    /// the stored layout. Every member name must already be registered.
    pub(crate) fn insert_groups(&mut self, key: PickIndex, groups: Vec<RefGroup>) {
        debug_assert!(
            groups
                .iter()
                .flat_map(|g| g.members.iter())
                .all(|name| self.by_name.contains_key(name.as_ref())),
            "BUG: ingest must register every reference before copying groups"
        );
        self.layout.entry(key).or_default().extend(groups);
    }

    /// The ordered parents of `entry` — parent number position is the parent order; references
    /// have none.
    pub(crate) fn parents(&self, entry: impl Into<EditorIndex>) -> Vec<PickIndex> {
        match entry.into() {
            EditorIndex::Pick(i) => self
                .arena
                .parent_indices(i)
                .into_iter()
                .map(PickIndex)
                .collect(),
            EditorIndex::Ref(_) => Vec::new(),
        }
    }

    /// How many parents `entry` has.
    pub(crate) fn parent_count(&self, entry: impl Into<EditorIndex>) -> usize {
        self.parents(entry).len()
    }

    /// Every edge that names `entry` as a parent, as sorted `(child, parent number)`
    /// pairs — answered from the maintained children index, not an arena scan.
    pub(crate) fn incoming_edges(&self, entry: impl Into<EditorIndex>) -> &[Edge] {
        let entry = entry.into();
        match entry {
            EditorIndex::Pick(i) => &self.children[i],
            EditorIndex::Ref(_) => &[],
        }
    }

    /// Append `parent` as `child`'s last parent; returns its parent number.
    pub(crate) fn push_parent(&mut self, child: PickIndex, parent: PickIndex) -> usize {
        self.update_parents(child, |parents| {
            parents.push(parent);
            parents.len() - 1
        })
    }

    /// Insert `parent` at `parent number` of `child` (clamped to the array end); later parent numbers shift up
    /// with their statements. Returns the parent number actually used.
    pub(crate) fn insert_parent(
        &mut self,
        child: PickIndex,
        parent_number: usize,
        parent: PickIndex,
    ) -> usize {
        let len = self.parent_count(child);
        let parent_number = parent_number.min(len);
        let renames: Vec<_> = (parent_number..len)
            .map(|s| ((child, s), (child, s + 1)))
            .collect();
        self.rename_edges(&renames);
        self.update_parents(child, |parents| parents.insert(parent_number, parent));
        parent_number
    }

    /// Remove `child`'s parent at `parent number`, returning it; later parent numbers shift down with their
    /// statements, and statements naming the removed parent number are dropped.
    pub(crate) fn remove_parent(
        &mut self,
        child: PickIndex,
        parent_number: usize,
    ) -> Option<PickIndex> {
        let len = self.parent_count(child);
        if parent_number >= len {
            return None;
        }
        let target = self.update_parents(child, |parents| parents.remove(parent_number));
        self.retain_edges(|&edge| edge != (child, parent_number));
        let renames: Vec<_> = (parent_number + 1..len)
            .map(|s| ((child, s), (child, s - 1)))
            .collect();
        self.rename_edges(&renames);
        Some(target)
    }

    /// Re-point `child`'s parent at `parent number` onto `new_parent`. The parent number — and so the
    /// statement name — is untouched: groups stated on the edge follow it to its new target.
    pub(crate) fn replace_parent(
        &mut self,
        child: PickIndex,
        parent_number: usize,
        new_parent: PickIndex,
    ) {
        self.update_parents(child, |parents| match parents.get_mut(parent_number) {
            Some(entry) => *entry = new_parent,
            None => debug_assert!(
                false,
                "replace_parent: {child} has no parent_number {parent_number}"
            ),
        });
    }

    /// Move `from`'s whole parent array onto `to` (which must have none); statements follow
    /// parent number-for-parent number.
    pub(crate) fn transplant_parents(&mut self, from: PickIndex, to: PickIndex) {
        debug_assert_eq!(
            self.parent_count(to),
            0,
            "transplant target {to} already has parents"
        );
        let parents = self.update_parents(from, std::mem::take);
        let renames: Vec<_> = (0..parents.len()).map(|s| ((from, s), (to, s))).collect();
        self.update_parents(to, |parent_number| *parent_number = parents);
        self.rename_edges(&renames);
    }

    /// Re-target every parent-array entry naming `from` onto `to`, parent numbers preserved —
    /// statement names are `(source, parent number)`, so they stay valid untouched.
    pub(crate) fn redirect_children(&mut self, from: PickIndex, to: PickIndex) {
        // The index is sorted by child, so consecutive edges of one child dedup away.
        let mut children: Vec<PickIndex> = self
            .incoming_edges(from)
            .iter()
            .map(|&(child, _)| child)
            .collect();
        children.dedup();
        for child in children {
            self.update_parents(child, |parents| {
                for parent in parents.iter_mut() {
                    if *parent == from {
                        *parent = to;
                    }
                }
            });
        }
    }

    /// Empty `child`'s parent array, returning it. Group statements naming the drained parent numbers
    /// are DELIBERATELY untouched: the caller re-states the orphaned names onto their new
    /// carrier itself (the below-insert path renames them onto the range's parent-most).
    pub(crate) fn drain_parents(&mut self, child: PickIndex) -> Vec<PickIndex> {
        self.update_parents(child, std::mem::take)
    }

    /// Apply several edge renames SIMULTANEOUSLY: every stated carry edge is matched against
    /// the pre-rename names once, so shifting parent numbers in a renumber can't collide mid-flight.
    /// Each renamed edge is `(old, new)` — a parent number renumbered or re-sourced onto another pick.
    pub(crate) fn rename_edges(&mut self, renames: &[(Edge, Edge)]) {
        for groups in self.layout.values_mut() {
            for group in groups.iter_mut() {
                let GroupCarry::Edges(edges) = &mut group.carry else {
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
    // Carry entries name edges as `(child, parent number)`, and mutators keep those names
    // current: shifting a parent number renames the entries with it. Removing a parent drops
    // its entries for good — an operation that wants them back must state them again; the
    // store never resurrects them by accident.

    /// Drop every stated carry edge `keep` rejects.
    fn retain_edges(&mut self, keep: impl Fn(&Edge) -> bool) {
        for groups in self.layout.values_mut() {
            for group in groups.iter_mut() {
                if let GroupCarry::Edges(edges) = &mut group.carry {
                    edges.retain(&keep);
                }
            }
        }
    }

    /// Rewrite `child`'s parent array through `f` — the single seam every parent mutation
    /// flows through into the arena's parent number write. Parents are [`PickIndex`] by type:
    /// references in an edge are unrepresentable.
    fn update_parents<R>(
        &mut self,
        child: PickIndex,
        f: impl FnOnce(&mut Vec<PickIndex>) -> R,
    ) -> R {
        let old = self.arena.parent_indices(child.0);
        let mut parents: Vec<PickIndex> = old.iter().copied().map(PickIndex).collect();
        let result = f(&mut parents);
        let targets: Vec<usize> = parents.into_iter().map(|parent| parent.0).collect();
        for (parent_number, &parent) in old.iter().enumerate() {
            let edges = &mut self.children[parent];
            if let Ok(at) = edges.binary_search(&(child, parent_number)) {
                edges.remove(at);
            }
        }
        for (parent_number, &parent) in targets.iter().enumerate() {
            let edges = &mut self.children[parent];
            match edges.binary_search(&(child, parent_number)) {
                Err(at) => edges.insert(at, (child, parent_number)),
                Ok(_) => debug_assert!(
                    false,
                    "children index already names ({child}, {parent_number})"
                ),
            }
        }
        // Preserved parents pin the pick's onto-commits only while its edges are
        // untouched (they carry raw parents the walk didn't materialize). Once a
        // mutation rewrites the parent array, the live edges are the truth — a stale
        // preserved list would make the rebase silently ignore the reparenting.
        if targets.as_slice() != old && self.settings[child.0].preserved_parents.is_some() {
            self.settings[child.0].preserved_parents = None;
        }
        self.arena.set_parents(child.0, targets);
        result
    }
}
