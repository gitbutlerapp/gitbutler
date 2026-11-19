use petgraph::Direction;

use crate::{Graph, SegmentIndex, init::types::EdgeOwned};

impl Graph {
    /// Rebuild outgoing edges so petgraph traversal follows the source commit's
    /// `parent_order`. Petgraph traverses edges in reverse creation order, so
    /// edges are re-added from last parent to first parent.
    pub(crate) fn rebuild_edges_in_parent_order(&mut self) {
        for sidx in self.segments().collect::<Vec<_>>() {
            self.rebuild_outgoing_edges_in_parent_order(sidx);
        }
    }

    /// Like [`Self::rebuild_edges_in_parent_order()`], for `sidx` alone.
    fn rebuild_outgoing_edges_in_parent_order(&mut self, sidx: SegmentIndex) {
        let mut outgoing_edges: Vec<EdgeOwned> = self
            .inner
            .edges_directed(sidx, Direction::Outgoing)
            .map(EdgeOwned::from)
            .collect();
        if outgoing_edges.len() < 2 {
            return;
        }
        outgoing_edges.sort_by_key(|e| std::cmp::Reverse(e.weight.parent_order));
        for edge in &outgoing_edges {
            self.inner.remove_edge(edge.id);
        }
        for edge in &outgoing_edges {
            self.inner.add_edge(edge.source, edge.target, edge.weight);
        }
    }

    /// Fill in generation numbers by walking down the graph topologically, so each segment
    /// knows how far down the graph it is.
    pub(crate) fn compute_generation_numbers(&mut self) {
        let mut topo = petgraph::visit::Topo::new(&self.inner);
        while let Some(sidx) = topo.next(&self.inner) {
            let max_gen_of_incoming = self
                .inner
                .neighbors_directed(sidx, Direction::Incoming)
                .map(|sidx| self[sidx].generation + 1)
                .max()
                .unwrap_or(0);
            self[sidx].generation = max_gen_of_incoming;
        }
    }
}
