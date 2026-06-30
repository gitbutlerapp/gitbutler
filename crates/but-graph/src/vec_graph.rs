//! A small directed graph with stable `usize` node ids.
//!
//! Candidate replacement for the petgraph `StableGraph` that but-graph (and but-rebase's
//! `StepGraph`) use purely as a *container*: the only petgraph features actually exercised are
//! stable ids across node removal, bidirectional adjacency (`Incoming`/`Outgoing`), a topological
//! sort, and a debug renderer — none of petgraph's algorithm library. The graphs are tiny (a
//! workspace has a handful of segments), so this trades that generality for a structure that is
//! easy to read and hands out plain `usize` ids (no `NodeIndex` leaking into the public API).
//!
//! Generic over node weight `N` and edge weight `E`, so the same structure can back the segment
//! graph (`N = Segment`, `E = Edge`) and the rebase step graph (`N = Step`, `E = StepEdge`).
//!

use std::collections::VecDeque;

pub type NodeId = usize;
pub type EdgeId = usize;

/// Which side of a node's edges to traverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Edges leaving the node.
    Outgoing,
    /// Edges entering the node.
    Incoming,
}

#[derive(Debug, Clone)]
struct EdgeRec<E> {
    source: NodeId,
    target: NodeId,
    weight: E,
}

/// A live edge yielded during iteration.
#[derive(Debug, Clone, Copy)]
pub struct EdgeRef<'a, E> {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub weight: &'a E,
}

/// Accessor methods mirroring petgraph's `EdgeRef` trait, so call sites read the same either way.
impl<'a, E> EdgeRef<'a, E> {
    pub fn id(&self) -> EdgeId {
        self.id
    }
    pub fn source(&self) -> NodeId {
        self.source
    }
    pub fn target(&self) -> NodeId {
        self.target
    }
    pub fn weight(&self) -> &'a E {
        self.weight
    }
}

/// A directed graph that mirrors the two petgraph `StableGraph` behaviours but-graph's post-passes
/// rely on: removing a node/edge tombstones its slot, and the slot is reused (LIFO, like petgraph's
/// free-list) before a fresh id is handed out — so ids stay stable *and* match what petgraph would
/// assign across the same removals. `edges_directed` also yields a node's edges newest-first.
#[derive(Debug, Clone)]
pub struct Graph<N, E> {
    nodes: Vec<Option<N>>,          // index = NodeId
    edges: Vec<Option<EdgeRec<E>>>, // index = EdgeId
    outgoing: Vec<Vec<EdgeId>>,     // node -> its outgoing edge ids, in insertion order
    incoming: Vec<Vec<EdgeId>>,     // node -> its incoming edge ids, in insertion order
    free_nodes: Vec<NodeId>,        // tombstoned node slots, reused LIFO
    free_edges: Vec<EdgeId>,        // tombstoned edge slots, reused LIFO
}

impl<N, E> Default for Graph<N, E> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
            free_nodes: Vec::new(),
            free_edges: Vec::new(),
        }
    }
}

