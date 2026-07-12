//! Operations for mutating the editor

use std::collections::{HashMap, HashSet};

use crate::graph_rebase::graph_editor::{PickIndex, RefIndex};
use crate::graph_rebase::ref_ops::{
    SplitBoundary, StackPlace, carry_stack_above, land_stack_above, move_ref, place_ref,
    readopt_dangling_refs, redirect_edges, repoint_ref, settle_group_lower, split_group,
    transfer_stack, unhook_ref,
};
use crate::graph_rebase::{EditorIndex, GraphEditor, positions};
use anyhow::{Context as _, Result, anyhow, bail};
use but_core::RefMetadata;
use serde::{Deserialize, Serialize};

use crate::graph_rebase::selector::{SelectorSet, SomeSelectors, StepRange};
use crate::graph_rebase::{Editor, Selector, Step, ToSelector};

/// Edge positions captured at one instant (a frame), looked up later against parent arrays
/// that have since shifted. `current` maps a captured position to today's parent number;
/// removals and inserts are noted so later lookups stay aligned. `None` means the named
/// edge itself was already removed.
#[derive(Default)]
struct NumberLedger {
    map: HashMap<PickIndex, Vec<Option<usize>>>,
}

impl NumberLedger {
    /// Today's parent number of the edge captured as `(source, frame_number)`, or `None` if it was
    /// removed. The identity map is built lazily, so a source must be looked up before any
    /// of its parent numbers mutate.
    fn current(
        &mut self,
        graph: &GraphEditor,
        source: PickIndex,
        frame_number: usize,
    ) -> Option<usize> {
        self.map
            .entry(source)
            .or_insert_with(|| (0..graph.parent_count(source)).map(Some).collect())
            .get(frame_number)
            .copied()
            .flatten()
    }

    fn note_remove(&mut self, source: PickIndex, removed: usize) {
        let entries = self.map.get_mut(&source).expect("looked up before noting");
        for entry in entries.iter_mut() {
            match entry {
                Some(parent_number) if *parent_number == removed => *entry = None,
                Some(parent_number) if *parent_number > removed => *parent_number -= 1,
                _ => {}
            }
        }
    }

    fn note_insert(&mut self, source: PickIndex, inserted: usize) {
        let entries = self.map.get_mut(&source).expect("looked up before noting");
        for parent_number in entries.iter_mut().flatten() {
            if *parent_number >= inserted {
                *parent_number += 1;
            }
        }
    }
}

/// The ref-table handle of `entry`; an error when it addresses a commit instead.
fn ref_entry(entry: EditorIndex) -> Result<RefIndex> {
    entry
        .as_ref()
        .context("operation targets a reference, but a commit was addressed")
}

/// The node-arena handle of `entry`; an error when it addresses a reference instead.
pub(crate) fn node_entry(entry: EditorIndex) -> Result<PickIndex> {
    entry
        .as_pick()
        .context("operation targets a commit, but a reference was addressed")
}

/// Phase 1 of [`Editor::disconnect_range_from`]: verify that all parents and children to
/// disconnect (`None` = all) are directly connected to the target range.
fn verify_disconnect_bounds(
    parents_to_disconnect: Option<&[Selector]>,
    children_to_disconnect: Option<&[Selector]>,
    available_parents: &HashSet<PickIndex>,
    available_children: &HashSet<PickIndex>,
) -> Result<()> {
    fn verify(
        to_disconnect: Option<&[Selector]>,
        available: &HashSet<PickIndex>,
        what: &str,
    ) -> Result<()> {
        for selector in to_disconnect.into_iter().flatten() {
            if !selector
                .id
                .as_pick()
                .is_some_and(|n| available.contains(&n))
            {
                return Err(anyhow!(
                    "Invalid {what} delimitation: requested {what} is not a direct {what} of target.{what}"
                ));
            }
        }
        Ok(())
    }
    verify(parents_to_disconnect, available_parents, "parent")?;
    verify(children_to_disconnect, available_children, "child")
}

/// Route a step command to its namespace: references into the ref table, everything else
/// into the node arena.
fn add_step_to_graph(graph: &mut GraphEditor, step: Step) -> EditorIndex {
    match step {
        Step::Reference { refname, mutable } => graph.add_reference(refname, mutable).into(),
        Step::Pick(pick) => graph.add_node(Some(pick)).into(),
        Step::None => graph.add_node(None).into(),
    }
}

