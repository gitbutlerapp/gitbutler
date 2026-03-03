//! Operations for mutating the editor

use std::collections::HashSet;

use anyhow::{Result, anyhow};
use petgraph::{Direction, visit::EdgeRef};
use serde::{Deserialize, Serialize};

use crate::graph_rebase::{
    Edge, Editor, Pick, Selector, Step, ToCommitSelector, ToReferenceSelector, ToSelector,
};

/// Describes where relative to the selector a step should be inserted
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    fn to_selector(&self, editor: &Editor) -> Result<Selector> {
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
pub enum RelativeTo<'a> {
    /// Relative to a commit
    Commit(gix::ObjectId),
    /// Relative to a reference
    Reference(&'a gix::refs::FullNameRef),
}

impl ToSelector for RelativeTo<'_> {
    fn to_selector(&self, editor: &Editor) -> Result<Selector> {
        match self {
            Self::Commit(id) => editor.select_commit(*id),
            Self::Reference(reference) => editor.select_reference(reference),
        }
    }
}

impl ToCommitSelector for gix::ObjectId {
    fn to_commit_selector(&self, editor: &Editor) -> Result<Selector> {
        editor.select_commit(*self)
    }
}

impl ToReferenceSelector for &gix::refs::FullNameRef {
    fn to_reference_selector(&self, editor: &Editor) -> Result<Selector> {
        editor.select_reference(self)
    }
}

impl ToReferenceSelector for gix::refs::FullName {
    fn to_reference_selector(&self, editor: &Editor) -> Result<Selector> {
        editor.select_reference(self.as_ref())
    }
}

