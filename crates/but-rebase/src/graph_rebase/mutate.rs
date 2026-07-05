//! Operations for mutating the editor

use std::collections::{HashMap, HashSet};

use crate::graph_rebase::arrangement::{
    SplitBoundary, StackSlot, carry_stack_above, land_stack_above, move_ref, place_ref,
    readopt_dangling_refs, repoint_ref, settle_chain_lower, splice_out, split_chain,
    transfer_stack, unhook_ref,
};
use crate::graph_rebase::{StepGraph, StepGraphIndex, positions};
use anyhow::{Context as _, Result, anyhow, bail};
use but_core::RefMetadata;
use serde::{Deserialize, Serialize};

use crate::graph_rebase::{
    Editor, Selector, Step, ToCommitSelector, ToReferenceSelector, ToSelector,
};

/// Parent-slot names captured at one instant (a frame), resolved against a store that has
/// since shifted. `current` maps a frame name to today's slot; removals and inserts are noted
/// so later lookups stay aligned. `None` means the named leg itself was already removed.
#[derive(Default)]
struct SlotLedger {
    map: HashMap<StepGraphIndex, Vec<Option<usize>>>,
}

impl SlotLedger {
    /// Today's slot of the leg captured as `(source, frame_slot)`, or `None` if it was
    /// removed. The identity map is built lazily, so a source must be looked up before any
    /// of its slots mutate.
    fn current(
        &mut self,
        graph: &StepGraph,
        source: StepGraphIndex,
        frame_slot: usize,
    ) -> Option<usize> {
        self.map
            .entry(source)
            .or_insert_with(|| (0..graph.parent_count(source)).map(Some).collect())
            .get(frame_slot)
            .copied()
            .flatten()
    }

    fn note_remove(&mut self, source: StepGraphIndex, removed: usize) {
        let entries = self.map.get_mut(&source).expect("looked up before noting");
        for entry in entries.iter_mut() {
            match entry {
                Some(slot) if *slot == removed => *entry = None,
                Some(slot) if *slot > removed => *slot -= 1,
                _ => {}
            }
        }
    }

    fn note_insert(&mut self, source: StepGraphIndex, inserted: usize) {
        let entries = self.map.get_mut(&source).expect("looked up before noting");
        for slot in entries.iter_mut().flatten() {
            if *slot >= inserted {
                *slot += 1;
            }
        }
    }
}

/// Route a step command to its namespace: references into the ref table, everything else
/// into the node arena.
fn add_step_to_graph(graph: &mut StepGraph, step: Step) -> StepGraphIndex {
    match step {
        Step::Reference { refname, mutable } => graph.add_reference(refname, mutable),
        step => graph.add_node(step),
    }
}

/// Describes where relative to the selector a step should be inserted
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum InsertSide {
    /// When inserting above, any nodes that point to the selector will now
    /// point to the inserted node instead.
    ///
    /// IE: Any child commits will become a child of what is getting inserted.
    Above,
    /// When inserting below, any nodes that the selector points to will now be
    /// pointed to by the inserted node instead.
    ///
    /// IE: Any parent commits will become a parent of what is getting inserted.
    Below,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(InsertSide);

/// Controls where reparented insertion-location parents are ordered relative to
/// existing parents on the segment.
#[derive(Debug, Clone)]
pub enum ParentReparentingOrder {
    /// Put reparented insertion-location parents before existing segment parents.
    Prepend,
    /// Put reparented insertion-location parents after existing segment parents.
    Append,
}

/// Defines the start and end of a segment by pointing to it's parent-most and child-most nodes.
#[derive(Debug, Clone)]
pub struct SegmentDelimiter<C, P>
where
    C: ToSelector,
    P: ToSelector,
{
    /// The child-most node contained within the segment being defined.
    pub child: C,
    /// The parent-most node contained within the segment being defined.
    pub parent: P,
}

/// A set of some selectors
#[derive(Debug, Clone)]
pub struct SomeSelectors {
    selectors: Vec<AnySelector>,
}

impl SomeSelectors {
    /// Creates a set of selectors from different selector input types.
    ///
    /// Errors out if the selectors iterator is empty.
    pub fn new<T>(selectors: impl IntoIterator<Item = T>) -> Result<Self>
    where
        T: Into<AnySelector>,
    {
        let selectors: Vec<AnySelector> = selectors.into_iter().map(Into::into).collect();

        if selectors.is_empty() {
            return Err(anyhow!("Invalid selector set: This cannot be empty"));
        }

        Ok(Self { selectors })
    }

    /// Returns selectors as a slice.
    pub fn as_slice(&self) -> &[AnySelector] {
        &self.selectors
    }
}

/// A heterogeneous selector input.
#[derive(Debug, Clone)]
pub enum AnySelector {
    /// A selector that already points into the current graph revision.
    Selector(Selector),
    /// A commit id that should resolve to a pick step.
    Commit(gix::ObjectId),
    /// A reference name that should resolve to a reference step.
    Reference(gix::refs::FullName),
}

impl ToSelector for AnySelector {
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        match self {
            Self::Selector(selector) => selector.to_selector(editor),
            Self::Commit(id) => editor.select_commit(*id),
            Self::Reference(reference) => editor.select_reference(reference.as_ref()),
        }
    }
}

impl From<Selector> for AnySelector {
    fn from(value: Selector) -> Self {
        Self::Selector(value)
    }
}

impl From<gix::ObjectId> for AnySelector {
    fn from(value: gix::ObjectId) -> Self {
        Self::Commit(value)
    }
}

impl From<gix::refs::FullName> for AnySelector {
    fn from(value: gix::refs::FullName) -> Self {
        Self::Reference(value)
    }
}

impl<T> TryFrom<Vec<T>> for SomeSelectors
where
    T: Into<AnySelector>,
{
    type Error = anyhow::Error;

    fn try_from(value: Vec<T>) -> std::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Defines a set of node children or parents, to perform an action on.
///
/// Currently, this is used in the disconnect functionality.
#[derive(Debug, Clone, Default)]
pub enum SelectorSet {
    /// Select all of the children or parents.
    #[default]
    All,
    /// No children or parents should be selected.
    None,
    /// A subset of children or parents should be selected.
    Some(SomeSelectors),
}

/// An enum that is helpful for describing where something should be inserted
/// relative to.
#[derive(Debug, Clone)]
pub enum RelativeToRef<'a> {
    /// Relative to a commit
    Commit(gix::ObjectId),
    /// Relative to a reference
    Reference(&'a gix::refs::FullNameRef),
}

impl ToSelector for RelativeToRef<'_> {
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        match self {
            Self::Commit(id) => editor.select_commit(*id),
            Self::Reference(reference) => editor.select_reference(reference),
        }
    }
}

