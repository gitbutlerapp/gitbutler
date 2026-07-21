//! Operations for mutating the graph

use std::collections::HashSet;

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::{
    NodeIndex, NodeKind,
    edit::{MutableNodeGraph, Pick, ToCommitSelector, ToReferenceSelector, ToSelector},
};

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
    /// A node index into the graph.
    Node(NodeIndex),
    /// A commit id that should resolve to a pick step.
    Commit(gix::ObjectId),
    /// A reference name that should resolve to a reference step.
    Reference(gix::refs::FullName),
}

impl ToSelector for AnySelector {
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        match self {
            Self::Node(index) => index.to_selector(graph),
            Self::Commit(id) => graph.select_commit(*id),
            Self::Reference(reference) => graph.select_reference(reference.as_ref()),
        }
    }
}

impl From<NodeIndex> for AnySelector {
    fn from(value: NodeIndex) -> Self {
        Self::Node(value)
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
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        match self {
            Self::Commit(id) => graph.select_commit(*id),
            Self::Reference(reference) => graph.select_reference(reference),
        }
    }
}

/// Specifies a location relative to which a commit operation should occur.
/// This is the fully-owned cousin of [RelativeToRef].
#[derive(Debug, Clone)]
pub enum RelativeTo {
    /// Relative to a commit.
    Commit(gix::ObjectId),
    /// Relative to a reference.
    Reference(gix::refs::FullName),
}

impl ToSelector for RelativeTo {
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        match self {
            Self::Commit(commit) => graph.select_commit(*commit),
            Self::Reference(reference) => graph.select_reference(reference.as_ref()),
        }
    }
}

impl ToCommitSelector for gix::ObjectId {
    fn to_commit_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_commit(*self)
    }
}

impl ToCommitSelector for gix::Id<'_> {
    fn to_commit_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_commit(self.detach())
    }
}

impl ToSelector for gix::ObjectId {
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_commit(*self)
    }
}

impl ToSelector for gix::Id<'_> {
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_commit(self.detach())
    }
}

impl ToReferenceSelector for &gix::refs::FullNameRef {
    fn to_reference_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_reference(self)
    }
}

impl ToReferenceSelector for gix::refs::FullName {
    fn to_reference_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_reference(self.as_ref())
    }
}

impl ToSelector for &gix::refs::FullNameRef {
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_reference(self)
    }
}

impl ToSelector for gix::refs::FullName {
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        graph.select_reference(self.as_ref())
    }
}

/// Operations for mutating the commit graph
impl MutableNodeGraph {
    /// Get the node index of a particular commit in the graph
    pub fn select_commit(&self, target: gix::ObjectId) -> Result<NodeIndex> {
        match self.try_select_commit(target) {
            Some(index) => Ok(index),
            None => Err(anyhow!("Failed to find commit {target} in graph")),
        }
    }

    /// Get the node index of a particular reference in the graph
    pub fn select_reference(&self, target: &gix::refs::FullNameRef) -> Result<NodeIndex> {
        match self.try_select_reference(target) {
            Some(index) => Ok(index),
            None => Err(anyhow!("Failed to find reference {target} in graph")),
        }
    }

    /// Get the node index of a particular commit in the graph
    ///
    /// Convergence boundaries are addressable commits and can be selected.
    pub fn try_select_commit(&self, target: gix::ObjectId) -> Option<NodeIndex> {
        self.indices()
            .find(|node_idx| self.nodes[*node_idx].kind().addressable_commit_id() == Some(target))
    }

    /// Get the node index of a particular reference in the graph
    pub fn try_select_reference(&self, target: &gix::refs::FullNameRef) -> Option<NodeIndex> {
        self.indices().find(|node_idx| {
            matches!(self.nodes[*node_idx].kind(), NodeKind::Reference(reference) if target == reference.ref_info.ref_name.as_ref())
        })
    }

    /// Returns all direct children of `target` together with their edge order.
    ///
    /// Children are represented as incoming edges into `target` in the graph.
    pub fn direct_children(&self, target: impl ToSelector) -> Result<Vec<(NodeIndex, usize)>> {
        let target = target.to_selector(self)?;
        Ok(self.children(target))
    }

