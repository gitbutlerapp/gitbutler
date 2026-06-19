//! Operations for mutating the editor

use std::collections::HashSet;

use anyhow::{Context as _, Result, anyhow, bail};
use but_core::RefMetadata;
use petgraph::{Direction, visit::EdgeRef};
use serde::{Deserialize, Serialize};

use crate::graph_rebase::{
    Edge, Editor, Pick, Selector, Step, ToCommitSelector, ToReferenceSelector, ToSelector,
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
        for node_idx in self.graph.node_indices() {
            if let Step::Pick(Pick { id, .. }) = self.graph[node_idx]
                && id == target
            {
                return Some(self.new_selector(node_idx));
            }
        }

        None
    }

    /// Get a selector to a particular reference in the graph
    pub fn try_select_reference(&self, target: &gix::refs::FullNameRef) -> Option<Selector> {
        for node_idx in self.graph.node_indices() {
            if let Step::Reference { refname, .. } = &self.graph[node_idx]
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
            .edges_directed(target.id, Direction::Incoming)
            .map(|edge| (self.new_selector(edge.source()), edge.weight().order))
            .collect())
    }

    /// Returns all direct parents of `target` together with their edge order.
    ///
    /// Parents are represented as outgoing edges from `target` in the step graph.
    pub fn direct_parents(&self, target: impl ToSelector) -> Result<Vec<(Selector, usize)>> {
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        Ok(self
            .graph
            .edges_directed(target.id, Direction::Outgoing)
            .map(|edge| (self.new_selector(edge.target()), edge.weight().order))
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
            for edge in self.graph.edges_directed(tip, Direction::Incoming) {
                let child = edge.source();
                if !seen.insert(child) {
                    continue;
                }

                match &self.graph[child] {
                    Step::None => tips.push(child),
                    Step::Reference { .. } => {
                        references.push(self.new_selector(child));
                        tips.push(child);
                    }
                    _ => {}
                }
            }
        }

        Ok(references)
    }

    /// Replaces the node that the function was pointing to.
    ///
    /// Returns the replaced step.
    pub fn replace(&mut self, target: impl ToSelector, mut step: Step) -> Result<Step> {
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        std::mem::swap(&mut self.graph[target.id], &mut step);
        Ok(step)
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

        // Edges to children.
        let incoming_edges = self
            .graph
            .edges_directed(target_child.id, Direction::Incoming)
            .map(|e| (e.id(), e.weight().to_owned(), e.source()))
            .collect::<Vec<_>>();

        // Edges to parents.
        let outgoing_edges = self
            .graph
            .edges_directed(target_parent.id, Direction::Outgoing)
            .map(|e| (e.id(), e.weight().to_owned(), e.target()))
            .collect::<Vec<_>>();

        // All available parents
        let available_parents = outgoing_edges
            .iter()
            .map(|(_, _, edge_target)| *edge_target)
            .collect::<HashSet<_>>();
        let available_children = incoming_edges
            .iter()
            .map(|(_, _, edge_source)| *edge_source)
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

        let mut disconnected_parent_edges = Vec::new();
        // 2. Disconnect parents.
        for (edge_id, edge_weight, edge_target) in outgoing_edges {
            let should_disconnect = parent_ids_to_disconnect
                .as_ref()
                .is_none_or(|ids| ids.contains(&edge_target));
            if should_disconnect {
                self.graph.remove_edge(edge_id);
                disconnected_parent_edges.push((edge_weight, edge_target));
            }
        }

        // 3. Disconnect children and reconnect to the disconnected parents.
        for (edge_id, _, edge_source) in incoming_edges {
            let should_disconnect = child_ids_to_disconnect
                .as_ref()
                .is_none_or(|ids| ids.contains(&edge_source));
            if should_disconnect {
                // Remove the child edge.
                self.graph.remove_edge(edge_id);
                // Reconnect the child node to all the disconnected parents.
                if !skip_reconnect_step {
                    self.reconnect_edges_to_parents(&disconnected_parent_edges, edge_source);
                }
            }
        }

        Ok(())
    }

    /// Remove the child edge, and reconnect to the right parents.
    fn reconnect_edges_to_parents(
        &mut self,
        disconnected_parent_edges: &[(Edge, petgraph::prelude::NodeIndex)],
        child_node: petgraph::prelude::NodeIndex,
    ) {
        // Reconnect the child node to all the disconnected parents.
        for (parent_edge_weight, edge_target) in disconnected_parent_edges {
            self.graph
                .add_edge(child_node, *edge_target, parent_edge_weight.clone());
        }
    }

    fn add_edges_to_parents(
        &mut self,
        child_node: petgraph::prelude::NodeIndex,
        new_parent_nodes: impl IntoIterator<Item = petgraph::prelude::NodeIndex>,
        parent_reparenting_order: ParentReparentingOrder,
    ) {
        let mut existing_parent_edges = self
            .graph
            .edges_directed(child_node, Direction::Outgoing)
            .map(|edge| (edge.id(), edge.weight().order, edge.target()))
            .collect::<Vec<_>>();
        existing_parent_edges.sort_by_key(|(_, order, _)| *order);

        for (edge_id, _, _) in &existing_parent_edges {
            self.graph.remove_edge(*edge_id);
        }

        let new_parent_nodes = new_parent_nodes.into_iter().collect::<Vec<_>>();
        match parent_reparenting_order {
            ParentReparentingOrder::Prepend => {
                for (order, parent_node) in new_parent_nodes.iter().enumerate() {
                    self.graph
                        .add_edge(child_node, *parent_node, Edge { order });
                }

                // Insertion-location parents define the first-parent lane. Existing parents stay
                // attached after them as merge-side parents.
                let shifted_by = new_parent_nodes.len();
                for (offset, (_, _, parent_node)) in existing_parent_edges.into_iter().enumerate() {
                    self.graph.add_edge(
                        child_node,
                        parent_node,
                        Edge {
                            order: shifted_by + offset,
                        },
                    );
                }
            }
            ParentReparentingOrder::Append => {
                let shifted_by = existing_parent_edges.len();
                for (order, (_, _, parent_node)) in existing_parent_edges.into_iter().enumerate() {
                    self.graph.add_edge(child_node, parent_node, Edge { order });
                }

                for (offset, parent_node) in new_parent_nodes.into_iter().enumerate() {
                    self.graph.add_edge(
                        child_node,
                        parent_node,
                        Edge {
                            order: shifted_by + offset,
                        },
                    );
                }
            }
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
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        let child = self.history.normalize_selector(child.to_selector(self)?)?;
        let parent = self.history.normalize_selector(parent.to_selector(self)?)?;

        match side {
            InsertSide::Above => {
                // Find the child node of the highest order from the child-most node in the segment being inserted.
                let chubbiest_grand_child = self
                    .graph
                    .edges_directed(child.id, Direction::Incoming)
                    .map(|e| (e.id(), e.weight().to_owned(), e.source()))
                    .max_by_key(|gc| gc.1.order);

                if let Some(nodes_to_connect) = nodes_to_connect {
                    // If there were nodes to connect defined, create edges from them into the child node of the segment
                    // being inserted.
                    for (index, any_selector) in nodes_to_connect.as_slice().iter().enumerate() {
                        let selector = any_selector.to_selector(self)?;
                        let node = self.history.normalize_selector(selector)?;
                        // Avoid weight collision by adding the order value of the highest order child plus one,
                        // accommodating for order 0.
                        let new_weight = if let Some((_, grand_child_weight, _)) =
                            chubbiest_grand_child.as_ref()
                        {
                            Edge {
                                order: index + grand_child_weight.order + 1,
                            }
                        } else {
                            Edge { order: index }
                        };
                        self.graph.add_edge(node.id, child.id, new_weight);
                    }
                } else {
                    let edges = self
                        .graph
                        .edges_directed(target.id, Direction::Incoming)
                        .map(|e| (e.id(), e.weight().to_owned(), e.source()))
                        .collect::<Vec<_>>();

                    // Connect all target's children with the child-most node in the given segment.
                    for (edge_id, edge_weight, edge_source) in edges {
                        self.graph.remove_edge(edge_id);
                        // Avoid weight collision by adding the order value of the highest order child plus one,
                        // accommodating for order 0.
                        let new_weight = if let Some((_, grand_child_weight, _)) =
                            chubbiest_grand_child.as_ref()
                        {
                            Edge {
                                order: edge_weight.order + grand_child_weight.order + 1,
                            }
                        } else {
                            edge_weight
                        };
                        self.graph.add_edge(edge_source, child.id, new_weight);
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
                    let mut edges = self
                        .graph
                        .edges_directed(target.id, Direction::Outgoing)
                        .map(|e| (e.id(), e.weight().order, e.target()))
                        .collect::<Vec<_>>();
                    edges.sort_by_key(|(_, order, _)| *order);

                    let mut nodes = Vec::with_capacity(edges.len());
                    for (edge_id, _, edge_target) in edges {
                        self.graph.remove_edge(edge_id);
                        nodes.push(edge_target);
                    }
                    nodes
                };

                self.add_edges_to_parents(parent.id, parents_to_add, parent_reparenting_order);

                // Find the child node of the highest order from the child-most node in the segment being inserted.
                let chubbiest_grand_child = self
                    .graph
                    .edges_directed(child.id, Direction::Incoming)
                    .map(|e| (e.id(), e.weight().to_owned(), e.source()))
                    .max_by_key(|gc| gc.1.order);

                let new_weight =
                    if let Some((_, grand_child_weight, _)) = chubbiest_grand_child.as_ref() {
                        Edge {
                            order: grand_child_weight.order + 1,
                        }
                    } else {
                        Edge { order: 0 }
                    };
                // Connect the target to the child-most node in the given segment.
                self.graph.add_edge(target.id, child.id, new_weight);
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
                let edges = self
                    .graph
                    .edges_directed(target.id, Direction::Incoming)
                    .map(|e| (e.id(), e.weight().to_owned(), e.source()))
                    .collect::<Vec<_>>();

                let new_idx = self.graph.add_node(step);
                self.graph.add_edge(new_idx, target.id, Edge { order: 0 });

                for (edge_id, edge_weight, edge_source) in edges {
                    self.graph.remove_edge(edge_id);
                    self.graph.add_edge(edge_source, new_idx, edge_weight);
                }

                Ok(self.new_selector(new_idx))
            }
            InsertSide::Below => {
                let edges = self
                    .graph
                    .edges_directed(target.id, Direction::Outgoing)
                    .map(|e| (e.id(), e.weight().to_owned(), e.target()))
                    .collect::<Vec<_>>();

                let new_idx = self.graph.add_node(step);
                self.graph.add_edge(target.id, new_idx, Edge { order: 0 });

                for (edge_id, edge_weight, edge_target) in edges {
                    self.graph.remove_edge(edge_id);
                    self.graph.add_edge(new_idx, edge_target, edge_weight);
                }

                Ok(self.new_selector(new_idx))
            }
        }
    }

    /// Add an edge to the graph with a desired order.
    ///
    /// Bails if there is already an edge from the child to the parent with the
    /// same order.
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
                for parent in self
                    .graph
                    .edges_directed(tip, Direction::Outgoing)
                    .map(|e| e.target())
                {
                    if seen.insert(parent) {
                        tips.push(parent);
                    }
                }
            }

            if seen.contains(&child.id) {
                bail!("BUG: Add edge introduces a cycle");
            }
        }

        if self
            .graph
            .edges_directed(child.id, Direction::Outgoing)
            .any(|edge| edge.weight().order == desired_order)
        {
            bail!("An edge with desired order {desired_order} already exists");
        }

        self.graph.add_edge(
            child.id,
            parent.id,
            Edge {
                order: desired_order,
            },
        );

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

        let edges = self
            .graph
            .edges_directed(child.id, Direction::Outgoing)
            .filter_map(|e| (e.target() == parent.id).then_some(e.id()))
            .collect::<Vec<_>>();

        let mut orders = vec![];
        for edge in edges {
            let weight = self
                .graph
                .remove_edge(edge)
                .context("BUG: Failed to remove edge")?;

            orders.push(weight.order);
        }

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
