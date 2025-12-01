//! Operations for mutation the editor

use crate::graph_rebase::{Editor, Selector, Step};

/// Operations for mutating the commit graph
impl Editor {
    /// Get a selector to a particular commit in the graph
    pub fn select_commit(&self, target: gix::ObjectId) -> Option<Selector> {
        for node_idx in self.graph.node_indices() {
            if let Step::Pick { id } = self.graph[node_idx]
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
}