    /// Returns all direct parents of `target` together with their edge order.
    ///
    /// Parents are represented as parent-slot positions in the graph.
    pub fn direct_parents(&self, target: impl ToSelector) -> Result<Vec<(NodeIndex, usize)>> {
        let target = target.to_selector(self)?;
        Ok(self
            .parents(target)
            .iter()
            .enumerate()
            .map(|(slot, parent)| (*parent, slot))
            .collect())
    }

    /// For a given step, find all the references that point to it.
    ///
    /// The reference indexes are provided in no particular order.
    pub fn step_references(&self, target: impl ToSelector) -> Result<Vec<NodeIndex>> {
        let target = target.to_selector(self)?;

        let mut references = vec![];
        let mut seen = HashSet::new();
        let mut tips = vec![target];

        while let Some(tip) = tips.pop() {
            for (child, _slot) in self.children(tip) {
                if !seen.insert(child) {
                    continue;
                }

                match self.nodes[child].kind() {
                    // Tombstones (and unavailable history) are transparent.
                    NodeKind::None
                    | NodeKind::Boundary {
                        reason: crate::BoundaryKind::Shallow,
                        ..
                    } => tips.push(child),
                    NodeKind::Reference(_) => {
                        references.push(child);
                        tips.push(child);
                    }
                    NodeKind::Commit { .. }
                    | NodeKind::Boundary {
                        reason: crate::BoundaryKind::Convergence,
                        ..
                    } => {}
                }
            }
        }

        Ok(references)
    }

    /// Replaces the node that the selector points to with a commit holding
    /// `pick`.
    ///
    /// If a commit has been replaced with another commit, the commit mappings
    /// will get updated to include an entry going from the old to the new
    /// object id.
    pub fn replace_commit(&mut self, target: impl ToSelector, pick: Pick) -> Result<()> {
        let target = target.to_selector(self)?;
        if let Some(from) = self.pick_at(target)
            && !from.exclude_from_tracking
            && !pick.exclude_from_tracking
        {
            self.session.commit_mappings.update(from.id, pick.id);
        };
        self.install_pick(target, pick);
        Ok(())
    }

    /// Replaces the node that the selector points to with a mutable reference
    /// named `refname`.
    ///
    /// Replacing a reference with a reference of the same name keeps the
    /// node's discovered reference information.
    pub fn replace_with_reference(
        &mut self,
        target: impl ToSelector,
        refname: gix::refs::FullName,
    ) -> Result<()> {
        let target = target.to_selector(self)?;
        self.install_reference(target, refname);
        Ok(())
    }