impl<N, E> Graph<N, E> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of live (non-tombstoned) nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    pub fn add_node(&mut self, weight: N) -> NodeId {
        if let Some(id) = self.free_nodes.pop() {
            // Reused slot; its adjacency lists were cleared on removal.
            self.nodes[id] = Some(weight);
            id
        } else {
            let id = self.nodes.len();
            self.nodes.push(Some(weight));
            self.outgoing.push(Vec::new());
            self.incoming.push(Vec::new());
            id
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&N> {
        self.nodes.get(id).and_then(Option::as_ref)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut N> {
        self.nodes.get_mut(id).and_then(Option::as_mut)
    }

    /// Tombstone `id` and drop every edge incident to it. Other ids stay valid.
    pub fn remove_node(&mut self, id: NodeId) -> Option<N> {
        let removed = self.nodes.get_mut(id)?.take()?;
        let incident: Vec<EdgeId> = self.outgoing[id]
            .iter()
            .chain(self.incoming[id].iter())
            .copied()
            .collect();
        for edge in incident {
            self.remove_edge(edge);
        }
        self.outgoing[id] = Vec::new();
        self.incoming[id] = Vec::new();
        self.free_nodes.push(id);
        Some(removed)
    }

    /// Add an edge `source -> target`. Both endpoints must be live nodes.
    pub fn add_edge(&mut self, source: NodeId, target: NodeId, weight: E) -> EdgeId {
        debug_assert!(
            self.node(source).is_some(),
            "edge source must be a live node"
        );
        debug_assert!(
            self.node(target).is_some(),
            "edge target must be a live node"
        );
        let rec = EdgeRec {
            source,
            target,
            weight,
        };
        let id = if let Some(id) = self.free_edges.pop() {
            self.edges[id] = Some(rec);
            id
        } else {
            let id = self.edges.len();
            self.edges.push(Some(rec));
            id
        };
        self.outgoing[source].push(id);
        self.incoming[target].push(id);
        id
    }

    pub fn remove_edge(&mut self, id: EdgeId) -> Option<E> {
        let rec = self.edges.get_mut(id)?.take()?;
        self.outgoing[rec.source].retain(|&e| e != id);
        self.incoming[rec.target].retain(|&e| e != id);
        self.free_edges.push(id);
        Some(rec.weight)
    }

    pub fn edge_weight(&self, id: EdgeId) -> Option<&E> {
        self.edges
            .get(id)
            .and_then(Option::as_ref)
            .map(|r| &r.weight)
    }

    /// Live node ids, ascending.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_ref().map(|_| i))
    }

    /// petgraph-compatible alias for [`Self::node`].
    pub fn node_weight(&self, id: NodeId) -> Option<&N> {
        self.node(id)
    }

    /// petgraph-compatible alias for [`Self::node_ids`].
    pub fn node_indices(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.node_ids()
    }

    /// Edges leaving (`Outgoing`) or entering (`Incoming`) `node`, newest-first (most recently added
    /// edge first), matching petgraph's `StableGraph` iteration order.
    pub fn edges_directed(
        &self,
        node: NodeId,
        dir: Direction,
    ) -> impl Iterator<Item = EdgeRef<'_, E>> + '_ {
        let list: &[EdgeId] = match dir {
            Direction::Outgoing => self.outgoing.get(node),
            Direction::Incoming => self.incoming.get(node),
        }
        .map_or(&[], Vec::as_slice);
        list.iter().rev().filter_map(move |&eid| {
            self.edges[eid].as_ref().map(|rec| EdgeRef {
                id: eid,
                source: rec.source,
                target: rec.target,
                weight: &rec.weight,
            })
        })
    }

    /// Neighboring node ids in the given direction, in insertion order.
    pub fn neighbors_directed(
        &self,
        node: NodeId,
        dir: Direction,
    ) -> impl Iterator<Item = NodeId> + '_ {
        self.edges_directed(node, dir).map(move |e| match dir {
            Direction::Outgoing => e.target,
            Direction::Incoming => e.source,
        })
    }

    /// Topological order (Kahn's algorithm): every live edge's `source` appears before its
    /// `target`. Assumes a DAG; nodes left over after a cycle are simply omitted.
    pub fn toposort(&self) -> Vec<NodeId> {
        let mut in_degree = vec![0usize; self.nodes.len()];
        for id in self.node_ids() {
            in_degree[id] = self.edges_directed(id, Direction::Incoming).count();
        }
        let mut queue: VecDeque<NodeId> =
            self.node_ids().filter(|&id| in_degree[id] == 0).collect();
        let mut order = Vec::with_capacity(self.node_count());
        while let Some(id) = queue.pop_front() {
            order.push(id);
            for nb in self.neighbors_directed(id, Direction::Outgoing) {
                in_degree[nb] -= 1;
                if in_degree[nb] == 0 {
                    queue.push_back(nb);
                }
            }
        }
        order
    }