/// Operations for mutating the commit graph
impl Editor {
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
                return Some(Selector {
                    id: node_idx,
                    revision: self.history.current_revision(),
                });
            }
        }

        None
    }

    /// Get a selector to a particular reference in the graph
    pub fn try_select_reference(&self, target: &gix::refs::FullNameRef) -> Option<Selector> {
        for node_idx in self.graph.node_indices() {
            if let Step::Reference { refname } = &self.graph[node_idx]
                && target == refname.as_ref()
            {
                return Some(Selector {
                    id: node_idx,
                    revision: self.history.current_revision(),
                });
            }
        }

        None
    }

    /// Replaces the node that the function was pointing to.
    ///
    /// Returns the replaced step.
    pub fn replace(&mut self, target: impl ToSelector, step: Step) -> Result<Step> {
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        let old = self.graph[target.id].clone();
        self.graph[target.id] = step;
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
    /// Returns an error when:
    /// - `parents_to_disconnect` is `SelectorSet::None`
    /// - `parents_to_disconnect` contains any parent that is not a direct parent of `target.parent`
    /// - `children_to_disconnect` contains any child that is not a direct parent of `target.child`
    pub fn disconnect_segment_from<C, P>(
        &mut self,
        target: SegmentDelimiter<C, P>,
        children_to_disconnect: SelectorSet,
        parents_to_disconnect: SelectorSet,
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
                return Err(anyhow!(
                    "Invalid parents to disconnect: SelectorSet::None is not allowed"
                ));
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
                self.reconnect_edges_to_parents(&disconnected_parent_edges, edge_id, edge_source);
            }
        }

        Ok(())
    }

    /// Remove the child edge, and reconnect to the right parents.
    fn reconnect_edges_to_parents(
        &mut self,
        disconnected_parent_edges: &[(Edge, petgraph::prelude::NodeIndex)],
        child_edge_id: petgraph::prelude::EdgeIndex,
        child_node: petgraph::prelude::NodeIndex,
    ) {
        // Remove the child edge.
        self.graph.remove_edge(child_edge_id);
        // Reconnect the child node to all the disconnected parents.
        for (parent_edge_weight, edge_target) in disconnected_parent_edges {
            self.graph
                .add_edge(child_node, *edge_target, parent_edge_weight.clone());
        }
    }

    /// Insert a segement relative to a selector.
    ///
    /// The segment is described by its delimiter: First (parent-most) and last (child-most) node.
    ///
    /// If inserted above, all the target selector's children will be disconnected and reconnected to the last
    /// node of the segment.
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
        let SegmentDelimiter { child, parent } = delimiter;
        let target = self.history.normalize_selector(target.to_selector(self)?)?;
        let child = self.history.normalize_selector(child.to_selector(self)?)?;
        let parent = self.history.normalize_selector(parent.to_selector(self)?)?;

        match side {
            InsertSide::Above => {
                // Children edges of target.
                let edges = self
                    .graph
                    .edges_directed(target.id, Direction::Incoming)
                    .map(|e| (e.id(), e.weight().to_owned(), e.source()))
                    .collect::<Vec<_>>();

                // Find the child node of the highest order from the child-most node in the segment being inserted.
                let chubbiest_grand_child = self
                    .graph
                    .edges_directed(child.id, Direction::Incoming)
                    .map(|e| (e.id(), e.weight().to_owned(), e.source()))
                    .max_by_key(|gc| gc.1.order);

                // Connect all target's children with the child-most node in the given segment.
                for (edge_id, edge_weight, edge_source) in edges {
                    self.graph.remove_edge(edge_id);
                    // Avoid weight collision by adding the order value of the highest order child plus one,
                    // accommodating for order 0.
                    let new_weight =
                        if let Some((_, grand_child_weight, _)) = chubbiest_grand_child.as_ref() {
                            Edge {
                                order: edge_weight.order + grand_child_weight.order + 1,
                            }
                        } else {
                            edge_weight
                        };
                    self.graph.add_edge(edge_source, child.id, new_weight);
                }

                // Find the parent node of the highest order from the parent-most node in the segment being inserted.
                let chubbiest_grand_parent = self
                    .graph
                    .edges_directed(parent.id, Direction::Outgoing)
                    .map(|e| (e.id(), e.weight().to_owned(), e.target()))
                    .max_by_key(|gc| gc.1.order);

                let new_weight =
                    if let Some((_, grand_parent_weight, _)) = chubbiest_grand_parent.as_ref() {
                        Edge {
                            order: grand_parent_weight.order + 1,
                        }
                    } else {
                        Edge { order: 0 }
                    };
                // Connect the target to the parent-most node in the given segment.
                self.graph.add_edge(parent.id, target.id, new_weight);
            }
            InsertSide::Below => {
                let edges = self
                    .graph
                    .edges_directed(target.id, Direction::Outgoing)
                    .map(|e| (e.id(), e.weight().to_owned(), e.target()))
                    .collect::<Vec<_>>();

                // Find the parent node of the highest order from the parent-most node in the segment being inserted.
                let chubbiest_grand_parent = self
                    .graph
                    .edges_directed(parent.id, Direction::Outgoing)
                    .map(|e| (e.id(), e.weight().to_owned(), e.target()))
                    .max_by_key(|gc| gc.1.order);

                // Connect all target's parents to the parent-most node in the given segment.
                for (edge_id, edge_weight, edge_target) in edges {
                    self.graph.remove_edge(edge_id);
                    // Avoid weight collision by adding the order value of the highest order parent plus one,
                    // accommodating for order 0.
                    let new_weight = if let Some((_, grand_parent_weight, _)) =
                        chubbiest_grand_parent.as_ref()
                    {
                        Edge {
                            order: edge_weight.order + grand_parent_weight.order + 1,
                        }
                    } else {
                        edge_weight
                    };
                    self.graph.add_edge(parent.id, edge_target, new_weight);
                }

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

                Ok(Selector {
                    id: new_idx,
                    revision: self.history.current_revision(),
                })
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

                Ok(Selector {
                    id: new_idx,
                    revision: self.history.current_revision(),
                })
            }
        }
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
