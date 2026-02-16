//! Operations for mutating the editor

use anyhow::{Result, anyhow};
use petgraph::{Direction, visit::EdgeRef};
use serde::{Deserialize, Serialize};

use crate::graph_rebase::{Edge, Editor, Pick, Selector, Step, ToCommitSelector, ToReferenceSelector, ToSelector};

/// Describes where relative to the selector a step should be inserted
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertSide {
    /// When inserting above, any nodes that point to the selector will now
    /// point to the inserted node instead.
    Above,
    /// When inserting below, any nodes that the selector points to will now be
    /// pointed to by the inserted node instead.
    Below,
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
            None => Err(anyhow!("Failed to find reference {target} in rebase editor")),
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

    /// Inserts a new node relative to a selector
    ///
    /// When inserting above, any nodes that point to the selector will now
    /// point to the inserted node instead. When inserting below, any nodes
    /// that the selector points to will now be pointed to by the inserted node
    /// instead.
    ///
    /// Returns a selector to the inserted step
    pub fn insert(&mut self, target: impl ToSelector, step: Step, side: InsertSide) -> Result<Selector> {
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
