//! The segment graph, where each [`Segment`] directly owns its outgoing [`Connection`]s.
//!
//! There is no separate edge list and no generic graph container: the segments *are* the graph.
//! A [`SegmentGraph`] is just an arena of segments (`Vec<Option<Segment>>` with a free-list so ids
//! stay stable across removal), and every connection — which target segment, and which commit
//! connects to which — lives on the source segment in [`Segment::connections`].
//!
//! Incoming edges are derived by scanning (the graph is tiny: a handful of segments per workspace),
//! so only the outgoing direction is stored. Outgoing connections are kept in first-parent order
//! (see `order_outgoing_connections_by_first_parent`), which is the order traversal wants.

use crate::{CommitIndex, Segment, SegmentIndex};

/// Which side of a segment's connections to traverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Connections leaving the segment (stored on it).
    Outgoing,
    /// Connections entering the segment (derived by scanning).
    Incoming,
}

/// An outgoing connection from one [`Segment`] to another, stored on the source segment.
///
/// It carries the intent of the connection: *which* commit in the source connects to *which* commit
/// in the target. This is load-bearing during graph construction (empty segments have `src: None`,
/// segment-splitting reroutes by `src_id`, the walker filters by commit range); a finished, validated
/// graph always has `src` at the source's last commit and `dst` at the target's first.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Connection {
    /// The segment this connection points to.
    pub target: SegmentIndex,
    /// `None` if the source segment has no commit.
    pub(crate) src: Option<CommitIndex>,
    /// The commit id at `src` in the source segment's commit list.
    pub(crate) src_id: Option<gix::ObjectId>,
    /// The commit index this connection points at in the target segment.
    pub(crate) dst: Option<CommitIndex>,
    /// The commit id at `dst` in the target segment's commit list.
    pub(crate) dst_id: Option<gix::ObjectId>,
}

impl Connection {
    /// Create a connection to `target` carrying the given commit endpoints.
    pub(crate) fn new(
        target: SegmentIndex,
        src: Option<CommitIndex>,
        src_id: Option<gix::ObjectId>,
        dst: Option<CommitIndex>,
        dst_id: Option<gix::ObjectId>,
    ) -> Self {
        Connection {
            target,
            src,
            src_id,
            dst,
            dst_id,
        }
    }

    /// The parent commit this connection points at; `None` for synthetic connections with no concrete commit.
    pub fn dst_id(&self) -> Option<gix::ObjectId> {
        self.dst_id
    }

    /// The source commit id this connection emanates from, if it starts at a concrete commit.
    pub fn src_id(&self) -> Option<gix::ObjectId> {
        self.src_id
    }

    /// Re-point this connection at `dst_sidx` and re-normalize its endpoints to the source's last
    /// commit and the target's first, dropping commits that no longer exist after segment surgery.
    pub(crate) fn adjusted_for(
        mut self,
        src_sidx: SegmentIndex,
        dst_sidx: SegmentIndex,
        graph: &SegmentGraph,
    ) -> Self {
        self.target = dst_sidx;
        let commits = &graph[src_sidx].commits;
        let (id, idx) = commits
            .last()
            .map(|c| (Some(c.id), Some(commits.len() - 1)))
            .unwrap_or_default();
        self.src_id = id;
        self.src = idx;

        let commits = &graph[dst_sidx].commits;
        let (id, idx) = commits
            .first()
            .map(|c| (Some(c.id), Some(0)))
            .unwrap_or_default();
        self.dst_id = id;
        self.dst = idx;

        self
    }
}

/// A connection together with the segment it leaves, yielded while iterating a segment's edges.
///
/// Mirrors the accessor shape the call sites used when edges were a separate graph element, so they
/// read the same: `e.source()`, `e.target()`, `e.weight()`.
#[derive(Debug, Clone, Copy)]
pub struct EdgeRef<'a> {
    /// The segment this connection leaves.
    pub source: SegmentIndex,
    /// The segment this connection points to.
    pub target: SegmentIndex,
    /// The connection payload.
    pub weight: &'a Connection,
}

impl<'a> EdgeRef<'a> {
    pub fn source(&self) -> SegmentIndex {
        self.source
    }
    pub fn target(&self) -> SegmentIndex {
        self.target
    }
    pub fn weight(&self) -> &'a Connection {
        self.weight
    }
}

