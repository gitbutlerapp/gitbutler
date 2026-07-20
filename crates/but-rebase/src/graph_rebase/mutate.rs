//! Operations for mutating the editor

use std::collections::HashSet;

use anyhow::{Result, anyhow, bail};
use but_core::RefMetadata;
use serde::{Deserialize, Serialize};

use crate::graph_rebase::{
    Editor, Pick, Selector, Step, ToCommitSelector, ToReferenceSelector, ToSelector,
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
        for node_idx in self.graph.indices() {
            if let Step::Pick(Pick { id, .. }) = self.graph.step(node_idx)
                && id == target
            {
                return Some(self.new_selector(node_idx));
            }
        }

        None
    }

    /// Get a selector to a particular reference in the graph
    pub fn try_select_reference(&self, target: &gix::refs::FullNameRef) -> Option<Selector> {
        for node_idx in self.graph.indices() {
            if let Step::Reference { refname, .. } = self.graph.step(node_idx)
                && target == refname.as_ref()
            {
                return Some(self.new_selector(node_idx));
            }
        }

        None
    }

    /// Returns all direct children of `target` together with their edge order.
    ///
    /// Children are represented as incoming edges into `target` in the step graph.
    pub fn direct_children(&self, target: impl ToSelector) -> Result<Vec<(Selector, usize)>> {
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        Ok(self
            .graph
            .children(target.id)
            .into_iter()
            .map(|(child, slot)| (self.new_selector(child), slot))
            .collect())
    }

    /// Returns all direct parents of `target` together with their edge order.
    ///
    /// Parents are represented as parent-slot positions in the step graph.
    pub fn direct_parents(&self, target: impl ToSelector) -> Result<Vec<(Selector, usize)>> {
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        Ok(self
            .graph
            .parents(target.id)
            .iter()
            .enumerate()
            .map(|(slot, parent)| (self.new_selector(*parent), slot))
            .collect())
    }

    /// For a given step, find all the references that point to it.
    ///
    /// The reference selectors are provided in no particular order.
    pub fn step_references(&self, target: impl ToSelector) -> Result<Vec<Selector>> {
        let target = self.history.normalize_selector(target.to_selector(self)?)?;

        let mut references = vec![];
        let mut seen = HashSet::new();
        let mut tips = vec![target.id];

        while let Some(tip) = tips.pop() {
            for (child, _slot) in self.graph.children(tip) {
                if !seen.insert(child) {
                    continue;
                }

                match self.graph.step(child) {
                    Step::None => tips.push(child),
                    Step::Reference { .. } => {
                        references.push(self.new_selector(child));
                        tips.push(child);
                    }
                    Step::Pick(_) => {}
                }
            }
        }

        Ok(references)
    }

    /// Replaces the node that the function was pointing to.
    ///
    /// If a commit step has been replaced with another commit step, the commit
    /// mappings will get updated to include an entry going from the old to the
    /// new object id.
    ///
    /// Returns the replaced step.
    pub fn replace(&mut self, target: impl ToSelector, step: Step) -> Result<Step> {
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        if let (Step::Pick(from), Step::Pick(to)) = (self.graph.step(target.id), &step)
            && !from.exclude_from_tracking
            && !to.exclude_from_tracking
        {
            self.history.update_mapping(from.id, to.id);
        };
        Ok(self.graph.set_step(target.id, step))
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
        let target_child = self.history.normalize_selector(child.to_selector(self)?)?;
        let target_parent = self.history.normalize_selector(parent.to_selector(self)?)?;
        let children_to_disconnect = match children_to_disconnect {
            SelectorSet::All => None,
            SelectorSet::None => Some(Vec::new()),
            SelectorSet::Some(children) => Some(
                children
                    .as_slice()
                    .iter()
                    .map(|from_child| from_child.to_selector(self))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .map(|selector| self.history.normalize_selector(selector))
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
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .map(|selector| self.history.normalize_selector(selector))
                    .collect::<Result<Vec<_>>>()?,
            ),
        };

        // Child edges pointing at the segment's child-most node.
        let incoming_edges = self.graph.children(target_child.id);

        // The segment's parent-most node's parents.
        let outgoing_parents = self.graph.parents(target_parent.id).to_vec();

        // All available parents
        let available_parents = outgoing_parents.iter().copied().collect::<HashSet<_>>();
        let available_children = incoming_edges
            .iter()
            .map(|(child, _)| *child)
            .collect::<HashSet<_>>();

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

        // 2. Disconnect parents, keeping the disconnected ones in slot order.
        let mut disconnected_parents = Vec::new();
        {
            let parents = self.graph.parents_mut(target_parent.id);
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
            let reconnect_parents = if matches!(self.graph.step(child), Step::Reference { .. }) {
                &disconnected_parents[..disconnected_parents.len().min(1)]
            } else {
                &disconnected_parents[..]
            };
            // Remove by value: when the delimiter's parent is itself a child of
            // the delimiter's child, step 2 already dropped the shared edge and
            // the recorded slots are stale.
            let parents = self.graph.parents_mut(child);
            let first_slot = parents
                .iter()
                .position(|parent| *parent == target_child.id)
                .unwrap_or_else(|| slots[0].min(parents.len()));
            parents.retain(|parent| *parent != target_child.id);
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
        child_node: crate::graph_rebase::StepGraphIndex,
        new_parent_nodes: impl IntoIterator<Item = crate::graph_rebase::StepGraphIndex>,
        parent_reparenting_order: ParentReparentingOrder,
    ) {
        let parents = self.graph.parents_mut(child_node);
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
        *parents = combined
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
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        let child = self.history.normalize_selector(child.to_selector(self)?)?;
        let parent = self.history.normalize_selector(parent.to_selector(self)?)?;

        match side {
            InsertSide::Above => {
                if let Some(nodes_to_connect) = nodes_to_connect {
                    // If there were nodes to connect defined, create edges from them into the child node of the segment
                    // being inserted.
                    for any_selector in nodes_to_connect.as_slice() {
                        let selector = any_selector.to_selector(self)?;
                        let node = self.history.normalize_selector(selector)?;
                        self.graph.parents_mut(node.id).push(child.id);
                    }
                } else {
                    // Repoint all target's child slots at the child-most node in
                    // the given segment, keeping each child's parent order.
                    for (child_node, slot) in self.graph.children(target.id) {
                        self.graph.parents_mut(child_node)[slot] = child.id;
                    }
                }

                // Connect the target to the parent-most node in the given segment according to
                // the requested parent ordering policy.
                self.add_edges_to_parents(parent.id, [target.id], parent_reparenting_order);
            }
            InsertSide::Below => {
                let parents_to_add = if let Some(nodes_to_connect) = nodes_to_connect {
                    let mut nodes = Vec::new();
                    for any_selector in nodes_to_connect.as_slice() {
                        let selector = any_selector.to_selector(self)?;
                        let node = self.history.normalize_selector(selector)?;
                        nodes.push(node.id);
                    }
                    nodes
                } else {
                    std::mem::take(self.graph.parents_mut(target.id))
                };

                self.add_edges_to_parents(parent.id, parents_to_add, parent_reparenting_order);

                // Connect the target to the child-most node in the given segment.
                self.graph.parents_mut(target.id).push(child.id);
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
        let new_idx = self.graph.add_node(step);
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
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        match side {
            InsertSide::Above => {
                let new_idx = self.graph.add_node(step);
                for (child_node, slot) in self.graph.children(target.id) {
                    self.graph.parents_mut(child_node)[slot] = new_idx;
                }
                *self.graph.parents_mut(new_idx) = vec![target.id];

                Ok(self.new_selector(new_idx))
            }
            InsertSide::Below => {
                let new_idx = self.graph.add_node(step);
                // A reference stands on its target (its last parent); any other
                // parent slots (a workspace ref's stack overlays) are annotations
                // that stay on the reference.
                let is_reference =
                    matches!(self.graph.step(target.id), Step::Reference { .. });
                let parents = self.graph.parents_mut(target.id);
                if is_reference {
                    if let Some(slot) = parents.last_mut() {
                        let moved = *slot;
                        *slot = new_idx;
                        *self.graph.parents_mut(new_idx) = vec![moved];
                    } else {
                        parents.push(new_idx);
                    }
                } else {
                    let moved = std::mem::replace(parents, vec![new_idx]);
                    *self.graph.parents_mut(new_idx) = moved;
                }

                Ok(self.new_selector(new_idx))
            }
        }
    }

    /// Add an edge to the graph at the desired parent slot.
    ///
    /// The parent is inserted at `desired_order` (clamped to the number of
    /// existing parents), shifting later slots.
    pub fn add_edge(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
        desired_order: usize,
    ) -> Result<()> {
        let child = self.history.normalize_selector(child.to_selector(self)?)?;
        let parent = self.history.normalize_selector(parent.to_selector(self)?)?;

        if cfg!(debug_assertions) {
            let mut seen = HashSet::from([parent.id]);
            let mut tips = vec![parent.id];

            while let Some(tip) = tips.pop() {
                for parent in self.graph.parents(tip).to_vec() {
                    if seen.insert(parent) {
                        tips.push(parent);
                    }
                }
            }

            if seen.contains(&child.id) {
                bail!("BUG: Add edge introduces a cycle");
            }
        }

        let parents = self.graph.parents_mut(child.id);
        let position = desired_order.min(parents.len());
        parents.insert(position, parent.id);

        Ok(())
    }

    /// Removes all edges between a child and parent, returning the orders of the removed edges.
    pub fn remove_edges(
        &mut self,
        child: impl ToSelector,
        parent: impl ToSelector,
    ) -> Result<Vec<usize>> {
        let child = self.history.normalize_selector(child.to_selector(self)?)?;
        let parent = self.history.normalize_selector(parent.to_selector(self)?)?;

        let parents = self.graph.parents_mut(child.id);
        let mut orders = vec![];
        let mut slot = 0;
        parents.retain(|candidate| {
            let keep = *candidate != parent.id;
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
