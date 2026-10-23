//! The segment graph store, which keeps edges in traversal order.
use std::ops::{Deref, Index, IndexMut};

use petgraph::{Direction, stable_graph::EdgeIndex, visit::EdgeRef};

use super::PetGraph;
use crate::{Edge, Segment, SegmentIndex};

/// The petgraph that stores segments and the edges between them, keeping edges in
/// *traversal order*: the order readers iterate them in and rely on.
///
/// Petgraph hands back a node's edges newest first. For the outgoing edges of a segment that
/// order must match Git: the workspace stack order for edges that have one, then the parent
/// order of the commit. For the incoming edges of a segment there is nothing to recompute it
/// from, so it is whatever order the edges were added in.
///
/// [`Self::add_edge()`] is the only way to insert an edge and restores the outgoing order of its
/// source after each insert. [`Self::add_edges_in_traversal_order()`] adds a batch so that it
/// keeps iterating in the order given, for moving edges from one target to another.
/// Reads go through [`Deref`] to the petgraph; there is deliberately no `DerefMut`, so neither
/// order can be bypassed.
#[derive(Default, Clone)]
pub(crate) struct TraversalOrderedGraph {
    inner: PetGraph,
}

/// Mutation
impl TraversalOrderedGraph {
    /// Add `edge` from `src` to `dst`, keeping the outgoing edges of `src` in traversal order.
    ///
    /// Restoring the order may recreate the other outgoing edges of `src`, so any [`EdgeIndex`]
    /// of those held across this call is stale. This is why no index is returned.
    pub(crate) fn add_edge(&mut self, src: SegmentIndex, dst: SegmentIndex, edge: Edge) {
        let id = self.inner.add_edge(src, dst, edge);
        self.place_new_edge(src, id);
    }

    /// Add `edges` so that at each target they iterate in the order given.
    ///
    /// Use this to put back edges obtained from `collect_edges_at_commit_in_traversal_order()`
    /// after removing them, possibly with a new target.
    pub(crate) fn add_edges_in_traversal_order(
        &mut self,
        edges: impl IntoIterator<Item = (SegmentIndex, SegmentIndex, Edge)>,
    ) {
        let edges: Vec<_> = edges.into_iter().collect();
        // Petgraph iterates newest edges first, so insert the last one first.
        for (src, dst, edge) in edges.into_iter().rev() {
            self.add_edge(src, dst, edge);
        }
    }

    /// Remove the edge with `id`, returning its weight if it existed.
    pub(crate) fn remove_edge(&mut self, id: EdgeIndex) -> Option<Edge> {
        self.inner.remove_edge(id)
    }

    /// Add `segment` as a new node and return its index.
    pub(crate) fn add_node(&mut self, segment: Segment) -> SegmentIndex {
        self.inner.add_node(segment)
    }

    /// Remove the segment at `sidx` along with all its edges, returning it if it existed.
    pub(crate) fn remove_node(&mut self, sidx: SegmentIndex) -> Option<Segment> {
        self.inner.remove_node(sidx)
    }

    /// Make the edge with `id` point at its target segment as a whole rather than at a commit in it.
    ///
    /// This is the only edge-weight mutation offered as the ordering fields must stay untouched.
    pub(crate) fn clear_edge_destination(&mut self, id: EdgeIndex) {
        let edge = self
            .inner
            .edge_weight_mut(id)
            .expect("edge ids passed here were just listed");
        edge.dst = None;
        edge.dst_id = None;
    }

    /// Mutable access to the segment at `sidx`, if it exists. Segments carry no edge order.
    pub(crate) fn node_weight_mut(&mut self, sidx: SegmentIndex) -> Option<&mut Segment> {
        self.inner.node_weight_mut(sidx)
    }

    /// Mutable access to all segments. Segments carry no edge order.
    pub(crate) fn node_weights_mut(&mut self) -> impl Iterator<Item = &mut Segment> {
        self.inner.node_weights_mut()
    }

    /// Re-add the outgoing edges of `src` so petgraph iterates them in traversal order, with the
    /// newly inserted edge `new_id` placed after every edge of the same order.
    ///
    /// The other outgoing edges are assumed to be in traversal order already, which this type
    /// guarantees, so edges of equal order keep iterating in insertion order.
    fn place_new_edge(&mut self, src: SegmentIndex, new_id: EdgeIndex) {
        let current: Vec<_> = self
            .inner
            .edges_directed(src, Direction::Outgoing)
            .map(|e| (e.id(), e.target(), traversal_key(e.weight())))
            .collect();
        if current.len() < 2 {
            return;
        }
        // Petgraph iterates the newest edge first; move it last so a stable sort puts it
        // behind its equals, then order everything.
        let mut ordered: Vec<_> = current.iter().filter(|e| e.0 != new_id).copied().collect();
        ordered.extend(current.iter().filter(|e| e.0 == new_id).copied());
        ordered.sort_by_key(|(_, _, key)| *key);
        if ordered == current {
            return;
        }
        let edges: Vec<_> = ordered
            .into_iter()
            .map(|(id, target, _)| {
                let weight = self
                    .inner
                    .remove_edge(id)
                    .expect("edge was listed just before removal");
                (target, weight)
            })
            .collect();
        // Petgraph iterates newest edges first, so insert the last one first.
        for (target, weight) in edges.into_iter().rev() {
            self.inner.add_edge(src, target, weight);
        }
    }
}

/// Validation
impl TraversalOrderedGraph {
    /// Fail if any segment's outgoing edges don't iterate in traversal order.
    pub(crate) fn check_traversal_order(&self) -> anyhow::Result<()> {
        for sidx in self.inner.node_indices() {
            let keys: Vec<_> = self
                .inner
                .edges_directed(sidx, Direction::Outgoing)
                .map(|e| traversal_key(e.weight()))
                .collect();
            anyhow::ensure!(
                keys.windows(2).all(|w| w[0] <= w[1]),
                "BUG: outgoing edges of segment {sidx:?} are not in traversal order: {keys:?}"
            );
        }
        Ok(())
    }
}

/// The sort key for the outgoing edges of one segment.
///
/// Edges placed by workspace metadata come first, in that order, followed by all others in
/// Git's parent order.
fn traversal_key(edge: &Edge) -> (bool, u32, u32) {
    (
        edge.workspace_parent_order.is_none(),
        edge.workspace_parent_order.unwrap_or(0),
        edge.parent_order,
    )
}

impl std::fmt::Debug for TraversalOrderedGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl Deref for TraversalOrderedGraph {
    type Target = PetGraph;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Index<SegmentIndex> for TraversalOrderedGraph {
    type Output = Segment;

    fn index(&self, index: SegmentIndex) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<EdgeIndex> for TraversalOrderedGraph {
    type Output = Edge;

    fn index(&self, index: EdgeIndex) -> &Self::Output {
        &self.inner[index]
    }
}

impl IndexMut<SegmentIndex> for TraversalOrderedGraph {
    fn index_mut(&mut self, index: SegmentIndex) -> &mut Self::Output {
        &mut self.inner[index]
    }
}