    /// Live nodes with no edges in `dir` — sinks for `Outgoing`, roots for `Incoming`.
    pub fn externals(&self, dir: Direction) -> impl Iterator<Item = NodeId> + '_ {
        self.node_ids().filter(move |&id| {
            match dir {
                Direction::Outgoing => &self.outgoing[id],
                Direction::Incoming => &self.incoming[id],
            }
            .is_empty()
        })
    }

    /// Number of live (non-tombstoned) edges.
    pub fn edge_count(&self) -> usize {
        self.edges.iter().filter(|e| e.is_some()).count()
    }

    /// Live edges from `source` to `target`, in insertion order.
    pub fn edges_connecting(
        &self,
        source: NodeId,
        target: NodeId,
    ) -> impl Iterator<Item = EdgeRef<'_, E>> + '_ {
        self.edges_directed(source, Direction::Outgoing)
            .filter(move |e| e.target == target)
    }

    /// petgraph-compatible alias: a node's outgoing edges.
    pub fn edges(&self, node: NodeId) -> impl Iterator<Item = EdgeRef<'_, E>> + '_ {
        self.edges_directed(node, Direction::Outgoing)
    }

    /// All live edges, ascending by id.
    pub fn edge_references(&self) -> impl Iterator<Item = EdgeRef<'_, E>> + '_ {
        self.edges.iter().enumerate().filter_map(|(id, rec)| {
            rec.as_ref().map(|r| EdgeRef {
                id,
                source: r.source,
                target: r.target,
                weight: &r.weight,
            })
        })
    }

    /// petgraph-compatible alias for [`Self::node_mut`].
    pub fn node_weight_mut(&mut self, id: NodeId) -> Option<&mut N> {
        self.node_mut(id)
    }

    pub fn edge_weight_mut(&mut self, id: EdgeId) -> Option<&mut E> {
        self.edges
            .get_mut(id)
            .and_then(Option::as_mut)
            .map(|r| &mut r.weight)
    }

    /// One past the largest node id ever handed out (tombstoned slots included), like petgraph's
    /// `NodeIndexable::node_bound`.
    pub fn node_bound(&self) -> usize {
        self.nodes.len()
    }

    /// Weights of all live nodes, ascending by id.
    pub fn node_weights(&self) -> impl Iterator<Item = &N> + '_ {
        self.nodes.iter().filter_map(Option::as_ref)
    }

    /// Mutable weights of all live nodes, ascending by id.
    pub fn node_weights_mut(&mut self) -> impl Iterator<Item = &mut N> + '_ {
        self.nodes.iter_mut().filter_map(Option::as_mut)
    }

    /// Two distinct live nodes, mutably — like petgraph's `index_twice_mut`.
    pub fn index_twice_mut(&mut self, a: NodeId, b: NodeId) -> (&mut N, &mut N) {
        assert_ne!(a, b, "index_twice_mut requires distinct nodes");
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let (left, right) = self.nodes.split_at_mut(hi);
        let lo_ref = left[lo].as_mut().expect("live node");
        let hi_ref = right[0].as_mut().expect("live node");
        if a < b {
            (lo_ref, hi_ref)
        } else {
            (hi_ref, lo_ref)
        }
    }
}

impl<N, E> std::ops::Index<NodeId> for Graph<N, E> {
    type Output = N;
    fn index(&self, id: NodeId) -> &N {
        self.node(id).expect("node id refers to a live node")
    }
}

impl<N, E> std::ops::IndexMut<NodeId> for Graph<N, E> {
    fn index_mut(&mut self, id: NodeId) -> &mut N {
        self.node_mut(id).expect("node id refers to a live node")
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, Graph};

    fn collect<I: IntoIterator<Item = usize>>(it: I) -> Vec<usize> {
        it.into_iter().collect()
    }