/// Specifies a location relative to which a commit operation should occur.
/// This is the fully-owned cousin of [RelativeTo].
#[derive(Debug, Clone)]
pub enum RelativeTo {
    /// Relative to a commit.
    Commit(gix::ObjectId),
    /// Relative to a reference.
    Reference(gix::refs::FullName),
}

impl ToSelector for RelativeTo {
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        match self {
            Self::Commit(commit) => editor.select_commit(*commit),
            Self::Reference(reference) => editor.select_reference(reference.as_ref()),
        }
    }
}

impl ToCommitSelector for gix::ObjectId {
    fn to_commit_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_commit(*self)
    }
}

impl ToCommitSelector for gix::Id<'_> {
    fn to_commit_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_commit(self.detach())
    }
}

impl ToSelector for gix::ObjectId {
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_commit(*self)
    }
}

impl ToSelector for gix::Id<'_> {
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_commit(self.detach())
    }
}

impl ToReferenceSelector for &gix::refs::FullNameRef {
    fn to_reference_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_reference(self)
    }
}

impl ToReferenceSelector for gix::refs::FullName {
    fn to_reference_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_reference(self.as_ref())
    }
}

impl ToSelector for &gix::refs::FullNameRef {
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_reference(self)
    }
}

impl ToSelector for gix::refs::FullName {
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        editor.select_reference(self.as_ref())
    }
}