/// The segment arena. Removing a segment tombstones its slot, which is reused (LIFO) before a fresh
/// id is handed out, so segment ids stay stable across construction-time surgery.
#[derive(Debug, Clone, Default)]
pub struct SegmentGraph {
    segments: Vec<Option<Segment>>, // index = SegmentIndex
    free: Vec<SegmentIndex>,        // tombstoned slots, reused LIFO
}

impl SegmentGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of live (non-tombstoned) segments.
    pub fn node_count(&self) -> usize {
        self.segments.iter().filter(|n| n.is_some()).count()
    }

    /// Insert `segment` and return its id, reusing a tombstoned slot if available.
    /// The caller is responsible for having set `segment.id` to the returned id where it matters.
    pub fn add_node(&mut self, segment: Segment) -> SegmentIndex {
        if let Some(id) = self.free.pop() {
            self.segments[id] = Some(segment);
            id
        } else {
            let id = self.segments.len();
            self.segments.push(Some(segment));
            id
        }
    }

    /// The segment at `id`, if live.
    pub fn node(&self, id: SegmentIndex) -> Option<&Segment> {
        self.segments.get(id).and_then(Option::as_ref)
    }

    /// The segment at `id` mutably, if live.
    pub fn node_mut(&mut self, id: SegmentIndex) -> Option<&mut Segment> {
        self.segments.get_mut(id).and_then(Option::as_mut)
    }

    /// Alias for [`Self::node`].
    pub fn node_weight(&self, id: SegmentIndex) -> Option<&Segment> {
        self.node(id)
    }
    /// Alias for [`Self::node_mut`].
    pub fn node_weight_mut(&mut self, id: SegmentIndex) -> Option<&mut Segment> {
        self.node_mut(id)
    }

    /// Tombstone `id` and drop every connection incident to it (its own, and those pointing at it).
    pub fn remove_node(&mut self, id: SegmentIndex) -> Option<Segment> {
        let removed = self.segments.get_mut(id)?.take()?;
        for seg in self.segments.iter_mut().flatten() {
            seg.connections.retain(|c| c.target != id);
        }
        self.free.push(id);
        Some(removed)
    }

    /// Add a connection `source -> target`, appended to `source`'s connections.
    pub fn add_edge(&mut self, source: SegmentIndex, connection: Connection) {
        debug_assert!(
            self.node(source).is_some(),
            "connection source must be live"
        );
        debug_assert!(
            self.node(connection.target).is_some(),
            "connection target must be live"
        );
        self.segments[source]
            .as_mut()
            .expect("live source")
            .connections
            .push(connection);
    }

    /// Remove the first connection leaving `source` that equals `weight`, returning it.
    /// Connections have no global id; they are identified by source plus value.
    pub fn remove_edge(&mut self, source: SegmentIndex, weight: &Connection) -> Option<Connection> {
        let conns = &mut self.node_mut(source)?.connections;
        let pos = conns.iter().position(|c| c == weight)?;
        Some(conns.remove(pos))
    }

    /// Mutable access to the first connection leaving `source` that equals `weight`.
    pub fn edge_weight_mut(
        &mut self,
        source: SegmentIndex,
        weight: &Connection,
    ) -> Option<&mut Connection> {
        self.node_mut(source)?
            .connections
            .iter_mut()
            .find(|c| **c == *weight)
    }

    /// Live segment ids, ascending.
    pub fn node_ids(&self) -> impl Iterator<Item = SegmentIndex> + '_ {
        self.segments
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_ref().map(|_| i))
    }

    /// petgraph-compatible alias for [`Self::node_ids`].
    pub fn node_indices(&self) -> impl Iterator<Item = SegmentIndex> + '_ {
        self.node_ids()
    }

    /// Connections leaving (`Outgoing`, in stored first-parent order) or entering (`Incoming`,
    /// derived by scanning in ascending source order) `node`.
    pub fn edges_directed(
        &self,
        node: SegmentIndex,
        dir: Direction,
    ) -> impl Iterator<Item = EdgeRef<'_>> + '_ {
        // Collect so the two directions share a return type without boxing.
        let mut out = Vec::new();
        match dir {
            Direction::Outgoing => {
                if let Some(seg) = self.node(node) {
                    for c in &seg.connections {
                        out.push(EdgeRef {
                            source: node,
                            target: c.target,
                            weight: c,
                        });
                    }
                }
            }
            Direction::Incoming => {
                for src in self.node_ids() {
                    for c in &self.segments[src].as_ref().expect("live").connections {
                        if c.target == node {
                            out.push(EdgeRef {
                                source: src,
                                target: c.target,
                                weight: c,
                            });
                        }
                    }
                }
            }
        }
        out.into_iter()
    }

    /// Neighboring segment ids in the given direction.
    pub fn neighbors_directed(
        &self,
        node: SegmentIndex,
        dir: Direction,
    ) -> impl Iterator<Item = SegmentIndex> + '_ {
        self.edges_directed(node, dir)
            .map(move |e| match dir {
                Direction::Outgoing => e.target,
                Direction::Incoming => e.source,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Topological order (Kahn's algorithm): every connection's source precedes its target.
    /// Assumes a DAG; nodes left over after a cycle are omitted.
    pub fn toposort(&self) -> Vec<SegmentIndex> {
        let bound = self.segments.len();
        let mut in_degree = vec![0usize; bound];
        for src in self.node_ids() {
            for c in &self.segments[src].as_ref().expect("live").connections {
                in_degree[c.target] += 1;
            }
        }
        let mut queue: std::collections::VecDeque<SegmentIndex> =
            self.node_ids().filter(|&id| in_degree[id] == 0).collect();
        let mut order = Vec::with_capacity(self.node_count());
        while let Some(id) = queue.pop_front() {
            order.push(id);
            for c in &self.segments[id].as_ref().expect("live").connections {
                in_degree[c.target] -= 1;
                if in_degree[c.target] == 0 {
                    queue.push_back(c.target);
                }
            }
        }
        order
    }

    /// Live segments with no connections in `dir` — sinks for `Outgoing`, roots for `Incoming`.
    pub fn externals(&self, dir: Direction) -> impl Iterator<Item = SegmentIndex> + '_ {
        self.node_ids()
            .filter(move |&id| self.edges_directed(id, dir).next().is_none())
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Number of live connections.
    pub fn edge_count(&self) -> usize {
        self.segments
            .iter()
            .flatten()
            .map(|s| s.connections.len())
            .sum()
    }

    /// A node's outgoing connections.
    pub fn edges(&self, node: SegmentIndex) -> impl Iterator<Item = EdgeRef<'_>> + '_ {
        self.edges_directed(node, Direction::Outgoing)
    }

    /// All live connections, in ascending source order.
    pub fn edge_references(&self) -> impl Iterator<Item = EdgeRef<'_>> + '_ {
        let mut out = Vec::new();
        for src in self.node_ids() {
            for c in &self.segments[src].as_ref().expect("live").connections {
                out.push(EdgeRef {
                    source: src,
                    target: c.target,
                    weight: c,
                });
            }
        }
        out.into_iter()
    }

    /// Live connections from `source` to `target`, in stored order.
    pub fn edges_connecting(
        &self,
        source: SegmentIndex,
        target: SegmentIndex,
    ) -> impl Iterator<Item = EdgeRef<'_>> + '_ {
        self.edges_directed(source, Direction::Outgoing)
            .filter(move |e| e.target == target)
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// One past the largest segment id ever handed out (tombstoned slots included).
    pub fn node_bound(&self) -> usize {
        self.segments.len()
    }

    /// All live segments, ascending by id.
    pub fn node_weights(&self) -> impl Iterator<Item = &Segment> + '_ {
        self.segments.iter().filter_map(Option::as_ref)
    }

    /// All live segments mutably, ascending by id.
    pub fn node_weights_mut(&mut self) -> impl Iterator<Item = &mut Segment> + '_ {
        self.segments.iter_mut().filter_map(Option::as_mut)
    }

    /// Two distinct live segments, mutably.
    pub fn index_twice_mut(
        &mut self,
        a: SegmentIndex,
        b: SegmentIndex,
    ) -> (&mut Segment, &mut Segment) {
        assert_ne!(a, b, "index_twice_mut requires distinct segments");
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let (left, right) = self.segments.split_at_mut(hi);
        let lo_ref = left[lo].as_mut().expect("live segment");
        let hi_ref = right[0].as_mut().expect("live segment");
        if a < b {
            (lo_ref, hi_ref)
        } else {
            (hi_ref, lo_ref)
        }
    }
}

impl std::ops::Index<SegmentIndex> for SegmentGraph {
    type Output = Segment;
    fn index(&self, id: SegmentIndex) -> &Segment {
        self.node(id).expect("segment id refers to a live segment")
    }
}

impl std::ops::IndexMut<SegmentIndex> for SegmentGraph {
    fn index_mut(&mut self, id: SegmentIndex) -> &mut Segment {
        self.node_mut(id)
            .expect("segment id refers to a live segment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Segment;

    fn conn(target: SegmentIndex) -> Connection {
        Connection::new(target, None, None, None, None)
    }

    #[test]
    fn add_remove_and_lifo_id_reuse() {
        let mut g = SegmentGraph::new();
        let a = g.add_node(Segment::default());
        let b = g.add_node(Segment::default());
        assert_eq!(g.node_count(), 2);
        assert!(g.remove_node(a).is_some());
        assert_eq!(g.node_count(), 1);
        // The freed slot is reused before a fresh id is handed out.
        let c = g.add_node(Segment::default());
        assert_eq!(c, a, "freed slot reused");
        assert!(g.node(b).is_some(), "other ids stay stable");
    }

    #[test]
    fn outgoing_is_stored_order_and_incoming_is_scanned() {
        let mut g = SegmentGraph::new();
        let a = g.add_node(Segment::default());
        let b = g.add_node(Segment::default());
        let c = g.add_node(Segment::default());
        g.add_edge(a, conn(b));
        g.add_edge(a, conn(c));
        // Outgoing keeps insertion (stored) order — no newest-first reversal.
        assert_eq!(
            g.neighbors_directed(a, Direction::Outgoing)
                .collect::<Vec<_>>(),
            vec![b, c]
        );
        assert_eq!(
            g.neighbors_directed(c, Direction::Incoming)
                .collect::<Vec<_>>(),
            vec![a]
        );
        assert_eq!(g.edge_count(), 2);
    }

    #[test]
    fn remove_node_drops_incident_connections() {
        let mut g = SegmentGraph::new();
        let a = g.add_node(Segment::default());
        let b = g.add_node(Segment::default());
        let c = g.add_node(Segment::default());
        g.add_edge(a, conn(b));
        g.add_edge(b, conn(c));
        g.remove_node(b);
        assert_eq!(g.neighbors_directed(a, Direction::Outgoing).count(), 0);
        assert_eq!(g.neighbors_directed(c, Direction::Incoming).count(), 0);
        assert!(g.node(b).is_none());
    }

    #[test]
    fn remove_edge_by_value() {
        let mut g = SegmentGraph::new();
        let a = g.add_node(Segment::default());
        let b = g.add_node(Segment::default());
        let e = conn(b);
        g.add_edge(a, e);
        assert_eq!(g.edge_count(), 1);
        assert!(g.remove_edge(a, &e).is_some());
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn toposort_orders_sources_before_targets() {
        // Diamond: a -> b, a -> c, b -> d, c -> d.
        let mut g = SegmentGraph::new();
        let a = g.add_node(Segment::default());
        let b = g.add_node(Segment::default());
        let c = g.add_node(Segment::default());
        let d = g.add_node(Segment::default());
        g.add_edge(a, conn(b));
        g.add_edge(a, conn(c));
        g.add_edge(b, conn(d));
        g.add_edge(c, conn(d));
        let order = g.toposort();
        assert_eq!(order.len(), 4);
        let pos = |n: SegmentIndex| order.iter().position(|&x| x == n).unwrap();
        assert!(pos(a) < pos(b) && pos(a) < pos(c) && pos(b) < pos(d) && pos(c) < pos(d));
    }

    #[test]
    fn externals_finds_roots_and_sinks() {
        let mut g = SegmentGraph::new();
        let a = g.add_node(Segment::default());
        let b = g.add_node(Segment::default());
        let c = g.add_node(Segment::default());
        g.add_edge(a, conn(b));
        g.add_edge(b, conn(c));
        assert_eq!(
            g.externals(Direction::Incoming).collect::<Vec<_>>(),
            vec![a]
        );
        assert_eq!(
            g.externals(Direction::Outgoing).collect::<Vec<_>>(),
            vec![c]
        );
    }
}
