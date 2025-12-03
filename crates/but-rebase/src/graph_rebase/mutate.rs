//! Operations for mutation the editor

use petgraph::{Direction, visit::EdgeRef};

use crate::graph_rebase::{Edge, Editor, Selector, Step};

/// Describes where relative to the selector a step should be inserted
#[derive(Debug, Clone, Copy)]
pub enum InsertSide {
    /// When inserting above, any nodes that point to the selector will now
    /// point to the inserted node instead.
    Above,
    /// When inserting below, any nodes that the selector points to will now be
    /// pointed to by the inserted node instead.
    Below,
}

/// Operations for mutating the commit graph
impl Editor {
    /// Get a selector to a particular commit in the graph
    pub fn select_commit(&self, target: gix::ObjectId) -> Option<Selector> {
        for node_idx in self.graph.node_indices() {
            if let Step::Pick { id, .. } = self.graph[node_idx]
                && id == target
            {
                return Some(Selector { id: node_idx });
            }
        }

        None
    }

    /// Get a selector to a particular reference in the graph
    pub fn select_reference(&self, target: &gix::refs::FullNameRef) -> Option<Selector> {
        for node_idx in self.graph.node_indices() {
            if let Step::Reference { refname } = &self.graph[node_idx]
                && target == refname.as_ref()
            {
                return Some(Selector { id: node_idx });
            }
        }

        None
    }

    /// Replaces the node that the function was pointing to.
    ///
    /// Returns the replaced step.
    pub fn replace(&mut self, target: &Selector, step: Step) -> Step {
        let old = self.graph[target.id].clone();
        self.graph[target.id] = step;
        old
    }

    /// Inserts a new node relative to a selector
    ///
    ///
    /// When inserting above, any nodes that point to the selector will now
    /// point to the inserted node instead. When inserting below, any nodes
    /// that the selector points to will now be pointed to by the inserted node
    /// instead.
    pub fn insert(&mut self, target: &Selector, step: Step, side: InsertSide) {
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
            }
        }
    }
}