    /// Removes the node that the selector points to, leaving a tombstone
    /// ([`NodeKind::None`]) so node indexes stay stable.
    pub fn remove(&mut self, target: impl ToSelector) -> Result<()> {
        let target = target.to_selector(self)?;
        self.install_none(target);
        Ok(())
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
        let target_child = child.to_selector(self)?;
        let target_parent = parent.to_selector(self)?;
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

        // Child edges pointing at the segment's child-most node.
        let incoming_edges = self.children(target_child);

        // The segment's parent-most node's parents.
        let outgoing_parents = self.parents(target_parent).to_vec();

        // All available parents
        let available_parents = outgoing_parents.iter().copied().collect::<HashSet<_>>();
        let available_children = incoming_edges
            .iter()
            .map(|(child, _)| *child)
            .collect::<HashSet<_>>();

        // 1. Verify that all parents and children to disconnect are directly connected to the target segment.
        if let Some(parents_to_disconnect) = parents_to_disconnect.as_ref() {
            for index in parents_to_disconnect {
                if !available_parents.contains(index) {
                    return Err(anyhow!(
                        "Invalid parent delimitation: requested parent is not a direct parent of target.parent"
                    ));
                }
            }
        }

        if let Some(children_to_disconnect) = children_to_disconnect.as_ref() {
            for index in children_to_disconnect {
                if !available_children.contains(index) {
                    return Err(anyhow!(
                        "Invalid parent delimitation: requested child is not a direct parent of target.child"
                    ));
                }
            }
        }

        let parent_ids_to_disconnect = parents_to_disconnect
            .as_ref()
            .map(|parents| parents.iter().copied().collect::<HashSet<_>>());
        let child_ids_to_disconnect = children_to_disconnect
            .as_ref()
            .map(|children| children.iter().copied().collect::<HashSet<_>>());

        // 2. Disconnect parents, keeping the disconnected ones in slot order.
        let mut disconnected_parents = Vec::new();
        {
            let parents = self.parents_mut(target_parent);
            let mut kept = Vec::with_capacity(parents.len());
            for parent in parents.iter().copied() {
                let should_disconnect = parent_ids_to_disconnect
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&parent));
                if should_disconnect {
                    disconnected_parents.push(parent);
                } else {
                    kept.push(parent);
                }
            }
            *parents = kept;
        }

        // 3. Disconnect children, splicing the disconnected parents into the
        // slot the removed edge occupied.
        let mut slots_by_child = std::collections::BTreeMap::<_, Vec<usize>>::new();
        for (child, slot) in incoming_edges {
            let should_disconnect = child_ids_to_disconnect
                .as_ref()
                .is_none_or(|ids| ids.contains(&child));
            if should_disconnect {
                slots_by_child.entry(child).or_default().push(slot);
            }
        }
        for (child, mut slots) in slots_by_child {
            slots.sort_unstable();
            // A reference stands on a single commit, so it follows the first
            // (git first-parent) disconnected parent; commit children adopt
            // every disconnected parent in order.
            let reconnect_parents = if matches!(self.nodes[child].kind(), NodeKind::Reference(_)) {
                &disconnected_parents[..disconnected_parents.len().min(1)]
            } else {
                &disconnected_parents[..]
            };
            // Remove by value: when the delimiter's parent is itself a child of
            // the delimiter's child, step 2 already dropped the shared edge and
            // the recorded slots are stale.
            let parents = self.parents_mut(child);
            let first_slot = parents
                .iter()
                .position(|parent| *parent == target_child)
                .unwrap_or_else(|| slots[0].min(parents.len()));
            parents.retain(|parent| *parent != target_child);
            if !skip_reconnect_step {
                for (offset, parent) in reconnect_parents.iter().enumerate() {
                    let position = (first_slot + offset).min(parents.len());
                    parents.insert(position, *parent);
                }
            }
        }

        Ok(())
    }

    fn add_edges_to_parents(
        &mut self,
        child_node: NodeIndex,
        new_parent_nodes: impl IntoIterator<Item = NodeIndex>,
        parent_reparenting_order: ParentReparentingOrder,
    ) {
        let parents = self.parents_mut(child_node);
        let existing = std::mem::take(parents);
        let new_parent_nodes = new_parent_nodes.into_iter().collect::<Vec<_>>();
        let combined: Vec<_> = match parent_reparenting_order {
            ParentReparentingOrder::Prepend => {
                new_parent_nodes.into_iter().chain(existing).collect()
            }
            ParentReparentingOrder::Append => {
                existing.into_iter().chain(new_parent_nodes).collect()
            }
        };
        let mut seen = HashSet::new();
        *self.parents_mut(child_node) = combined
            .into_iter()
            .filter(|parent| seen.insert(*parent))
            .collect();
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

        match side {
            InsertSide::Above => {
                if let Some(nodes_to_connect) = nodes_to_connect {
                    // If there were nodes to connect defined, create edges from them into the child node of the segment
                    // being inserted.
                    for any_selector in nodes_to_connect.as_slice() {
                        let node = any_selector.to_selector(self)?;
                        self.parents_mut(node).push(child);
                    }
                } else {
                    // Repoint all target's child slots at the child-most node in
                    // the given segment, keeping each child's parent order.
                    for (child_node, slot) in self.children(target) {
                        self.parents_mut(child_node)[slot] = child;
                    }
                }

                // Connect the target to the parent-most node in the given segment according to
                // the requested parent ordering policy.
                self.add_edges_to_parents(parent, [target], parent_reparenting_order);
            }
            InsertSide::Below => {
                let parents_to_add = if let Some(nodes_to_connect) = nodes_to_connect {
                    let mut nodes = Vec::new();
                    for any_selector in nodes_to_connect.as_slice() {
                        nodes.push(any_selector.to_selector(self)?);
                    }
                    nodes
                } else {
                    std::mem::take(self.parents_mut(target))
                };

                self.add_edges_to_parents(parent, parents_to_add, parent_reparenting_order);

                // Connect the target to the child-most node in the given segment.
                self.parents_mut(target).push(child);
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

    /// Inserts a new commit node with default pick policy relative to a
    /// selector.
    ///
    /// When inserting above, any nodes that point to the selector will now
    /// point to the inserted node instead. When inserting below, any nodes
    /// that the selector points to will now be pointed to by the inserted node
    /// instead.
    ///
    /// Returns the index of the inserted node
    pub fn insert_commit(
        &mut self,
        target: impl ToSelector,
        id: gix::ObjectId,
        side: InsertSide,
    ) -> Result<NodeIndex> {
        self.insert_commit_with(target, Pick::new_pick(id), side)
    }

    /// Inserts a new commit node holding `pick` relative to a selector.
    ///
    /// See [`Self::insert_commit`] for the insertion semantics.
    pub fn insert_commit_with(
        &mut self,
        target: impl ToSelector,
        pick: Pick,
        side: InsertSide,
    ) -> Result<NodeIndex> {
        let target = target.to_selector(self)?;
        let new_idx = self.add_commit(pick);
        Ok(self.insert_appended(target, new_idx, side))
    }

    /// Inserts a new mutable reference node relative to a selector.
    ///
    /// See [`Self::insert_commit`] for the insertion semantics.
    pub fn insert_reference(
        &mut self,
        target: impl ToSelector,
        refname: gix::refs::FullName,
        side: InsertSide,
    ) -> Result<NodeIndex> {
        let target = target.to_selector(self)?;
        let new_idx = self.add_reference(refname);
        Ok(self.insert_appended(target, new_idx, side))
    }

    /// Wire the freshly appended, disconnected node `new_idx` into the graph
    /// relative to `target`.
    fn insert_appended(
        &mut self,
        target: NodeIndex,
        new_idx: NodeIndex,
        side: InsertSide,
    ) -> NodeIndex {
        match side {
            InsertSide::Above => {
                for (child_node, slot) in self.children(target) {
                    if child_node == new_idx {
                        continue;
                    }
                    self.parents_mut(child_node)[slot] = new_idx;
                }
                *self.parents_mut(new_idx) = vec![target];
            }
            InsertSide::Below => {
                // A reference stands on its target (its last parent); any other
                // parent slots (a workspace ref's stack overlays) are annotations
                // that stay on the reference.
                let is_reference = matches!(self.nodes[target].kind(), NodeKind::Reference(_));
                let parents = self.parents_mut(target);
                if is_reference {
                    if let Some(slot) = parents.last_mut() {
                        let moved = *slot;
                        *slot = new_idx;
                        *self.parents_mut(new_idx) = vec![moved];
                    } else {
                        parents.push(new_idx);
                    }
                } else {
                    let moved = std::mem::replace(parents, vec![new_idx]);
                    *self.parents_mut(new_idx) = moved;
                }
            }
        }
        new_idx
    }

    /// Add an edge to the graph at the desired parent slot.
    ///
    /// The parent is inserted at `desired_order` (clamped to the number of
    /// existing parents), shifting later slots. Introducing a cycle is an
    /// error.
    pub fn add_edge(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
        desired_order: usize,
    ) -> Result<()> {
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;

        // Cycles would otherwise only surface at rebase time; the walk here is
        // cheap relative to the object writes an edit performs anyway.
        let mut seen = HashSet::from([parent]);
        let mut tips = vec![parent];
        while let Some(tip) = tips.pop() {
            for parent in self.parents(tip).to_vec() {
                if seen.insert(parent) {
                    tips.push(parent);
                }
            }
        }
        if seen.contains(&child) {
            bail!("BUG: Add edge introduces a cycle");
        }

        let parents = self.parents_mut(child);
        let position = desired_order.min(parents.len());
        parents.insert(position, parent);

        Ok(())
    }

    /// Removes all edges between a child and parent, returning the orders of the removed edges.
    pub fn remove_edges(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
    ) -> Result<Vec<usize>> {
        let child = child.to_selector(self)?;
        let parent = parent.to_selector(self)?;

        let parents = self.parents_mut(child);
        let mut orders = vec![];
        let mut slot = 0;
        parents.retain(|candidate| {
            let keep = *candidate != parent;
            if !keep {
                orders.push(slot);
            }
            slot += 1;
            keep
        });

        Ok(orders)
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