    #[test]
    fn add_query_and_bidirectional_adjacency() {
        let mut g: Graph<&str, ()> = Graph::new();
        let a = g.add_node("a");
        let b = g.add_node("b");
        let c = g.add_node("c");
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
        g.add_edge(b, c, ());

        assert_eq!(g.node(a), Some(&"a"));
        assert_eq!(g.node_count(), 3);
        // Newest-first: a->c was added after a->b, so `c` comes first.
        assert_eq!(
            collect(g.neighbors_directed(a, Direction::Outgoing)),
            vec![c, b]
        );
        assert_eq!(
            collect(g.neighbors_directed(c, Direction::Incoming)),
            vec![b, a]
        );
        assert_eq!(
            collect(g.neighbors_directed(a, Direction::Incoming)),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn remove_node_keeps_other_ids_stable_and_drops_incident_edges() {
        let mut g: Graph<&str, ()> = Graph::new();
        let a = g.add_node("a");
        let b = g.add_node("b");
        let c = g.add_node("c");
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());

        let removed = g.remove_node(b);
        assert_eq!(removed, Some("b"));
        // a and c keep their ids and values.
        assert_eq!(g.node(a), Some(&"a"));
        assert_eq!(g.node(c), Some(&"c"));
        assert_eq!(g.node(b), None);
        assert_eq!(collect(g.node_ids()), vec![a, c]);
        // Edges incident to b are gone from both directions.
        assert_eq!(
            collect(g.neighbors_directed(a, Direction::Outgoing)),
            Vec::<usize>::new()
        );
        assert_eq!(
            collect(g.neighbors_directed(c, Direction::Incoming)),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn remove_edge_cleans_both_adjacency_lists() {
        let mut g: Graph<&str, i32> = Graph::new();
        let a = g.add_node("a");
        let b = g.add_node("b");
        let e0 = g.add_edge(a, b, 7);
        let e1 = g.add_edge(a, b, 9);

        assert_eq!(g.edge_weight(e0), Some(&7));
        assert_eq!(g.remove_edge(e0), Some(7));
        assert_eq!(g.edge_weight(e0), None);
        // The surviving parallel edge is still reachable in both directions.
        let out: Vec<_> = g
            .edges_directed(a, Direction::Outgoing)
            .map(|e| e.id)
            .collect();
        let inc: Vec<_> = g
            .edges_directed(b, Direction::Incoming)
            .map(|e| e.id)
            .collect();
        assert_eq!(out, vec![e1]);
        assert_eq!(inc, vec![e1]);
    }

    #[test]
    fn toposort_places_every_source_before_its_target() {
        // Diamond: a -> b, a -> c, b -> d, c -> d.
        let mut g: Graph<char, ()> = Graph::new();
        let a = g.add_node('a');
        let b = g.add_node('b');
        let c = g.add_node('c');
        let d = g.add_node('d');
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
        g.add_edge(b, d, ());
        g.add_edge(c, d, ());

        let order = g.toposort();
        assert_eq!(order.len(), 4);
        let pos = |n: usize| order.iter().position(|&x| x == n).unwrap();
        for id in g.node_ids() {
            for nb in g.neighbors_directed(id, Direction::Outgoing) {
                assert!(
                    pos(id) < pos(nb),
                    "source must precede target in topo order"
                );
            }
        }
    }

    #[test]
    fn toposort_skips_removed_nodes() {
        let mut g: Graph<char, ()> = Graph::new();
        let a = g.add_node('a');
        let b = g.add_node('b');
        let c = g.add_node('c');
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.remove_node(b);
        // a and c are now disconnected; both should appear, b should not.
        let order = g.toposort();
        assert_eq!(order.len(), 2);
        assert!(order.contains(&a) && order.contains(&c) && !order.contains(&b));
    }

    #[test]
    fn externals_edge_count_and_edges_connecting() {
        // a -> b (x2), a -> c, b -> c.
        let mut g: Graph<char, i32> = Graph::new();
        let a = g.add_node('a');
        let b = g.add_node('b');
        let c = g.add_node('c');
        let e0 = g.add_edge(a, b, 1);
        let e1 = g.add_edge(a, b, 2);
        g.add_edge(a, c, 3);
        g.add_edge(b, c, 4);

        // `c` is the only sink; `a` is the only root.
        assert_eq!(collect(g.externals(Direction::Outgoing)), vec![c]);
        assert_eq!(collect(g.externals(Direction::Incoming)), vec![a]);

        assert_eq!(g.edge_count(), 4);

        // Both parallel a->b edges are returned, newest-first; nothing for b->a.
        let connecting: Vec<_> = g.edges_connecting(a, b).map(|e| e.id).collect();
        assert_eq!(connecting, vec![e1, e0]);
        assert_eq!(g.edges_connecting(b, a).count(), 0);

        // edge_count tracks tombstones.
        g.remove_edge(e0);
        assert_eq!(g.edge_count(), 3);
        assert_eq!(
            g.edges_connecting(a, b).map(|e| e.id).collect::<Vec<_>>(),
            vec![e1]
        );
    }
}