/// Describes where relative to the selector a step should be inserted
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum InsertSide {
    /// Children of the selector become children of the inserted entry.
    Above,
    /// Parents of the selector become parents of the inserted entry.
    Below,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(InsertSide);

/// Controls where reparented insertion-location parents are ordered relative to
/// existing parents on the range.
#[derive(Debug, Clone)]
pub enum ParentReparentingOrder {
    /// Reparented parents come first: the first of them becomes the first-parent
    /// traversal path; existing parents stay attached after them as merge-side parents.
    Prepend,
    /// Existing parents keep the lowest parent orders; reparented parents append after them.
    Append,
}

/// Operations for mutating the editor graph
impl<M: RefMetadata> Editor<'_, M> {
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
        for node_idx in self.graph.node_ids() {
            if self.graph.commit_id(node_idx) == Some(target) {
                return Some(self.new_selector(node_idx));
            }
        }

        None
    }

    /// Get a selector to a particular reference in the graph
    pub fn try_select_reference(&self, target: &gix::refs::FullNameRef) -> Option<Selector> {
        for (ref_idx, refname, _) in self.graph.references() {
            if target == refname.as_ref() {
                return Some(self.new_selector(ref_idx));
            }
        }

        None
    }

    /// Replace the step at `target`, returning the old one. Replacing a pick with another
    /// pick records a commit mapping from the old to the new id. Replacement stays within
    /// its namespace: a pick can become a pick or a tombstone, a reference can be renamed
    /// or deleted — never one into the other.
    pub fn replace(&mut self, target: impl ToSelector, step: Step) -> Result<Step> {
        let target = target.to_selector(self)?;
        let old = self.graph.step_view(target.id);
        if let (Step::Pick(from), Step::Pick(to)) = (&old, &step)
            && !from.exclude_from_tracking
            && !to.exclude_from_tracking
        {
            self.history.update_mapping(from.id, to.id);
        }
        let is_reference_target = self.graph.state_of(target.id).is_some();
        if is_reference_target {
            self.ensure_mutable_ref(target.id)?;
        }
        match (is_reference_target, step) {
            (false, Step::Pick(pick)) => self.graph.set_step(node_entry(target.id)?, Some(pick)),
            (false, Step::None) => self.graph.set_step(node_entry(target.id)?, None),
            (true, Step::Reference { refname, mutable }) => {
                self.graph
                    .set_reference(ref_entry(target.id)?, refname, mutable)
            }
            // Deleting a reference removes it from the physical stack: splice dependents
            // past it. Name and stored position are kept for retention reads.
            (true, Step::None) => {
                let entry = ref_entry(target.id)?;
                let was_live = self.graph.is_reference(entry);
                self.graph.tombstone_reference(entry);
                if was_live {
                    self.graph.splice(entry);
                }
            }
            (false, Step::Reference { .. }) => {
                bail!("cannot replace a commit step with a reference")
            }
            (true, Step::Pick(_)) => bail!("cannot replace a reference with a commit step"),
        }
        Ok(old)
    }

    /// Disconnect a range (child and parent may be the same entry) from its surroundings:
    /// children from `target.child`, parents from `target.parent`. `SelectorSet::All`
    /// disconnects everything; a `Some` set must name direct children/parents or this
    /// errors. Disconnected children are reconnected to the disconnected parents unless
    /// `skip_reconnect_step` is set — which is also the only way `parents_to_disconnect`
    /// may be `SelectorSet::None`.
    pub fn disconnect_range_from<C, P>(
        &mut self,
        target: StepRange<C, P>,
        children_to_disconnect: SelectorSet,
        parents_to_disconnect: SelectorSet,
        skip_reconnect_step: bool,
    ) -> Result<()>
    where
        C: ToSelector,
        P: ToSelector,
    {
        let StepRange { child, parent } = target;
        let mut target_child = child.to_selector(self)?;
        let mut target_parent = parent.to_selector(self)?;
        // A single-entry range that is just a reference: it leaves its group (members above
        // close the gap) and gives up its edges — with a reconnect they stay as plain edges
        // onto the pick, without one they are removed outright.
        if target_child.id == target_parent.id && self.graph.is_positioned(target_child.id) {
            self.ensure_mutable_ref(target_child.id)?;
            unhook_ref(
                &mut self.graph,
                ref_entry(target_child.id)?,
                skip_reconnect_step,
            );
            return Ok(());
        }
        // A reference range stands for the pick it resolves to: edges are the truth for
        // picks, and the reference's group rides the pick's links as position data. A
        // reference child only owns the edges entering its own group — plain edges into
        // its pick belong to it and stay.
        let child_ref_positioned = self.graph.is_positioned(target_child.id);
        let child_ref_edges =
            child_ref_positioned.then(|| positions::edges_through(&self.graph, target_child.id));
        let child_ref_depth = child_ref_positioned
            .then_some(())
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

        // The range bounds as PICKS: a reference bound stands for the pick it resolves to.
        let parent_node = self.resolved_pick(target_parent.id)?;
        let child_node = self.resolved_pick(target_child.id)?;
        // Edges from children, as frame-coordinate (child, parent number) names.
        let incoming_edges = self
            .graph
            .incoming_edges(target_child.id)
            .iter()
            .copied()
            .filter(|edge| {
                child_ref_edges
                    .as_ref()
                    .is_none_or(|entering| entering.contains(edge))
            })
            .collect::<Vec<_>>();

        // Edges to parents, as frame-coordinate (parent number, parent) entries.
        let outgoing_edges = self
            .graph
            .parents(parent_node)
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();

        // All available parents
        let available_parents = outgoing_edges
            .iter()
            .map(|(_, edge_target)| *edge_target)
            .collect::<HashSet<_>>();
        let available_children = incoming_edges
            .iter()
            .map(|(edge_source, _)| *edge_source)
            .collect::<HashSet<_>>();

        // Requested selectors that are references stand for the links their positions
        // decorate: a parent reference maps to its resolved pick; a child reference maps to the
        // pick(s) entering its group.
        let parents_to_disconnect = parents_to_disconnect.map(|parents| {
            parents
                .into_iter()
                .map(|selector| {
                    match crate::graph_rebase::positions::resolve_to_pick(&self.graph, selector.id)
                    {
                        Some(pick) if EditorIndex::from(pick) != selector.id => {
                            self.new_selector(pick)
                        }
                        _ => selector,
                    }
                })
                .collect::<Vec<_>>()
        });
        // A requested child that is a reference is a group member above the range: its
        // edges are the edges to disconnect, and the member itself (with everything above it
        // in its group) follows the disconnected parents.
        let mut moving_ref_children: Vec<RefIndex> = Vec::new();
        let children_to_disconnect = children_to_disconnect.map(|children| {
            children
                .into_iter()
                .flat_map(|selector| {
                    if self.graph.is_positioned(selector.id) {
                        let edges = positions::edges_through(&self.graph, selector.id)
                            .into_iter()
                            .map(|(child, _)| self.new_selector(child))
                            .collect::<Vec<_>>();
                        moving_ref_children.extend(selector.id.as_ref());
                        edges
                    } else {
                        vec![selector]
                    }
                })
                .collect::<Vec<_>>()
        });

        // 1. Verify the requested bounds, then resolve them to node-id sets (`None` = all).
        verify_disconnect_bounds(
            parents_to_disconnect.as_deref(),
            children_to_disconnect.as_deref(),
            &available_parents,
            &available_children,
        )?;
        let parent_ids_to_disconnect = parents_to_disconnect.as_ref().map(|parents| {
            parents
                .iter()
                .filter_map(|s| s.id.as_pick())
                .collect::<HashSet<_>>()
        });
        let child_ids_to_disconnect = children_to_disconnect.as_ref().map(|children| {
            children
                .iter()
                .filter_map(|s| s.id.as_pick())
                .collect::<HashSet<_>>()
        });

        // One ledger spans both disconnect phases: the overlap case (the range's parent-most
        // sitting directly above the child-most's pick) captures the same edge in both frames.
        let mut ledger = NumberLedger::default();
        // 2. Disconnect parents.
        let (disconnected_parent_edges, carried_parent_tops) = self.disconnect_range_parents(
            parent_node,
            outgoing_edges,
            parent_ids_to_disconnect.as_ref(),
            &mut ledger,
        )?;
        // 3. Disconnect children and reconnect to the disconnected parents.
        self.disconnect_range_children(
            child_node,
            incoming_edges,
            child_ids_to_disconnect.as_ref(),
            child_ref_edges.as_ref(),
            child_ref_depth,
            &moving_ref_children,
            &carried_parent_tops,
            &disconnected_parent_edges,
            &mut ledger,
            skip_reconnect_step,
        );
        // 4. Re-adopt references left dangling on the range.
        self.readopt_range_danglers(&disconnected_parent_edges);

        Ok(())
    }

    /// Phase 2 of [`Self::disconnect_range_from`]: disconnect the selected parents (`None`
    /// = all) from `parent_node`. Groups the removed edges carried lose them from their
    /// entering edges. Returns the removed edges as `(parent number, parent)` entries, plus the top
    /// ref of each group a removed edge carried — the interposed parent the edge pointed at
    /// — so disconnected child refs can stack above it.
    #[expect(clippy::type_complexity)]
    fn disconnect_range_parents(
        &mut self,
        parent_node: PickIndex,
        outgoing_edges: Vec<(usize, PickIndex)>,
        parent_ids_to_disconnect: Option<&HashSet<PickIndex>>,
        ledger: &mut NumberLedger,
    ) -> Result<(Vec<(usize, PickIndex)>, Vec<RefIndex>)> {
        let mut disconnected_parent_edges: Vec<(usize, PickIndex)> = Vec::new();
        let mut carried_parent_tops: Vec<RefIndex> = Vec::new();
        for (frame_number, edge_target) in outgoing_edges {
            let should_disconnect =
                parent_ids_to_disconnect.is_none_or(|ids| ids.contains(&edge_target));
            if should_disconnect {
                // The ledger resolves the captured name to today's parent number (earlier removals
                // shift edges down). Shifts preserve relative order, so the recorded parent numbers
                // still sort the disconnected parents as their captured orders did.
                let parent_number = ledger
                    .current(&self.graph, parent_node, frame_number)
                    .context("BUG: disconnected parent edge vanished")?;
                let removed = (parent_node, parent_number);
                // Groups this edge carried — captured BEFORE it is removed, since the derived
                // entering set reflects the live parent arrays. Removing it then drops the edge from
                // every derived read automatically (no group bookkeeping needed).
                let carried: Vec<_> = self
                    .graph
                    .positioned_refs()
                    .filter(|&entry| {
                        positions::edges_through(&self.graph, entry).contains(&removed)
                    })
                    .collect();
                // The interposed parent this edge pointed at was the top of the group it
                // carried — remember it so disconnected child refs can stack above it.
                if let Some(top) = carried
                    .iter()
                    .copied()
                    .filter(|&entry| {
                        positions::resolve_to_pick(&self.graph, entry) == Some(edge_target)
                    })
                    .max_by_key(|&entry| (positions::ref_depth(&self.graph, entry), entry))
                {
                    carried_parent_tops.push(top);
                }
                self.graph.remove_parent(parent_node, parent_number);
                ledger.note_remove(parent_node, parent_number);
                disconnected_parent_edges.push((parent_number, edge_target));
            }
        }
        Ok((disconnected_parent_edges, carried_parent_tops))
    }

    /// Phase 3 of [`Self::disconnect_range_from`]: disconnect the selected children (`None`
    /// = all) and reconnect them to the disconnected parents (unless
    /// `skip_reconnect_step`), then move the groups that rode the range onto the first
    /// disconnected parent. A `Some` `child_ref_edges` means the range's child bound was a
    /// reference and holds the edges entering its group.
    #[allow(clippy::too_many_arguments)]
    fn disconnect_range_children(
        &mut self,
        child_node: PickIndex,
        incoming_edges: Vec<(PickIndex, usize)>,
        child_ids_to_disconnect: Option<&HashSet<PickIndex>>,
        child_ref_edges: Option<&Vec<(PickIndex, usize)>>,
        child_ref_depth: Option<usize>,
        moving_ref_children: &[RefIndex],
        carried_parent_tops: &[RefIndex],
        disconnected_parent_edges: &[(usize, PickIndex)],
        ledger: &mut NumberLedger,
        skip_reconnect_step: bool,
    ) {
        let full_child_disconnect = child_ids_to_disconnect.is_none();
        let mut sorted_disconnected = disconnected_parent_edges.to_vec();
        sorted_disconnected.sort_by_key(|(parent_number, _)| *parent_number);
        // A rewired reference resolves through its first (lowest-parent number) disconnected parent.
        let group_pick = sorted_disconnected.first().map(|(_, target)| *target);
        for (edge_source, frame_number) in incoming_edges {
            let should_disconnect =
                child_ids_to_disconnect.is_none_or(|ids| ids.contains(&edge_source));
            if !should_disconnect {
                continue;
            }
            // Earlier removals on the same child shift this edge down; the ledger resolves the
            // captured name to the parent number the store uses now.
            let Some(parent_number) = ledger.current(&self.graph, edge_source, frame_number) else {
                // The parent loop already removed this edge: the range bounds can name
                // overlapping edges when the range's parent-most sits directly above
                // the child-most's pick. Only the reconnect still applies.
                if !skip_reconnect_step {
                    self.reconnect_edges_to_parents(disconnected_parent_edges, edge_source);
                }
                continue;
            };
            let carrying = self.graph.positioned_refs().any(|entry| {
                positions::edges_through(&self.graph, entry).contains(&(edge_source, parent_number))
                    && positions::resolve_to_pick(&self.graph, entry) == Some(child_node)
            });
            if !skip_reconnect_step
                && carrying
                && child_ref_edges.is_none()
                && !sorted_disconnected.is_empty()
            {
                // An edge that carried the target's group is an edge into the group — it never
                // lost its parent. Fan it out in place: the first disconnected parent takes
                // the edge's parent number (the statement keeps its name, so the carried
                // groups follow), the rest are inserted right after.
                let mut targets = sorted_disconnected.iter().map(|(_, target)| *target);
                self.graph.replace_parent(
                    edge_source,
                    parent_number,
                    targets.next().expect("non-empty"),
                );
                for (offset, target) in targets.enumerate() {
                    self.graph
                        .insert_parent(edge_source, parent_number + 1 + offset, target);
                    ledger.note_insert(edge_source, parent_number + 1 + offset);
                }
                continue;
            }
            // Remove the child edge; groups it carried lose it from their derived
            // entering set automatically.
            self.graph.remove_parent(edge_source, parent_number);
            ledger.note_remove(edge_source, parent_number);
            if skip_reconnect_step {
                continue;
            }
            // Reconnect the child entry to all the disconnected parents.
            self.reconnect_edges_to_parents(disconnected_parent_edges, edge_source);
        }
        // The target's groups were the interposed direct children of its pick: a full child
        // disconnect rewires them onto the first disconnected parent, entering edges preserved. A
        // reference child bound means the range INCLUDES that reference and its group
        // at or below its rank — those stay with the range.
        if let Some(pick) = group_pick {
            for moving_node in moving_ref_children {
                transfer_stack(&mut self.graph, *moving_node, child_node, pick);
            }
        }
        if full_child_disconnect && let Some(pick) = group_pick {
            match child_ref_edges {
                None => {
                    // When the disconnected parent edge carried a group, the interposed parent
                    // was that group's top ref — the child refs stack above it and follow it
                    // through later moves. Step 2 removed the edge that used to enter this
                    // group; step 3's reconnect added fresh edges into `pick`. The combined
                    // stack now sits behind all of those fresh edges (`GroupCarry::All`), which
                    // is also right when `pick` is a merge.
                    let landed = carried_parent_tops.first().is_some_and(|&top| {
                        land_stack_above(&mut self.graph, child_node, top, pick)
                    });
                    if !landed {
                        positions::reposition_refs(&mut self.graph, child_node, pick, false);
                    }
                }
                Some(child_bound_edges) => {
                    // The bound and its group at or below its depth stay with the range;
                    // the group slice above it follows the pick move verbatim.
                    carry_stack_above(
                        &mut self.graph,
                        child_node,
                        child_bound_edges,
                        child_ref_depth.unwrap_or_default(),
                        pick,
                    );
                }
            }
        }
    }

    /// Phase 4 of [`Self::disconnect_range_from`]: references whose pick sat on the
    /// now-disconnected range re-point to the range's first disconnected parent — dangling
    /// references follow where the commit's place went; their entering edges stay.
    fn readopt_range_danglers(&mut self, disconnected_parent_edges: &[(usize, PickIndex)]) {
        if let Some(onto) = disconnected_parent_edges.first().map(|(_, target)| *target) {
            readopt_dangling_refs(&mut self.graph, onto);
        }
    }

    /// Reconnect `child_node` to the disconnected parents, appended after its existing
    /// parents in their original relative order.
    fn reconnect_edges_to_parents(
        &mut self,
        disconnected_parent_edges: &[(usize, PickIndex)],
        child_node: PickIndex,
    ) {
        let mut disconnected_parent_edges = disconnected_parent_edges.iter().collect::<Vec<_>>();
        disconnected_parent_edges.sort_by_key(|(parent_number, _)| *parent_number);
        for (_, edge_target) in disconnected_parent_edges {
            self.graph.push_parent(child_node, *edge_target);
        }
    }

    /// Returns the parent number orders assigned to `new_parent_nodes`, in the given order.
    /// Existing parent edges that get renumbered carry their group entries along.
    fn add_edges_to_parents(
        &mut self,
        child_node: PickIndex,
        new_parent_nodes: impl IntoIterator<Item = PickIndex>,
        parent_reparenting_order: ParentReparentingOrder,
    ) -> Vec<usize> {
        match parent_reparenting_order {
            // Insertion-location parents define the first-parent line. Existing parents stay
            // attached after them as merge-side parents.
            ParentReparentingOrder::Prepend => new_parent_nodes
                .into_iter()
                .enumerate()
                .map(|(parent_number, parent_node)| {
                    self.graph
                        .insert_parent(child_node, parent_number, parent_node);
                    parent_number
                })
                .collect(),
            ParentReparentingOrder::Append => new_parent_nodes
                .into_iter()
                .map(|parent_node| self.graph.push_parent(child_node, parent_node))
                .collect(),
        }
    }

    /// Insert a range (parent-most `range.parent` up to child-most `range.child`) relative
    /// to `target`. With `nodes_to_connect` unset, the target's children ([`InsertSide::Above`])
    /// or parents ([`InsertSide::Below`]) are disconnected and reconnected onto the range;
    /// when set, those entries are connected instead. [`ParentReparentingOrder`] decides
    /// whether newly connected parents go before or after existing parents on the range's
    /// parent-most entry.
    pub fn insert_range_into<C, P>(
        &mut self,
        target: impl ToSelector,
        range: StepRange<C, P>,
        side: InsertSide,
        nodes_to_connect: Option<SomeSelectors>,
        parent_reparenting_order: ParentReparentingOrder,
    ) -> Result<()>
    where
        C: ToSelector,
        P: ToSelector,
    {
        let StepRange { child, parent } = range;
        let target = target.to_selector(self)?;
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;

        // An empty range — a lone reference — is pure position data: it folds into the
        // target's group, and any entries to connect become the edges entering through it.
        if child.id == parent.id && self.graph.is_positioned(child.id) {
            self.ensure_mutable_ref(child.id)?;
            let parent_number = match (side, self.graph.is_positioned(target.id)) {
                (InsertSide::Above, true) => StackPlace::Above(ref_entry(target.id)?),
                (InsertSide::Below, true) => StackPlace::Below(ref_entry(target.id)?),
                (InsertSide::Above, false) => StackPlace::Bottom(node_entry(target.id)?),
                (InsertSide::Below, false) => {
                    // On top of the group the target pick's first-parent edge enters.
                    let target_node = node_entry(target.id)?;
                    let first_parent = self
                        .graph
                        .parents(target_node)
                        .first()
                        .copied()
                        .context("Cannot insert a reference below a parentless commit")?;
                    StackPlace::GroupTop {
                        pick: first_parent,
                        edge: (target_node, 0),
                    }
                }
            };
            move_ref(&mut self.graph, ref_entry(child.id)?, parent_number);
            if let Some(nodes_to_connect) = nodes_to_connect {
                self.push_edges_onto(&nodes_to_connect, child)?;
            }
            return Ok(());
        }

        match side {
            InsertSide::Above => self.insert_range_above(
                target,
                child,
                parent,
                nodes_to_connect,
                parent_reparenting_order,
            ),
            InsertSide::Below => self.insert_range_below(
                target,
                child,
                parent,
                nodes_to_connect,
                parent_reparenting_order,
            ),
        }
    }

    /// The [`InsertSide::Above`] arm of [`Self::insert_range_into`]: the range's child-most
    /// takes over the target's children (or gains `nodes_to_connect` as children), and the
    /// target connects under the range's parent-most.
    fn insert_range_above(
        &mut self,
        target: Selector,
        child: Selector,
        parent: Selector,
        nodes_to_connect: Option<SomeSelectors>,
        parent_reparenting_order: ParentReparentingOrder,
    ) -> Result<()> {
        if let Some(nodes_to_connect) = nodes_to_connect {
            // Connect the given entries as children of the range's child-most.
            // `push_edge` appends after existing parents and handles a reference
            // child-most (the edge joins its group).
            self.push_edges_onto(&nodes_to_connect, child)?;
        } else if let Some(on) = self.graph.positioned_on(target.id) {
            // Above a reference: split the group there. Members above move onto the
            // range's child-most pick; the reference and members below are now
            // entered through its parent-most pick.
            let pick = self.resolved_pick(on)?;
            let child_pick = positions::resolve_to_pick(&self.graph, child.id)
                .context("Range child should resolve to a commit")?;
            let parent_pick = positions::resolve_to_pick(&self.graph, parent.id)
                .context("Range parent should resolve to a commit")?;
            let target_edges = positions::edges_through(&self.graph, target.id);
            let split = split_group(
                &mut self.graph,
                ref_entry(target.id)?,
                SplitBoundary::Above,
                child_pick,
            );
            if !split.moved_any {
                // The group's edges now enter through the range's child-most pick.
                redirect_edges(&mut self.graph, &target_edges, pick, child_pick);
            }
            // Connect the parent-most entry to the reference's pick; a reference
            // parent-most (an empty range) re-points instead of gaining edges. The
            // target reference and members below are now entered through that edge.
            let entry_number = if self.graph.is_positioned(parent.id) {
                let pick_selector = self.new_selector(pick);
                self.insert_edge(parent, pick_selector, 0)?;
                // The range is positioned data: the split-off lower group is
                // entered through the range's group, which ends at the pick.
                0
            } else {
                let orders = self.add_edges_to_parents(
                    node_entry(parent.id)?,
                    [pick],
                    parent_reparenting_order,
                );
                orders.first().copied().unwrap_or(0)
            };
            settle_group_lower(&mut self.graph, &split.lower, (parent_pick, entry_number));
            return Ok(());
        } else {
            // The range's child-most takes the target's place in each child's parent
            // array: the parent number — and any statement on it — is untouched.
            self.graph
                .redirect_children(node_entry(target.id)?, node_entry(child.id)?);
            // The target's groups slide under the range: refs sitting on the target
            // move up onto the range's child-most pick.
            if let Some(child_pick) = positions::resolve_to_pick(&self.graph, child.id) {
                positions::reposition_refs(
                    &mut self.graph,
                    node_entry(target.id)?,
                    child_pick,
                    true,
                );
            }
        }

        // Connect the target to the parent-most entry in the given range according to
        // the requested parent ordering policy. A reference target stands for its
        // pick, with the new edge entering its group; a reference parent-most (an
        // empty range) has no edges — it re-points onto the target instead.
        if self.graph.is_positioned(parent.id) {
            self.insert_edge(parent, target, 0)?;
        } else {
            let connect_to = match self.graph.positioned_on(target.id) {
                Some(on) => self.resolved_pick(on)?,
                None => node_entry(target.id)?,
            };
            let join = (EditorIndex::from(connect_to) != target.id)
                .then(|| ref_entry(target.id))
                .transpose()?
                .map(|r| positions::prepare_group_join(&self.graph, r));
            let parent_node = node_entry(parent.id)?;
            let orders =
                self.add_edges_to_parents(parent_node, [connect_to], parent_reparenting_order);
            if let (Some(join), Some(order)) = (join, orders.first()) {
                positions::apply_group_join(&mut self.graph, &join, (parent_node, *order));
            }
        }
        Ok(())
    }

    /// The [`InsertSide::Below`] arm of [`Self::insert_range_into`]: the target's parents
    /// (or `nodes_to_connect`) become parents of the range's parent-most, and the range's
    /// child-most connects under the target.
    fn insert_range_below(
        &mut self,
        target: Selector,
        child: Selector,
        parent: Selector,
        nodes_to_connect: Option<SomeSelectors>,
        parent_reparenting_order: ParentReparentingOrder,
    ) -> Result<()> {
        let mut moved_edge_orders = Vec::new();
        let mut ref_parents: Vec<(usize, RefIndex)> = Vec::new();
        let parents_to_add = if let Some(nodes_to_connect) = nodes_to_connect {
            let mut entries = Vec::new();
            for any_selector in nodes_to_connect.as_slice() {
                let entry = any_selector.to_selector(self)?;
                // A reference parent: the pick edge goes to its pick and the edge
                // joins its group once the final parent number is known.
                if self.graph.is_positioned(entry.id) {
                    let pick = self.resolved_pick(entry.id)?;
                    ref_parents.push((entries.len(), ref_entry(entry.id)?));
                    entries.push(pick);
                } else {
                    entries.push(node_entry(entry.id)?);
                }
            }
            entries
        } else if let Some(t_on) = self.graph.positioned_on(target.id) {
            // A reference target's one downward link is its pick: the range goes
            // between the reference and it. The reference's own re-pointing onto the
            // range happens in the connect step below.
            vec![self.resolved_pick(t_on)?]
        } else {
            // Statements stay named (target, parent number) until the rename below moves them
            // onto the range's parent-most.
            let drained = self.graph.drain_parents(node_entry(target.id)?);
            moved_edge_orders = (0..drained.len()).collect();
            drained
        };

        // Ordering matters. A reference target connects BEFORE the range gains its own
        // downward edge: re-pointing drags its entering edges along, and a pre-existing
        // fresh edge (a `GroupCarry::All` ref sees every edge into its pick) would be dragged
        // too and self-loop the range. A plain target connects AFTER: its orphaned edge
        // statements must first be renamed onto the range's parent-most, or the fresh
        // edge at parent number 0 would take their names. `parents_to_add` was captured up front,
        // so it still names the pre-re-point target position.
        let target_is_ref = self.graph.is_positioned(target.id);
        if target_is_ref {
            // A reference child-most stands for its pick, with the edge entering its group.
            self.insert_edge(target, child, 0)?;
        }

        // A reference parent-most (an empty range) has no edges — it re-points
        // onto its first new parent instead of gaining edges.
        if self.graph.is_positioned(parent.id) {
            if let Some(first) = parents_to_add.first() {
                let first = self.new_selector(*first);
                self.insert_edge(parent, first, 0)?;
            }
        } else {
            let joins: Vec<_> = ref_parents
                .iter()
                .map(|(k, ref_node)| (*k, positions::prepare_group_join(&self.graph, *ref_node)))
                .collect();
            let parent_node = node_entry(parent.id)?;
            let new_orders =
                self.add_edges_to_parents(parent_node, parents_to_add, parent_reparenting_order);
            for (k, join) in &joins {
                if let Some(order) = new_orders.get(*k) {
                    positions::apply_group_join(&mut self.graph, join, (parent_node, *order));
                }
            }
            // Groups those edges carried are now entered through the range's
            // parent-most pick. Only a plain (drained) target has moved edges; a
            // reference target contributes none.
            if !moved_edge_orders.is_empty() {
                let target_node = node_entry(target.id)?;
                let renames: Vec<_> = moved_edge_orders
                    .iter()
                    .zip(new_orders)
                    .map(|(old, new)| ((target_node, *old), (parent_node, new)))
                    .collect();
                self.graph.rename_edges(&renames);
            }
        }

        if !target_is_ref {
            // A plain target keeps its existing parents in front; the range appends.
            let parent_number = self.graph.parent_count(target.id);
            self.insert_edge(target, child, parent_number)?;
        }
        Ok(())
    }

    /// [`Self::insert_range_into`] with no `nodes_to_connect` and
    /// [`ParentReparentingOrder::Prepend`]: the target's children (above) or parents (below)
    /// are reconnected onto the range, reparented parents becoming the first-parent line.
    pub fn insert_range<C, P>(
        &mut self,
        target: impl ToSelector,
        range: StepRange<C, P>,
        side: InsertSide,
    ) -> Result<()>
    where
        C: ToSelector,
        P: ToSelector,
    {
        self.insert_range_into(target, range, side, None, ParentReparentingOrder::Prepend)
    }

    /// Add a step to the graph, unconnected. Almost always you want [`Self::insert`] instead.
    pub fn add_step(&mut self, step: Step) -> Result<Selector> {
        let new_idx = add_step_to_graph(&mut self.graph, step);
        Ok(self.new_selector(new_idx))
    }

    /// Insert a new step relative to `target` (see [`InsertSide`] for how edges rewire),
    /// returning a selector to it.
    pub fn insert(
        &mut self,
        target: impl ToSelector,
        step: Step,
        side: InsertSide,
    ) -> Result<Selector> {
        let target = target.to_selector(self)?;
        let inserting_reference = matches!(step, Step::Reference { .. });
        let target_positioned = self.graph.is_positioned(target.id);
        match (side, target_positioned) {
            (InsertSide::Above, false) if !inserting_reference => {
                // Above a pick: the interposed entry slides under the pick's groups — its
                // children rewire to the new entry with parent numbers preserved (so stored group
                // edges stay valid) and every ref sitting on it moves up.
                let Step::Pick(pick) = step else {
                    bail!("BUG: this arm inserts picks only");
                };
                let new_idx = self.graph.add_node(Some(pick));
                let target_node = node_entry(target.id)?;
                self.graph.redirect_children(target_node, new_idx);
                self.graph.push_parent(new_idx, target_node);
                positions::reposition_refs(&mut self.graph, target_node, new_idx, false);

                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Above, false) => {
                // A reference above a pick becomes the bottom of the pick's stack.
                let new_idx = add_step_to_graph(&mut self.graph, step);
                place_ref(
                    &mut self.graph,
                    ref_entry(new_idx)?,
                    StackPlace::Bottom(node_entry(target.id)?),
                );
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Above, true) if !inserting_reference => {
                // A pick above a reference splits the group at that reference: members above
                // move onto the new pick, the reference and members below are now entered
                // through it.
                let on = self
                    .graph
                    .positioned_on(target.id)
                    .expect("checked positioned above");
                self.interpose_pick_at_ref(target, on, step, SplitBoundary::Above)
            }
            (InsertSide::Above, true) => {
                // A reference above a reference joins its group one rank up.
                let new_idx = add_step_to_graph(&mut self.graph, step);
                place_ref(
                    &mut self.graph,
                    ref_entry(new_idx)?,
                    StackPlace::Above(ref_entry(target.id)?),
                );
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Below, false) if !inserting_reference => {
                // Below a pick: the pick's whole parent array moves onto the new entry with
                // parent numbers preserved, so groups carried by those edges follow the rename.
                let Step::Pick(pick) = step else {
                    bail!("BUG: this arm inserts picks only");
                };
                let target_node = node_entry(target.id)?;
                let new_idx = self.graph.add_node(Some(pick));
                self.graph.transplant_parents(target_node, new_idx);
                self.graph.push_parent(target_node, new_idx);

                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Below, false) => {
                // A reference below a pick sits on top of the group the pick's first-parent
                // edge enters (or starts one).
                let target_node = node_entry(target.id)?;
                let first_parent = self.graph.parents(target_node).first().copied();
                let new_idx = add_step_to_graph(&mut self.graph, step);
                if let Some(parent_pick) = first_parent {
                    place_ref(
                        &mut self.graph,
                        ref_entry(new_idx)?,
                        StackPlace::GroupTop {
                            pick: parent_pick,
                            edge: (target_node, 0),
                        },
                    );
                }
                Ok(self.new_selector(new_idx))
            }
            (InsertSide::Below, true) if !inserting_reference => {
                // A pick below a reference splits the group there: the reference and members
                // above re-point onto the new pick, members below are entered through it.
                self.ensure_mutable_ref(target.id)?;
                let on = self
                    .graph
                    .positioned_on(target.id)
                    .expect("checked positioned above");
                self.interpose_pick_at_ref(target, on, step, SplitBoundary::At)
            }
            (InsertSide::Below, true) => {
                // A reference below a reference takes its position; it and everything above
                // shift up.
                let new_idx = add_step_to_graph(&mut self.graph, step);
                place_ref(
                    &mut self.graph,
                    ref_entry(new_idx)?,
                    StackPlace::Below(ref_entry(target.id)?),
                );
                Ok(self.new_selector(new_idx))
            }
        }
    }

    /// Bail when `entry` is an immutable reference (e.g. a remote-tracking ref):
    /// materialization would refuse the write, so the op fails up front instead of
    /// succeeding session-only.
    fn ensure_mutable_ref(&self, entry: EditorIndex) -> Result<()> {
        if let Some(record) = self.graph.state_of(entry)
            && !record.mutable
        {
            bail!(
                "reference {} is immutable and cannot be moved, renamed, or deleted",
                record.refname
            );
        }
        Ok(())
    }

    /// The pick `entry` resolves to; an error when it resolves to nothing (an unborn ref).
    pub(crate) fn resolved_pick(&self, entry: impl Into<EditorIndex>) -> Result<PickIndex> {
        positions::resolve_to_pick(&self.graph, entry)
            .context("Reference target should resolve to a commit")
    }

    /// [`Self::push_edge`] from each of `entries` onto `parent`.
    fn push_edges_onto(&mut self, entries: &SomeSelectors, parent: Selector) -> Result<()> {
        for any_selector in entries.as_slice() {
            let entry = any_selector.to_selector(self)?;
            self.push_edge(entry, parent)?;
        }
        Ok(())
    }

    /// Interpose a new pick between `target` (a reference positioned `on` its pick) and that
    /// pick: split the target's group at `boundary`, rest the split-off lower part on the new
    /// pick, and redirect the entering edges when they should now enter through it — always
    /// for an `At` split; for an `Above` split only when nothing moved up (the target was the
    /// group's top, so the whole group now rests on the new pick).
    fn interpose_pick_at_ref(
        &mut self,
        target: Selector,
        on: PickIndex,
        step: Step,
        boundary: SplitBoundary,
    ) -> Result<Selector> {
        let pick = self.resolved_pick(on)?;
        let Step::Pick(new_pick) = step else {
            bail!("BUG: interposing inserts picks only");
        };
        // Capture the target's entering edges BEFORE adding the new pick's edge, so the
        // fresh edge does not leak into `target_edges` (the redirect would then re-point
        // it onto the new pick itself, a self-loop).
        let target_edges = positions::edges_through(&self.graph, target.id);
        let target_ref = ref_entry(target.id)?;
        let new_idx = self.graph.add_node(Some(new_pick));
        self.graph.push_parent(new_idx, pick);
        let split = split_group(&mut self.graph, target_ref, boundary, new_idx);
        settle_group_lower(&mut self.graph, &split.lower, (new_idx, 0));
        if matches!(boundary, SplitBoundary::At) || !split.moved_any {
            // The group's edges now enter through the new pick, parent numbers untouched.
            redirect_edges(&mut self.graph, &target_edges, pick, new_idx);
        }
        Ok(self.new_selector(new_idx))
    }

    /// Append an edge from `child` to `parent` after `child`'s existing parents.
    ///
    /// Reference endpoints behave as in [`Self::insert_edge`].
    pub fn push_edge(&mut self, child: impl ToSelector, parent: impl ToSelector) -> Result<()> {
        self.insert_edge(child, parent, usize::MAX)
    }

    /// Insert an edge from `child` to `parent` at `parent number` among `child`'s ordered parents
    /// (clamped to the end); parents at `parent number` and later shift up, statements following.
    ///
    /// An edge FROM a reference positions it at the parent, never a raw edge (references
    /// are positions): a live reference re-points through the layout machinery; a dead one
    /// (which upstream-integration retention still redirects) just re-points its retained
    /// position — no group cascade, since its stored position is stale. An edge INTO a
    /// reference enters its group: the pick edge goes to the pick and the reference (with
    /// members below it) gains the new edge.
    pub fn insert_edge(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
        parent_number: usize,
    ) -> Result<()> {
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;
        self.debug_assert_acyclic(child.id, parent.id)?;

        if self.graph.state_of(child.id).is_some() {
            // An edge FROM a reference re-points it; a reference PARENT merely gains an
            // entering edge and may stay immutable.
            self.ensure_mutable_ref(child.id)?;
            let onto = match self.graph.positioned_on(parent.id) {
                Some(parent_on) => self.resolved_pick(parent_on)?,
                None => node_entry(parent.id)?,
            };
            let child_ref = ref_entry(child.id)?;
            if self.graph.is_reference(child_ref) {
                repoint_ref(&mut self.graph, child_ref, onto);
            } else {
                self.graph.set_retained_position(child_ref, onto);
            }
            return Ok(());
        }
        let parent_is_ref = self.graph.is_positioned(parent.id);
        let parent_pick = match self.graph.positioned_on(parent.id) {
            Some(on) => self.resolved_pick(on)?,
            None => node_entry(parent.id)?,
        };
        let child_node = node_entry(child.id)?;
        let parent_number = self
            .graph
            .insert_parent(child_node, parent_number, parent_pick);
        // The group is captured AFTER the insert: normalization and the shift rename the
        // child's statements, so a pre-capture would hold stale edge names.
        if parent_is_ref {
            let join = positions::prepare_group_join(&self.graph, ref_entry(parent.id)?);
            positions::apply_group_join(&mut self.graph, &join, (child_node, parent_number));
        }
        Ok(())
    }

    fn debug_assert_acyclic(&self, child: EditorIndex, parent: EditorIndex) -> Result<()> {
        if cfg!(debug_assertions) {
            let mut seen = HashSet::from([parent]);
            let mut tips = vec![parent];

            while let Some(tip) = tips.pop() {
                for parent in self.graph.parents(tip) {
                    if seen.insert(parent.into()) {
                        tips.push(parent.into());
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
    /// parent numbers they occupied. Later parents shift down, statements following.
    pub fn remove_edges(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
    ) -> Result<Vec<usize>> {
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;

        // A reference child holds one conceptual downward edge (order 0) — its pick. It is
        // reported but not cleared; a follow-up insert_edge re-points, and a position without
        // a resolving pick is not representable.
        if self.graph.is_positioned(child.id) {
            let resolves_to_parent = positions::resolve_to_pick(&self.graph, child.id)
                == positions::resolve_to_pick(&self.graph, parent.id);
            return Ok(if resolves_to_parent { vec![0] } else { vec![] });
        }
        let numbers = match self.graph.positioned_on(parent.id) {
            // Disconnecting from a reference removes the edges carrying its group.
            Some(on) => {
                let target_pick = self.resolved_pick(on)?;
                let parent_edges = positions::edges_through(&self.graph, parent.id);
                let child_node = node_entry(child.id)?;
                self.graph
                    .parents(child_node)
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(parent_number, target)| {
                        (target == target_pick
                            && parent_edges.contains(&(child_node, parent_number)))
                        .then_some(parent_number)
                    })
                    .collect::<Vec<_>>()
            }
            // Disconnecting from a pick removes its parent numbers; groups riding a removed edge lose
            // it from their entering edges below.
            None => self
                .graph
                .parents(child.id)
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(parent_number, target)| {
                    (EditorIndex::from(target) == parent.id).then_some(parent_number)
                })
                .collect::<Vec<_>>(),
        };

        // Highest-first so earlier parent numbers keep their names; report the pre-removal parent numbers.
        let child_node = node_entry(child.id)?;
        for parent_number in numbers.iter().rev() {
            self.graph
                .remove_parent(child_node, *parent_number)
                .context("BUG: Failed to remove parent")?;
        }

        Ok(numbers)
    }
}