/// Operations for mutating the commit graph
impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Get a selector to a particular commit in the graph
    pub fn select_commit(&self, target: gix::ObjectId) -> Result<Selector> {
        match self.try_select_commit(target) {
            Some(selector) => Ok(selector),
            None => Err(anyhow!("Failed to find commit {target} in rebase editor")),
        }
    }

    /// Get a selector to a particular reference in the graph
    pub fn select_reference(&self, target: &gix::refs::FullNameRef) -> Result<Selector> {
        match self.try_select_reference(target) {
            Some(selector) => Ok(selector),
            None => Err(anyhow!(
                "Failed to find reference {target} in rebase editor"
            )),
        }
    }

    /// Get a selector to a particular commit in the graph
    pub fn try_select_commit(&self, target: gix::ObjectId) -> Option<Selector> {
        for node_idx in self.graph.node_indices() {
            if self.graph.commit_id(node_idx) == Some(target) {
                return Some(self.new_selector(node_idx));
            }
        }

        None
    }

    /// Get a selector to a particular reference in the graph
    pub fn try_select_reference(&self, target: &gix::refs::FullNameRef) -> Option<Selector> {
        for (node_idx, refname, _) in self.graph.references() {
            if target == refname.as_ref() {
                return Some(self.new_selector(node_idx));
            }
        }

        None
    }

    /// Returns all direct children of `target` together with their edge order.
    ///
    /// Children are represented as incoming edges into `target` in the step graph.
    pub fn direct_children(&self, target: impl ToSelector) -> Result<Vec<(Selector, usize)>> {
        let target = target.to_selector(self)?;
        // A reference's children are the legs approaching its position (the node-era edges
        // into the reference).
        if self.graph.position_of(target.id).is_some() {
            return Ok(positions::ref_approach(&self.graph, target.id)
                .into_iter()
                .map(|(leg, slot)| (self.new_selector(leg), slot))
                .collect());
        }
        Ok(self
            .graph
            .incoming_legs(target.id)
            .into_iter()
            .map(|(child, slot)| (self.new_selector(child), slot))
            .collect())
    }

    /// Returns all direct parents of `target` together with their edge order.
    ///
    /// Parents are represented as outgoing edges from `target` in the step graph.
    pub fn direct_parents(&self, target: impl ToSelector) -> Result<Vec<(Selector, usize)>> {
        let target = target.to_selector(self)?;
        // A reference's one downward link is its anchor.
        if let Some(stored) = self.graph.position_of(target.id) {
            let anchor = positions::resolve_to_pick(&self.graph, stored.anchor)
                .context("Reference target should resolve to a commit")?;
            return Ok(vec![(self.new_selector(anchor), 0)]);
        }
        Ok(self
            .graph
            .parents(target.id)
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, parent)| (self.new_selector(parent), slot))
            .collect())
    }

    /// The node-era parent view of `target`: reference chains interpose on the links into
    /// their anchors, exactly as the reference NODES used to.
    ///
    /// For a pick, each parent slot resolves to the top of the chain it carries (falling back
    /// to the pick it points at); for a reference, the next chain member below, then the
    /// anchor. Useful for renderers that interleave references with commits.
    pub fn position_parents(&self, target: impl ToSelector) -> Result<Vec<Selector>> {
        let target = target.to_selector(self)?;
        if let Some(stored) = self.graph.position_of(target.id) {
            let anchor = positions::resolve_to_pick(&self.graph, stored.anchor)
                .context("Reference target should resolve to a commit")?;
            // The physical member directly below is stored adjacency; the anchor when at
            // the bottom of the stack.
            return Ok(vec![self.new_selector(stored.below.unwrap_or(anchor))]);
        }
        Ok(self
            .graph
            .parents(target.id)
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, pick)| {
                let carried_top = self
                    .graph
                    .positioned_refs()
                    .filter(|(node, stored)| {
                        positions::ref_approach(&self.graph, *node).contains(&(target.id, slot))
                            && positions::resolve_to_pick(&self.graph, stored.anchor) == Some(pick)
                    })
                    .map(|(node, _)| node)
                    .max_by_key(|&node| (positions::ref_depth(&self.graph, node), node));
                self.new_selector(carried_top.unwrap_or(pick))
            })
            .collect())
    }

    /// The node-era child view of `target` — the inverse of [`Self::position_parents`].
    ///
    /// For a pick, its children are the bottom members of the chains anchored on it plus the
    /// plain legs into it; for a reference, the next chain member above, else its legs.
    pub fn position_children(&self, target: impl ToSelector) -> Result<Vec<Selector>> {
        let target = target.to_selector(self)?;
        if let Some(stored) = self.graph.position_of(target.id) {
            let anchor = positions::resolve_to_pick(&self.graph, stored.anchor);
            // Everything that pointed at this reference in the node era: members sitting
            // directly on it (chain-mates and root siblings stacked above), plus — when
            // this is the top of its own approach group — the legs that enter its chain.
            let mut out: Vec<Selector> = self
                .graph
                .positioned_refs()
                .filter(|(node, other)| *node != target.id && other.below == Some(target.id))
                .map(|(node, _)| self.new_selector(node))
                .collect();
            let target_approach = positions::ref_approach(&self.graph, target.id);
            let target_depth = positions::ref_depth(&self.graph, target.id);
            let top_of_approach_group = !self.graph.positioned_refs().any(|(node, other)| {
                node != target.id
                    && positions::ref_approach(&self.graph, node) == target_approach
                    && positions::ref_depth(&self.graph, node) > target_depth
                    && positions::resolve_to_pick(&self.graph, other.anchor) == anchor
            });
            if top_of_approach_group {
                out.extend(
                    target_approach
                        .iter()
                        .map(|(leg, _)| self.new_selector(*leg)),
                );
            }
            out.sort_by_key(|s| s.id);
            out.dedup_by_key(|s| s.id);
            return Ok(out);
        }
        // Bottom members sit directly on the pick; other legs are plain.
        let mut out: Vec<Selector> = self
            .graph
            .positioned_refs()
            .filter(|(_, stored)| {
                stored.below.is_none()
                    && positions::resolve_to_pick(&self.graph, stored.anchor) == Some(target.id)
            })
            .map(|(node, _)| self.new_selector(node))
            .collect();
        for (child, slot) in self.graph.incoming_legs(target.id) {
            let carrying = self.graph.positioned_refs().any(|(node, stored)| {
                positions::ref_approach(&self.graph, node).contains(&(child, slot))
                    && positions::resolve_to_pick(&self.graph, stored.anchor) == Some(target.id)
            });
            if !carrying {
                out.push(self.new_selector(child));
            }
        }
        out.sort_by_key(|s| s.id);
        out.dedup_by_key(|s| s.id);
        Ok(out)
    }

    /// For a given step, find all the references that point to it.
    ///
    /// The reference selectors are provided in no particular order.
    pub fn step_references(&self, target: impl ToSelector) -> Result<Vec<Selector>> {
        let target = target.to_selector(self)?;

        Ok(
            crate::graph_rebase::positions::refs_anchored_at(&self.graph, target.id)
                .into_iter()
                .map(|node| self.new_selector(node))
                .collect(),
        )
    }

    /// Replaces the node that the function was pointing to.
    ///
    /// Replacement stays within its namespace: a pick can become a pick or a tombstone, a
    /// reference can be renamed or deleted — never one into the other.
    ///
    /// Returns the replaced step.
    pub fn replace(&mut self, target: impl ToSelector, step: Step) -> Result<Step> {
        let target = target.to_selector(self)?;
        let old = self.graph.step_view(target.id);
        let is_ref_slot = self.graph.reference_record(target.id).is_some();
        match (is_ref_slot, step) {
            (false, step @ (Step::Pick(_) | Step::None)) => self.graph.set_step(target.id, step),
            (true, Step::Reference { refname, mutable }) => {
                self.graph.set_reference(target.id, refname, mutable)
            }
            // Deleting a reference removes it from the physical stack: splice dependents
            // past it. Name and stored anchor are kept for retention reads.
            (true, Step::None) => {
                let was_live = self.graph.is_reference(target.id);
                self.graph.tombstone_reference(target.id);
                if was_live && let Some(stored) = self.graph.position_of(target.id) {
                    splice_out(&mut self.graph, target.id, stored.below);
                }
            }
            (false, Step::Reference { .. }) => {
                bail!("cannot replace a commit step with a reference")
            }
            (true, Step::Pick(_)) => bail!("cannot replace a reference with a commit step"),
        }
        Ok(old)
    }

    /// Disconnect a segment from a parent segment.
    ///
    /// `target` - The segment to disconnect.
    /// `children_to_disconnect` - Child nodes to disconnect from `target.child`.
    /// If `SelectorSet::All`, all incoming children of `target.child` are disconnected.
    ///
    /// `parents_to_disconnect` - Parent nodes to disconnect from `target.parent`.
    /// If `SelectorSet::All`, all outgoing parents of `target.parent` are disconnected.
    ///
    /// `target` delimiter's child and parent can be the same node.
    /// This is the way to disconnect a single node.
    ///
    /// All disconnected children will be reconnected to all the disconnected parents unless
    /// the `skip_reconnect_step` is set to true.
    ///
    /// Returns an error when:
    /// - `parents_to_disconnect` is `SelectorSet::None` and `skip_reconnect_step` is false.
    /// - `parents_to_disconnect` contains any parent that is not a direct parent of `target.parent`.
    /// - `children_to_disconnect` contains any child that is not a direct parent of `target.child`.
    pub fn disconnect_segment_from<C, P>(
        &mut self,
        target: SegmentDelimiter<C, P>,
        children_to_disconnect: SelectorSet,
        parents_to_disconnect: SelectorSet,
        skip_reconnect_step: bool,
    ) -> Result<()>
    where
        C: ToSelector,
        P: ToSelector,
    {
        let SegmentDelimiter { child, parent } = target;
        let mut target_child = child.to_selector(self)?;
        let mut target_parent = parent.to_selector(self)?;
        // A single-node segment that is just a reference: the node-era op unhooked the
        // reference pending a reconnect. As a position: it leaves its chain (members above
        // close the gap) and gives up its legs — with a reconnect they stay as plain edges
        // onto the anchor (the node-era rewire), without one they are removed outright.
        if target_child.id == target_parent.id && self.graph.position_of(target_child.id).is_some()
        {
            unhook_ref(&mut self.graph, target_child.id, skip_reconnect_step);
            return Ok(());
        }
        // A reference delimiter stands for the pick it resolves to: edges are the truth for
        // picks, and the reference's chain rides the pick's links as position data. A
        // reference child only owns the legs approaching its own chain — plain edges into
        // its anchor belong to the pick and stay.
        let child_ref_stored = self.graph.position_of(target_child.id);
        let child_ref_approach = child_ref_stored
            .as_ref()
            .map(|_| positions::ref_approach(&self.graph, target_child.id));
        let child_ref_depth = child_ref_stored
            .as_ref()
            .map(|_| positions::ref_depth(&self.graph, target_child.id));
        if let Some(pick) =
            crate::graph_rebase::positions::resolve_to_pick(&self.graph, target_child.id)
        {
            target_child = self.new_selector(pick);
        }
        if let Some(pick) =
            crate::graph_rebase::positions::resolve_to_pick(&self.graph, target_parent.id)
        {
            target_parent = self.new_selector(pick);
        }
        let children_to_disconnect = match children_to_disconnect {
            SelectorSet::All => None,
            SelectorSet::None => Some(Vec::new()),
            SelectorSet::Some(children) => Some(
                children
                    .as_slice()
                    .iter()
                    .map(|from_child| from_child.to_selector(self))
                    .collect::<Result<Vec<_>>>()?,
            ),
        };

        let parents_to_disconnect = match parents_to_disconnect {
            SelectorSet::All => None,
            SelectorSet::None => {
                if skip_reconnect_step {
                    Some(Vec::new())
                } else {
                    return Err(anyhow!(
                        "Invalid parents to disconnect: SelectorSet::None is not allowed"
                    ));
                }
            }
            SelectorSet::Some(parents) => Some(
                parents
                    .as_slice()
                    .iter()
                    .map(|from_parent| from_parent.to_selector(self))
                    .collect::<Result<Vec<_>>>()?,
            ),
        };

        // Legs from children, as frame-coordinate (child, slot) names.
        let incoming_legs = self
            .graph
            .incoming_legs(target_child.id)
            .into_iter()
            .filter(|leg| {
                child_ref_approach
                    .as_ref()
                    .is_none_or(|approach| approach.contains(leg))
            })
            .collect::<Vec<_>>();

        // Legs to parents, as frame-coordinate (slot, parent) entries.
        let outgoing_legs = self
            .graph
            .parents(target_parent.id)
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();

        // All available parents
        let available_parents = outgoing_legs
            .iter()
            .map(|(_, edge_target)| *edge_target)
            .collect::<HashSet<_>>();
        let available_children = incoming_legs
            .iter()
            .map(|(edge_source, _)| *edge_source)
            .collect::<HashSet<_>>();

        // Requested selectors that are references stand for the links their positions
        // decorate: a parent reference maps to its anchor pick; a child reference maps to the
        // pick(s) approaching its chain.
        let parents_to_disconnect = parents_to_disconnect.map(|parents| {
            parents
                .into_iter()
                .map(|selector| {
                    match crate::graph_rebase::positions::resolve_to_pick(&self.graph, selector.id)
                    {
                        Some(pick) if pick != selector.id => self.new_selector(pick),
                        _ => selector,
                    }
                })
                .collect::<Vec<_>>()
        });
        // A requested child that is a reference is a chain member above the segment: its
        // legs are the edges to disconnect, and the member itself (with everything above it
        // in its chain) follows the disconnected parents.
        let mut moving_ref_children: Vec<StepGraphIndex> = Vec::new();
        let children_to_disconnect = children_to_disconnect.map(|children| {
            children
                .into_iter()
                .flat_map(|selector| match self.graph.position_of(selector.id) {
                    Some(_) => {
                        let legs = positions::ref_approach(&self.graph, selector.id)
                            .into_iter()
                            .map(|(child, _)| self.new_selector(child))
                            .collect::<Vec<_>>();
                        moving_ref_children.push(selector.id);
                        legs
                    }
                    None => vec![selector],
                })
                .collect::<Vec<_>>()
        });

        // 1. Verify that all parents and children to disconnect are directly connected to the target segment.
        if let Some(parents_to_disconnect) = parents_to_disconnect.as_ref() {
            for selector in parents_to_disconnect {
                if !available_parents.contains(&selector.id) {
                    return Err(anyhow!(
                        "Invalid parent delimitation: requested parent is not a direct parent of target.parent"
                    ));
                }
            }
        }

        if let Some(children_to_disconnect) = children_to_disconnect.as_ref() {
            for selector in children_to_disconnect {
                if !available_children.contains(&selector.id) {
                    return Err(anyhow!(
                        "Invalid parent delimitation: requested child is not a direct parent of target.child"
                    ));
                }
            }
        }

        let parent_ids_to_disconnect = parents_to_disconnect
            .as_ref()
            .map(|parents| parents.iter().map(|s| s.id).collect::<HashSet<_>>());
        let child_ids_to_disconnect = children_to_disconnect
            .as_ref()
            .map(|children| children.iter().map(|s| s.id).collect::<HashSet<_>>());

        // One ledger spans both loops: the overlap case (the segment's parent-most sitting
        // directly above the child-most's anchor) captures the same leg in both frames.
        let mut ledger = SlotLedger::default();
        let mut disconnected_parent_edges: Vec<(usize, StepGraphIndex)> = Vec::new();
        let mut carried_parent_tops: Vec<StepGraphIndex> = Vec::new();
        // 2. Disconnect parents. Chains the removed legs carried lose them from their approach legs.
        for (frame_slot, edge_target) in outgoing_legs {
            let should_disconnect = parent_ids_to_disconnect
                .as_ref()
                .is_none_or(|ids| ids.contains(&edge_target));
            if should_disconnect {
                // Earlier removals shift this leg down; the ledger resolves the captured name
                // to the slot the store uses now. Shifts preserve relative order, so the slots
                // recorded here sort the disconnected parents exactly as their captured orders
                // did.
                let slot = ledger
                    .current(&self.graph, target_parent.id, frame_slot)
                    .context("BUG: disconnected parent leg vanished")?;
                let removed = (target_parent.id, slot);
                // Chains this leg carried — captured BEFORE it is removed, since the derived
                // approach reflects the live parent arrays. Removing it then drops the leg from
                // every derived approach automatically (no approach bookkeeping needed).
                let carried: Vec<_> = self
                    .graph
                    .positioned_refs()
                    .filter(|(node, _)| {
                        positions::ref_approach(&self.graph, *node).contains(&removed)
                    })
                    .collect();
                // The node-era parent this leg pointed at was the top of the chain it
                // carried — remember it so disconnected child refs can stack above it.
                if let Some(top) = carried
                    .iter()
                    .filter(|(_, stored)| {
                        positions::resolve_to_pick(&self.graph, stored.anchor) == Some(edge_target)
                    })
                    .map(|(node, _)| *node)
                    .max_by_key(|&node| (positions::ref_depth(&self.graph, node), node))
                {
                    carried_parent_tops.push(top);
                }
                self.graph.remove_parent(target_parent.id, slot);
                ledger.note_remove(target_parent.id, slot);
                disconnected_parent_edges.push((slot, edge_target));
            }
        }

        // 3. Disconnect children and reconnect to the disconnected parents.
        let full_child_disconnect = child_ids_to_disconnect.is_none();
        let mut sorted_disconnected = disconnected_parent_edges.clone();
        sorted_disconnected.sort_by_key(|(slot, _)| *slot);
        // The node era resolved a rewired reference through its first (lowest-slot) parent.
        let chain_anchor = sorted_disconnected.first().map(|(_, target)| *target);
        for (edge_source, frame_slot) in incoming_legs {
            let should_disconnect = child_ids_to_disconnect
                .as_ref()
                .is_none_or(|ids| ids.contains(&edge_source));
            if !should_disconnect {
                continue;
            }
            // Earlier removals on the same child shift this leg down; the ledger resolves the
            // captured name to the slot the store uses now.
            let Some(slot) = ledger.current(&self.graph, edge_source, frame_slot) else {
                // The parent loop already removed this leg: the delimiters can name
                // overlapping legs when the segment's parent-most sits directly above
                // the child-most's anchor. Only the reconnect still applies.
                if !skip_reconnect_step {
                    self.reconnect_edges_to_parents(&disconnected_parent_edges, edge_source);
                }
                continue;
            };
            let carrying = self.graph.positioned_refs().any(|(node, stored)| {
                positions::ref_approach(&self.graph, node).contains(&(edge_source, slot))
                    && positions::resolve_to_pick(&self.graph, stored.anchor)
                        == Some(target_child.id)
            });
            if !skip_reconnect_step
                && carrying
                && child_ref_approach.is_none()
                && !sorted_disconnected.is_empty()
            {
                // A leg that carried the target's chain was, in the node era, an edge into
                // the chain — it never lost its parent slot. Fan it out in place: the first
                // disconnected parent takes the leg's slot (the statement keeps its name, so
                // the carried chains follow), the rest slot in right after.
                let mut targets = sorted_disconnected.iter().map(|(_, target)| *target);
                self.graph
                    .replace_parent(edge_source, slot, targets.next().expect("non-empty"));
                for (offset, target) in targets.enumerate() {
                    self.graph
                        .insert_parent(edge_source, slot + 1 + offset, target);
                    ledger.note_insert(edge_source, slot + 1 + offset);
                }
                continue;
            }
            // Remove the child leg. The chains it carried lose it from their derived approach
            // automatically — the leg is gone.
            self.graph.remove_parent(edge_source, slot);
            ledger.note_remove(edge_source, slot);
            if skip_reconnect_step {
                continue;
            }
            // Reconnect the child node to all the disconnected parents.
            self.reconnect_edges_to_parents(&disconnected_parent_edges, edge_source);
        }
        // The target's chains were the node-era direct children of its pick: a full child
        // disconnect rewires them onto the first disconnected parent, approach preserved. A
        // reference child delimiter means the segment INCLUDES that reference and its chain
        // at or below its rank — those stay with the segment.
        if let Some(anchor) = chain_anchor {
            for moving_node in &moving_ref_children {
                transfer_stack(&mut self.graph, *moving_node, target_child.id, anchor);
            }
        }
        if full_child_disconnect && let Some(anchor) = chain_anchor {
            match &child_ref_stored {
                None => {
                    // When the disconnected parent edge carried a chain, the node-era parent
                    // was that chain's top ref — the child refs stack above it and follow it
                    // through later moves. The top's feeder was emptied in step 2; step 3's
                    // reconnect bridged fresh legs into `anchor`, and the joined tower rests
                    // behind that full bridged leg set (AllLegs, correct in the merge case too).
                    let landed = carried_parent_tops.first().is_some_and(|&top| {
                        land_stack_above(&mut self.graph, target_child.id, top, anchor)
                    });
                    if !landed {
                        positions::reanchor_refs_at(
                            &mut self.graph,
                            target_child.id,
                            anchor,
                            false,
                        );
                    }
                }
                Some(_) => {
                    // The delimiter and its chain at or below its depth stay with the segment;
                    // the lane slice above it follows the pick move verbatim.
                    let delimiter_approach = child_ref_approach.clone().unwrap_or_default();
                    carry_stack_above(
                        &mut self.graph,
                        target_child.id,
                        &delimiter_approach,
                        child_ref_depth.unwrap_or_default(),
                        anchor,
                    );
                }
            }
        }

        // 4. References whose anchor no longer resolves (it sat on the now-disconnected
        // segment) re-anchor to the segment's first disconnected parent — the ruled dangling
        // semantics: the position follows where the commit's place went, the approach (approach)
        // stays, so a rewired child renders its chain exactly as the edge-era rewire did.
        if let Some(new_anchor) = disconnected_parent_edges.first().map(|(_, target)| *target) {
            readopt_dangling_refs(&mut self.graph, new_anchor);
        }

        Ok(())
    }

    /// Remove the child edge, and reconnect to the right parents.
    fn reconnect_edges_to_parents(
        &mut self,
        disconnected_parent_edges: &[(usize, StepGraphIndex)],
        child_node: StepGraphIndex,
    ) {
        // Reconnect the child node to all the disconnected parents, appended after
        // `child_node`'s existing parents in their original relative order.
        let mut disconnected_parent_edges = disconnected_parent_edges.iter().collect::<Vec<_>>();
        disconnected_parent_edges.sort_by_key(|(slot, _)| *slot);
        for (_, edge_target) in disconnected_parent_edges {
            self.graph.push_parent(child_node, *edge_target);
        }
    }

    /// Returns the parent-slot orders assigned to `new_parent_nodes`, in the given order.
    /// Existing parent edges that get renumbered carry their approach entries along.
    fn add_edges_to_parents(
        &mut self,
        child_node: StepGraphIndex,
        new_parent_nodes: impl IntoIterator<Item = StepGraphIndex>,
        parent_reparenting_order: ParentReparentingOrder,
    ) -> Vec<usize> {
        match parent_reparenting_order {
            // Insertion-location parents define the first-parent lane. Existing parents stay
            // attached after them as merge-side parents.
            ParentReparentingOrder::Prepend => new_parent_nodes
                .into_iter()
                .enumerate()
                .map(|(slot, parent_node)| {
                    self.graph.insert_parent(child_node, slot, parent_node);
                    slot
                })
                .collect(),
            ParentReparentingOrder::Append => new_parent_nodes
                .into_iter()
                .map(|parent_node| self.graph.push_parent(child_node, parent_node))
                .collect(),
        }
    }

    /// Insert a segment relative to a selector.
    ///
    /// `target` - Selector to insert the segment relative to.
    ///
    /// `delimiter` - The segment is described by its delimiter: First (parent-most) and last (child-most) node.
    ///
    /// `side` - The relative side to do the insertion.
    ///
    /// `nodes_to_connect` - Optional set of selector to connect instead of the parents/children determined.
    ///
    /// `parent_reparenting_order` - Controls how newly connected parent edges are ordered relative to
    /// existing parent edges on the parent-most node of the inserted segment. With
    /// [`ParentReparentingOrder::Prepend`], the newly connected parents become the lowest-order parents,
    /// which makes the first inserted/reparented parent the first-parent traversal path. Existing parents
    /// remain attached after them in their previous relative order. With
    /// [`ParentReparentingOrder::Append`], existing parents keep the lowest parent orders and the newly
    /// connected parents are appended after them.
    ///
    /// If `nodes_to_connect` is None:
    ///     If inserted above, all the target selector's children will be disconnected and reconnected to the last
    ///     node of the segment. If inserted below, all the target selector's parents will be disconnected and
    ///     reconnected to the parent-most node of the segment using `parent_reparenting_order`.
    /// If `nodes_to_connect` is Some:
    ///     If inserted above, connect the given nodes as children. If inserted below, connect the given nodes as parents
    ///     using `parent_reparenting_order`.
    ///
    pub fn insert_segment_into<C, P>(
        &mut self,
        target: impl ToSelector,
        delimiter: SegmentDelimiter<C, P>,
        side: InsertSide,
        nodes_to_connect: Option<SomeSelectors>,
        parent_reparenting_order: ParentReparentingOrder,
    ) -> Result<()>
    where
        C: ToSelector,
        P: ToSelector,
    {
        let SegmentDelimiter { child, parent } = delimiter;
        let target = target.to_selector(self)?;
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;

        // An empty segment — a lone reference — is pure position data: it slots into the
        // target's chain, and any nodes to connect become its approaching legs.
        if child.id == parent.id && self.graph.position_of(child.id).is_some() {
            let slot = match (side, self.graph.position_of(target.id)) {
                (InsertSide::Above, Some(_)) => StackSlot::Above(target.id),
                (InsertSide::Below, Some(_)) => StackSlot::Below(target.id),
                (InsertSide::Above, None) => StackSlot::Bottom(target.id),
                (InsertSide::Below, None) => {
                    // On top of the chain the target pick's first-parent leg approaches.
                    let first_parent = self
                        .graph
                        .parents(target.id)
                        .first()
                        .copied()
                        .context("Cannot insert a reference below a parentless commit")?;
                    StackSlot::LaneTop {
                        pick: first_parent,
                        leg: (target.id, 0),
                    }
                }
            };
            move_ref(&mut self.graph, child.id, slot);
            if let Some(nodes_to_connect) = nodes_to_connect {
                for any_selector in nodes_to_connect.as_slice() {
                    let selector = any_selector.to_selector(self)?;
                    let node = selector;
                    self.push_edge(node, child)?;
                }
            }
            return Ok(());
        }

        match side {
            InsertSide::Above => {
                if let Some(nodes_to_connect) = nodes_to_connect {
                    // If there were nodes to connect defined, create edges from them into the child node of the segment
                    // being inserted. `push_edge` appends after `node`'s existing parents and
                    // handles a reference child-most (the leg joins its chain).
                    for any_selector in nodes_to_connect.as_slice() {
                        let selector = any_selector.to_selector(self)?;
                        let node = selector;
                        self.push_edge(node, child)?;
                    }
                } else if let Some(stored) = self.graph.position_of(target.id) {
                    // Above a reference: split the chain there. Members above move onto the
                    // segment's child-most pick; the reference and members below are now
                    // approached by its parent-most pick.
                    let anchor_pick = positions::resolve_to_pick(&self.graph, stored.anchor)
                        .context("Reference target should resolve to a commit")?;
                    let child_pick = positions::resolve_to_pick(&self.graph, child.id)
                        .context("Segment child should resolve to a commit")?;
                    let parent_pick = positions::resolve_to_pick(&self.graph, parent.id)
                        .context("Segment parent should resolve to a commit")?;
                    let target_approach = positions::ref_approach(&self.graph, target.id);
                    let split =
                        split_chain(&mut self.graph, target.id, SplitBoundary::Above, child_pick);
                    if !split.moved_any {
                        // The chain's legs now enter through the segment's child-most pick: each
                        // leg keeps its slot, so its chain statement follows the name.
                        for &(leg, slot) in &target_approach {
                            if self.graph.parents(leg).get(slot) == Some(&anchor_pick) {
                                self.graph.replace_parent(leg, slot, child_pick);
                            }
                        }
                    }
                    // Connect the parent-most node to the reference's anchor; a reference
                    // parent-most (an empty segment) re-anchors instead of gaining edges. The
                    // target reference and members below are now approached through that leg.
                    let entry_slot = if self.graph.position_of(parent.id).is_some() {
                        let anchor_selector = self.new_selector(anchor_pick);
                        self.insert_edge(parent, anchor_selector, 0)?;
                        // The segment is positioned data: the split-off lower chain is
                        // approached through the segment's chain, which ends at the anchor.
                        0
                    } else {
                        let orders = self.add_edges_to_parents(
                            parent.id,
                            [anchor_pick],
                            parent_reparenting_order,
                        );
                        orders.first().copied().unwrap_or(0)
                    };
                    settle_chain_lower(&mut self.graph, &split.lower, (parent_pick, entry_slot));
                    return Ok(());
                } else {
                    // The segment's child-most takes the target's place in each child's parent
                    // array: the slot — and any statement on it — is untouched.
                    self.graph.redirect_children(target.id, child.id);
                    // The target's chains slide under the segment: refs anchored on the target
                    // move up onto the segment's child-most pick.
                    if let Some(child_pick) = positions::resolve_to_pick(&self.graph, child.id) {
                        positions::reanchor_refs_at(&mut self.graph, target.id, child_pick, true);
                    }
                }

                // Connect the target to the parent-most node in the given segment according to
                // the requested parent ordering policy. A reference target stands for its
                // anchor, with the new leg entering its chain; a reference parent-most (an
                // empty segment) has no edges — it re-anchors onto the target instead.
                if self.graph.position_of(parent.id).is_some() {
                    self.insert_edge(parent, target, 0)?;
                } else {
                    let connect_to = match self.graph.position_of(target.id) {
                        Some(stored) => positions::resolve_to_pick(&self.graph, stored.anchor)
                            .context("Reference target should resolve to a commit")?,
                        None => target.id,
                    };
                    let join = (connect_to != target.id)
                        .then(|| positions::prepare_chain_join(&self.graph, target.id));
                    let orders = self.add_edges_to_parents(
                        parent.id,
                        [connect_to],
                        parent_reparenting_order,
                    );
                    if let (Some(join), Some(order)) = (join, orders.first()) {
                        positions::apply_chain_join(&mut self.graph, &join, (parent.id, *order));
                    }
                }
            }
            InsertSide::Below => {
                let mut moved_leg_orders = Vec::new();
                let mut ref_parents: Vec<(usize, StepGraphIndex)> = Vec::new();
                let parents_to_add = if let Some(nodes_to_connect) = nodes_to_connect {
                    let mut nodes = Vec::new();
                    for any_selector in nodes_to_connect.as_slice() {
                        let selector = any_selector.to_selector(self)?;
                        let node = selector;
                        // A reference parent: the pick edge goes to its anchor and the leg
                        // joins its chain once the final slot is known.
                        if self.graph.position_of(node.id).is_some() {
                            let anchor = positions::resolve_to_pick(&self.graph, node.id)
                                .context("Reference target should resolve to a commit")?;
                            ref_parents.push((nodes.len(), node.id));
                            nodes.push(anchor);
                        } else {
                            nodes.push(node.id);
                        }
                    }
                    nodes
                } else if let Some(t_stored) = self.graph.position_of(target.id) {
                    // A reference target's one downward link is its anchor: the segment goes
                    // between the reference and it. The reference's own re-anchoring onto the
                    // segment happens in the connect step below.
                    vec![
                        positions::resolve_to_pick(&self.graph, t_stored.anchor)
                            .context("Reference target should resolve to a commit")?,
                    ]
                } else {
                    // Statements stay named (target, slot) until the rename below moves them
                    // onto the segment's parent-most.
                    let drained = self.graph.drain_parents(target.id);
                    moved_leg_orders = (0..drained.len()).collect();
                    drained
                };

                // A reference target re-anchors onto the child-most, dragging its approaching
                // legs along, so it connects BEFORE the segment gains its own downward parent
                // edge: if the segment's fresh parent leg already existed (an `AllLegs` ref sees
                // every leg into its anchor), that leg would be dragged too and self-loop the
                // segment. A plain target connects AFTER instead: its orphaned leg statements
                // must first be renamed onto the segment's parent-most below — the fresh leg
                // at slot 0 would otherwise take their names. `parents_to_add` is captured up
                // front, so it still names the pre-re-anchor target position.
                let target_is_ref = self.graph.position_of(target.id).is_some();
                if target_is_ref {
                    // A reference child-most stands for its anchor, with the leg entering its chain.
                    self.insert_edge(target, child, 0)?;
                }

                // A reference parent-most (an empty segment) has no edges — it re-anchors
                // onto its first new parent instead of gaining edges.
                if self.graph.position_of(parent.id).is_some() {
                    if let Some(first) = parents_to_add.first() {
                        let first = self.new_selector(*first);
                        self.insert_edge(parent, first, 0)?;
                    }
                } else {
                    let joins: Vec<_> = ref_parents
                        .iter()
                        .map(|(k, ref_node)| {
                            (*k, positions::prepare_chain_join(&self.graph, *ref_node))
                        })
                        .collect();
                    let new_orders = self.add_edges_to_parents(
                        parent.id,
                        parents_to_add,
                        parent_reparenting_order,
                    );
                    for (k, join) in &joins {
                        if let Some(order) = new_orders.get(*k) {
                            positions::apply_chain_join(&mut self.graph, join, (parent.id, *order));
                        }
                    }
                    // Chains those legs carried are now approached through the segment's
                    // parent-most pick.
                    for (old_order, new_order) in moved_leg_orders.iter().zip(new_orders) {
                        self.graph
                            .rename_leg((target.id, *old_order), (parent.id, new_order));
                    }
                }

                if !target_is_ref {
                    // A plain target keeps its existing parents in front; the segment appends.
                    let slot = self.graph.parent_count(target.id);
                    self.insert_edge(target, child, slot)?;
                }
            }
        }

        Ok(())
    }
    /// Insert a segment relative to a selector.
    ///
    /// The segment is described by its delimiter: First (parent-most) and last (child-most) node.
    ///
    /// If inserted above, all the target selector's children will be disconnected and reconnected to the last
    /// node of the segment.
    /// If inserted below, all the target selector's parents will be disconnected and reconnected to the
    /// parent-most node of the segment.
    ///
    /// Reparented parents are prepended by default: newly connected parents receive the lowest parent orders,
    /// so the first inserted/reparented parent becomes the first-parent traversal path and existing parents
    /// remain attached after them in their previous relative order. Use [`Self::insert_segment_into`] with
    /// [`ParentReparentingOrder::Append`] when existing parents should keep the lowest parent orders instead.
    ///
    pub fn insert_segment<C, P>(
        &mut self,
        target: impl ToSelector,
        delimiter: SegmentDelimiter<C, P>,
        side: InsertSide,
    ) -> Result<()>
    where
        C: ToSelector,
        P: ToSelector,
    {
        self.insert_segment_into(
            target,
            delimiter,
            side,
            None,
            ParentReparentingOrder::Prepend,
        )
    }

    /// Add a step node to the graph.
    ///
    /// Almost always you really want to use `insert` function instead.
    pub fn add_step(&mut self, step: Step) -> Result<Selector> {
        let new_idx = add_step_to_graph(&mut self.graph, step);
        Ok(self.new_selector(new_idx))
    }

    /// Inserts a new node relative to a selector
    ///
    /// When inserting above, any nodes that point to the selector will now
    /// point to the inserted node instead. When inserting below, any nodes
    /// that the selector points to will now be pointed to by the inserted node
    /// instead.
    ///
    /// Returns a selector to the inserted step
    pub fn insert(
        &mut self,
        target: impl ToSelector,
        step: Step,
        side: InsertSide,
    ) -> Result<Selector> {
        let target = target.to_selector(self)?;
        let inserting_reference = matches!(step, Step::Reference { .. });
        let target_anchor = self.graph.position_of(target.id);
        match (side, target_anchor) {
            (InsertSide::Above, None) if !inserting_reference => {
                // Above a pick: the interposed node slides under the pick's chains — its
                // children rewire to the new node with slots preserved (so stored approach
                // legs stay valid) and every ref anchored on it moves up.
                let new_idx = self.graph.add_node(step);
                self.graph.redirect_children(target.id, new_idx);
                self.graph.push_parent(new_idx, target.id);
                positions::reanchor_refs_at(&mut self.graph, target.id, new_idx, false);

                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Above, None) => {
                // A reference above a pick becomes the bottom of the pick's stack.
                let new_idx = add_step_to_graph(&mut self.graph, step);
                place_ref(&mut self.graph, new_idx, StackSlot::Bottom(target.id));
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Above, Some(stored)) if !inserting_reference => {
                // A pick above a reference splits the chain at that reference: members above
                // move onto the new pick, the reference and members below are now approached
                // by it.
                let anchor_pick = positions::resolve_to_pick(&self.graph, stored.anchor)
                    .context("Reference target should resolve to a commit")?;
                // Capture the target's approaching legs BEFORE adding the new pick's edge, so the
                // fresh edge does not leak into `target_approach` (which `is_top` would then re-move
                // onto itself, a self-loop).
                let target_approach = positions::ref_approach(&self.graph, target.id);
                let new_idx = self.graph.add_node(step);
                self.graph.push_parent(new_idx, anchor_pick);
                let split = split_chain(&mut self.graph, target.id, SplitBoundary::Above, new_idx);
                settle_chain_lower(&mut self.graph, &split.lower, (new_idx, 0));
                if !split.moved_any {
                    // The chain's legs now enter through the new pick, slots untouched.
                    for &(leg, slot) in &target_approach {
                        if self.graph.parents(leg).get(slot) == Some(&anchor_pick) {
                            self.graph.replace_parent(leg, slot, new_idx);
                        }
                    }
                }
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Above, Some(_)) => {
                // A reference above a reference joins its chain one rank up.
                let new_idx = add_step_to_graph(&mut self.graph, step);
                place_ref(&mut self.graph, new_idx, StackSlot::Above(target.id));
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Below, None) if !inserting_reference => {
                // Below a pick: the pick's whole parent array moves onto the new node with
                // slots preserved, so chains carried by those legs follow the rename.
                let new_idx = self.graph.add_node(step);
                self.graph.transplant_parents(target.id, new_idx);
                self.graph.push_parent(target.id, new_idx);

                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Below, None) => {
                // A reference below a pick sits on top of the chain the pick's first-parent
                // leg approaches (or starts one).
                let first_parent = self.graph.parents(target.id).first().copied();
                let new_idx = add_step_to_graph(&mut self.graph, step);
                if let Some(parent_pick) = first_parent {
                    place_ref(
                        &mut self.graph,
                        new_idx,
                        StackSlot::LaneTop {
                            pick: parent_pick,
                            leg: (target.id, 0),
                        },
                    );
                }
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Below, Some(stored)) if !inserting_reference => {
                // A pick below a reference splits the chain there: the reference and members
                // above re-anchor onto the new pick, members below are approached by it.
                let anchor_pick = positions::resolve_to_pick(&self.graph, stored.anchor)
                    .context("Reference target should resolve to a commit")?;
                // Capture the target's approaching legs BEFORE the new pick's edge is added.
                let target_approach = positions::ref_approach(&self.graph, target.id);
                let new_idx = self.graph.add_node(step);
                self.graph.push_parent(new_idx, anchor_pick);
                let split = split_chain(&mut self.graph, target.id, SplitBoundary::At, new_idx);
                settle_chain_lower(&mut self.graph, &split.lower, (new_idx, 0));
                // The legs enter the (moved) upper part of the chain, which now rests on the
                // new pick, slots untouched.
                for &(leg, slot) in &target_approach {
                    if self.graph.parents(leg).get(slot) == Some(&anchor_pick) {
                        self.graph.replace_parent(leg, slot, new_idx);
                    }
                }
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Below, Some(_)) => {
                // A reference below a reference takes its position; it and everything above
                // shift up.
                let new_idx = add_step_to_graph(&mut self.graph, step);
                place_ref(&mut self.graph, new_idx, StackSlot::Below(target.id));
                Ok(self.new_selector(new_idx))
            }
        }
    }

    /// Append an edge from `child` to `parent` after `child`'s existing parents.
    ///
    /// Reference endpoints behave as in [`Self::insert_edge`].
    pub fn push_edge(&mut self, child: impl ToSelector, parent: impl ToSelector) -> Result<()> {
        self.insert_edge(child, parent, usize::MAX)
    }

    /// Insert an edge from `child` to `parent` at `slot` among `child`'s ordered parents
    /// (clamped to the end); parents at `slot` and later shift up, statements following.
    ///
    /// An edge FROM a reference is its downward link: it POSITIONS the reference at the
    /// parent, never a raw edge (references are positions). A LIVE reference re-anchors
    /// through the arrangement machinery; a DEAD one (which upstream-integration retention
    /// still redirects) just re-points its retained anchor — no chain cascade, since its
    /// stored position is stale. An edge INTO a reference enters its chain: the pick edge
    /// goes to the anchor and the reference (with members below it) gains the new leg.
    pub fn insert_edge(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
        slot: usize,
    ) -> Result<()> {
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;
        self.debug_assert_acyclic(child.id, parent.id)?;

        if self.graph.reference_record(child.id).is_some() {
            let new_anchor = match self.graph.position_of(parent.id) {
                Some(parent_stored) => {
                    positions::resolve_to_pick(&self.graph, parent_stored.anchor)
                        .context("Reference target should resolve to a commit")?
                }
                None => parent.id,
            };
            if self.graph.is_reference(child.id) {
                repoint_ref(&mut self.graph, child.id, new_anchor);
            } else {
                self.graph.set_retained_anchor(child.id, new_anchor);
            }
            return Ok(());
        }
        let parent_ref = self.graph.position_of(parent.id);
        let parent_pick = match &parent_ref {
            Some(stored) => positions::resolve_to_pick(&self.graph, stored.anchor)
                .context("Reference target should resolve to a commit")?,
            None => parent.id,
        };
        let slot = self.graph.insert_parent(child.id, slot, parent_pick);
        // The chain is captured AFTER the insert: normalization and the shift rename the
        // child's statements, so a pre-capture would hold stale leg names.
        if parent_ref.is_some() {
            let join = positions::prepare_chain_join(&self.graph, parent.id);
            positions::apply_chain_join(&mut self.graph, &join, (child.id, slot));
        }
        Ok(())
    }

    fn debug_assert_acyclic(&self, child: StepGraphIndex, parent: StepGraphIndex) -> Result<()> {
        if cfg!(debug_assertions) {
            let mut seen = HashSet::from([parent]);
            let mut tips = vec![parent];

            while let Some(tip) = tips.pop() {
                for parent in self.graph.parents(tip) {
                    if seen.insert(parent) {
                        tips.push(parent);
                    }
                }
            }

            if seen.contains(&child) {
                bail!("BUG: Add edge introduces a cycle");
            }
        }
        Ok(())
    }

    /// Removes all edges between a child and parent, returning the (pre-removal, ascending)
    /// parent slots they occupied. Later slots shift down, statements following.
    pub fn remove_edges(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
    ) -> Result<Vec<usize>> {
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;

        // A reference child holds one conceptual downward edge (order 0) — its anchor. It is
        // reported but not cleared; a follow-up insert_edge re-anchors, and a position without
        // a resolving anchor is not representable.
        if let Some(stored) = self.graph.position_of(child.id) {
            let resolves_to_parent = positions::resolve_to_pick(&self.graph, stored.anchor)
                == positions::resolve_to_pick(&self.graph, parent.id);
            return Ok(if resolves_to_parent { vec![0] } else { vec![] });
        }
        let slots = match self.graph.position_of(parent.id) {
            // Disconnecting from a reference removes the legs carrying its chain — the
            // node-era edge into the reference node.
            Some(stored) => {
                let target_pick = positions::resolve_to_pick(&self.graph, stored.anchor)
                    .context("Reference target should resolve to a commit")?;
                let parent_approach = positions::ref_approach(&self.graph, parent.id);
                self.graph
                    .parents(child.id)
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(slot, target)| {
                        (target == target_pick && parent_approach.contains(&(child.id, slot)))
                            .then_some(slot)
                    })
                    .collect::<Vec<_>>()
            }
            // Disconnecting from a pick removes its slots; chains riding a removed leg lose
            // it from their approach legs below.
            None => self
                .graph
                .parents(child.id)
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(slot, target)| (target == parent.id).then_some(slot))
                .collect::<Vec<_>>(),
        };

        // Highest-first so earlier slots keep their names; report the pre-removal slots.
        for slot in slots.iter().rev() {
            self.graph
                .remove_parent(child.id, *slot)
                .context("BUG: Failed to remove parent slot")?;
        }

        Ok(slots)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn empty_selector_set_creation_fails() {
        let empty_parent_set = SomeSelectors::new(Vec::<gix::ObjectId>::new())
            .expect_err("expected empty selector set creation to fail");
        assert!(
            empty_parent_set
                .to_string()
                .contains("Invalid selector set: This cannot be empty"),
            "unexpected error: {empty_parent_set:#}"
        );
    }
}
