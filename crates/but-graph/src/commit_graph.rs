//! A commit-node graph: one node per commit, with edges pointing from a commit to
//! each of its parents and carrying only the parent's position.
//!
//! The commit-first traversal ([`crate::init`]) builds this directly as it visits commits, and the
//! [`Workspace`](crate::Workspace) projection carries it. Merge-base and reachability are built on
//! it; the crate's `commit_graph` tests pin it as a faithful, lossless view of the commit topology
//! (commits, first-parent chains, and traversal stops).

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, HashSet},
};

use petgraph::{
    Direction,
    stable_graph::{NodeIndex, StableGraph},
    visit::{EdgeRef, IntoEdgeReferences, Topo},
};

use crate::{Commit, CommitFlags};

/// An edge from a child commit to one of its parents.
#[derive(Debug, Clone, Copy)]
pub struct ParentEdge {
    /// The 0-based position of the parent among the child's parents. `0` is the first parent.
    pub parent_order: u32,
}

/// A commit graph: nodes are commits, edges point from a commit to each of its in-graph parents.
///
/// Built directly by the commit-first traversal (see [`crate::init`]) and carried on the
/// [`Workspace`](crate::Workspace) projection.
#[derive(Debug, Clone, Default)]
pub struct CommitGraph {
    /// The underlying petgraph; edges point child → parent.
    pub inner: StableGraph<Commit, ParentEdge>,
    by_id: HashMap<gix::ObjectId, NodeIndex>,
}

/// Incremental construction, used by the commit-first traversal which builds this graph directly
/// instead of deriving it from segments.
impl CommitGraph {
    /// Insert `node`, assuming `node.id` is not yet present.
    pub(crate) fn add_commit(&mut self, node: Commit) -> NodeIndex {
        let id = node.id;
        let nx = self.inner.add_node(node);
        self.by_id.insert(id, nx);
        nx
    }

    /// Add the edge `child → parent` with `parent_order`. Both commits must be present.
    pub(crate) fn add_parent_edge(
        &mut self,
        child: gix::ObjectId,
        parent: gix::ObjectId,
        parent_order: u32,
    ) {
        let (c, p) = (self.by_id[&child], self.by_id[&parent]);
        self.inner.add_edge(c, p, ParentEdge { parent_order });
    }

    /// The flags of `id`, if present.
    pub(crate) fn flags_of(&self, id: gix::ObjectId) -> Option<CommitFlags> {
        self.node(id).map(|nx| self.inner[nx].flags)
    }

    /// `OR` `flags` into the commit `id`.
    pub(crate) fn add_flags(&mut self, id: gix::ObjectId, flags: CommitFlags) {
        let nx = self.by_id[&id];
        self.inner[nx].flags |= flags;
    }

    /// The node data for `id`, which must be present.
    pub(crate) fn node_data(&self, id: gix::ObjectId) -> &Commit {
        &self.inner[self.by_id[&id]]
    }

    /// All ancestor commit ids of `id` (including `id`), walking parent edges.
    pub fn ancestor_ids(&self, id: gix::ObjectId) -> std::collections::HashSet<gix::ObjectId> {
        self.node(id)
            .map(|nx| {
                self.ancestors(nx)
                    .into_iter()
                    .map(|nx| self.inner[nx].id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The in-graph first parent of `id`, by id.
    pub(crate) fn first_parent_id(&self, id: gix::ObjectId) -> Option<gix::ObjectId> {
        self.node(id)
            .and_then(|nx| self.first_parent(nx))
            .map(|p| self.inner[p].id)
    }

    /// Whether `sought` lies on the first-parent line starting at `start` (inclusive), walking
    /// first-parent edges toward history.
    pub fn first_parent_reaches(&self, start: gix::ObjectId, sought: gix::ObjectId) -> bool {
        let mut cur = Some(start);
        while let Some(id) = cur {
            if id == sought {
                return true;
            }
            cur = self.first_parent_id(id);
        }
        false
    }
}

impl CommitGraph {
    /// Return the node for `id`, if the commit is part of this graph.
    pub fn node(&self, id: gix::ObjectId) -> Option<NodeIndex> {
        self.by_id.get(&id).copied()
    }

    /// Return the commit for `id`, if it is part of this graph.
    pub fn commit(&self, id: gix::ObjectId) -> Option<&Commit> {
        self.node(id).map(|nx| &self.inner[nx])
    }

    /// The in-graph parents of `id` (toward history), by id.
    pub fn parent_ids(&self, id: gix::ObjectId) -> Vec<gix::ObjectId> {
        self.node(id)
            .map(|nx| self.parents(nx).map(|(p, _)| self.inner[p].id).collect())
            .unwrap_or_default()
    }

    /// The number of commit nodes in the graph.
    pub fn num_commits(&self) -> usize {
        self.inner.node_count()
    }

    /// The in-graph parents of `nx` (toward history), each with its parent order.
    pub fn parents(&self, nx: NodeIndex) -> impl Iterator<Item = (NodeIndex, u32)> + '_ {
        self.inner
            .edges_directed(nx, Direction::Outgoing)
            .map(|edge| (edge.target(), edge.weight().parent_order))
    }

    /// Every child→parent edge as `(child id, parent id, parent order)`, in the insertion order the
    /// walk established them — the canonical record of cross-run connections for the projection.
    pub(crate) fn parent_edges(
        &self,
    ) -> impl Iterator<Item = (gix::ObjectId, gix::ObjectId, u32)> + '_ {
        self.inner.edge_references().map(move |edge| {
            (
                self.inner[edge.source()].id,
                self.inner[edge.target()].id,
                edge.weight().parent_order,
            )
        })
    }

    /// The children of `nx`, i.e. the in-graph commits that list `nx` as a parent.
    pub fn children(&self, nx: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
        self.inner
            .edges_directed(nx, Direction::Incoming)
            .map(|edge| edge.source())
    }

    /// The merge-base of commits `a` and `b`: their lowest common ancestor in the commit graph, or
    /// `None` if they share no history (or either is absent).
    ///
    /// "Lowest" means closest to the tips — the common ancestor of least topological depth, with
    /// id used as a deterministic tie-break for the criss-cross case. Returns `a` if `a == b`.
    pub fn merge_base(&self, a: gix::ObjectId, b: gix::ObjectId) -> Option<gix::ObjectId> {
        let (na, nb) = (self.node(a)?, self.node(b)?);
        if na == nb {
            return Some(a);
        }
        let generations = self.generations();
        let ancestors_a = self.ancestors(na);
        self.ancestors(nb)
            .into_iter()
            .filter(|nx| ancestors_a.contains(nx))
            .min_by_key(|nx| (generations[nx], self.inner[*nx].id))
            .map(|nx| self.inner[nx].id)
    }

    /// All ancestors of `start` (including `start`), walking parent edges.
    fn ancestors(&self, start: NodeIndex) -> HashSet<NodeIndex> {
        self.ancestors_with(start, false)
    }

    /// All ancestors of `start` (including `start`). If `first_parent`, follows only the
    /// `parent_order == 0` edge at each step.
    fn ancestors_with(&self, start: NodeIndex, first_parent: bool) -> HashSet<NodeIndex> {
        let mut seen = HashSet::new();
        let mut stack = vec![start];
        while let Some(nx) = stack.pop() {
            if seen.insert(nx) {
                for (parent, order) in self.parents(nx) {
                    if first_parent && order != 0 {
                        continue;
                    }
                    stack.push(parent);
                }
            }
        }
        seen
    }

    /// Commit ids reachable from `a` but not from `b`, i.e. the set difference `b..a`.
    ///
    /// Ordered tip-toward-history: ascending topological generation, ties broken by discovery
    /// order with first-parents visited first — so within a linear run commits come out
    /// tip-to-base. When `first_parent` is set, both the included and excluded sides follow only
    /// `parent_order == 0` edges. Empty when `a == b` or either commit is absent.
    ///
    /// This matches the former segment-level `Graph::find_segments_reachable_from_a_not_b` flattened
    /// to commits: inter-segment edges always land on a segment's first commit, so a segment is
    /// reachable from one side exactly when all of its commits are, making the segment-set difference
    /// and this commit-set difference identical.
    pub fn commits_reachable_from_a_not_b(
        &self,
        a: gix::ObjectId,
        b: gix::ObjectId,
        first_parent: bool,
    ) -> Vec<gix::ObjectId> {
        let (Some(na), Some(nb)) = (self.node(a), self.node(b)) else {
            return Vec::new();
        };
        // If a commit is reachable from `b`, so are all of its ancestors — so we can treat the
        // whole excluded ancestry as hidden and prune there.
        let excluded = self.ancestors_with(nb, first_parent);

        let generations = self.generations();
        let mut heap: BinaryHeap<(Reverse<usize>, Reverse<usize>, NodeIndex)> = BinaryHeap::new();
        let mut queued = HashSet::new();
        let mut sequence = 0;
        heap.push((Reverse(generations[&na]), Reverse(sequence), na));
        queued.insert(na);
        sequence += 1;

        let mut out = Vec::new();
        while let Some((_, _, nx)) = heap.pop() {
            if excluded.contains(&nx) {
                continue;
            }
            out.push(self.inner[nx].id);
            // Push parents in `parent_order` so the discovery-order tie-break is deterministic and
            // first-parents precede later parents at the same generation.
            let mut parents: Vec<_> = self.parents(nx).collect();
            parents.sort_by_key(|(_, order)| *order);
            for (parent, order) in parents {
                if first_parent && order != 0 {
                    continue;
                }
                if queued.insert(parent) {
                    heap.push((Reverse(generations[&parent]), Reverse(sequence), parent));
                    sequence += 1;
                }
            }
        }
        out
    }

    /// Topological depth per commit: tips are `0`, increasing toward history. A commit is processed
    /// after all of its children, so its depth is one past the deepest child.
    pub(crate) fn generations(&self) -> HashMap<NodeIndex, usize> {
        let mut generations = HashMap::new();
        let mut topo = Topo::new(&self.inner);
        while let Some(nx) = topo.next(&self.inner) {
            let generation = self
                .inner
                .edges_directed(nx, Direction::Incoming)
                .map(|edge| generations.get(&edge.source()).copied().unwrap_or(0) + 1)
                .max()
                .unwrap_or(0);
            generations.insert(nx, generation);
        }
        generations
    }

    /// [`Self::generations`] keyed by commit id, for consumers that address commits by id.
    pub(crate) fn generation_by_commit_id(&self) -> HashMap<gix::ObjectId, usize> {
        self.generations()
            .into_iter()
            .map(|(nx, g)| (self.inner[nx].id, g))
            .collect()
    }

    /// The in-graph first parent of `nx` (the `parent_order == 0` edge), if any.
    pub(crate) fn first_parent(&self, nx: NodeIndex) -> Option<NodeIndex> {
        self.inner
            .edges_directed(nx, Direction::Outgoing)
            .find(|edge| edge.weight().parent_order == 0)
            .map(|edge| edge.target())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Commit;

    /// A stable, readable object id derived from a small index (`1` → `0000…0001`).
    fn oid(n: usize) -> gix::ObjectId {
        gix::ObjectId::from_hex(format!("{n:040x}").as_bytes()).expect("valid hex")
    }

    /// Build a [`CommitGraph`] from a concise DAG: one `(id, &[parents])` per commit, parents
    /// listed before children. The first listed parent is parent 0. Returns the graph plus a
    /// lookup from string id to object id. No flags or refs — purely topological, for testing the
    /// graph-query algorithms without git fixtures.
    fn dag(commits: &[(&str, &[&str])]) -> (CommitGraph, impl Fn(&str) -> gix::ObjectId) {
        // Assign a stable id to every commit and parent name, in first-seen order.
        let mut ids: HashMap<String, gix::ObjectId> = HashMap::new();
        for &(id, parents) in commits {
            for name in std::iter::once(id).chain(parents.iter().copied()) {
                let n = ids.len() + 1;
                ids.entry(name.to_string()).or_insert_with(|| oid(n));
            }
        }
        let mut cg = CommitGraph::default();
        for &(id, parents) in commits {
            cg.add_commit(Commit {
                id: ids[id],
                parent_ids: parents.iter().map(|p| ids[*p]).collect(),
                flags: CommitFlags::empty(),
                refs: Vec::new(),
            });
        }
        for &(id, parents) in commits {
            for (order, &p) in parents.iter().enumerate() {
                cg.add_parent_edge(ids[id], ids[p], order as u32);
            }
        }
        (cg, move |s: &str| ids[s])
    }

    #[test]
    fn merge_base_is_the_fork_point() {
        // a ─┬─ b
        //    └─ c
        let (cg, id) = dag(&[("a", &[]), ("b", &["a"]), ("c", &["a"])]);
        assert_eq!(cg.merge_base(id("b"), id("c")), Some(id("a")));
        assert_eq!(cg.merge_base(id("b"), id("a")), Some(id("a"))); // a is b's ancestor
        assert_eq!(cg.merge_base(id("a"), id("a")), Some(id("a"))); // self
    }

    #[test]
    fn merge_base_of_disjoint_histories_is_none() {
        let (cg, id) = dag(&[("a", &[]), ("x", &[])]);
        assert_eq!(cg.merge_base(id("a"), id("x")), None);
    }

    #[test]
    fn ancestor_ids_collects_the_reachable_subgraph() {
        // a <- b <- c, with d branching off a (not an ancestor of c)
        let (cg, id) = dag(&[("a", &[]), ("b", &["a"]), ("c", &["b"]), ("d", &["a"])]);
        assert_eq!(
            cg.ancestor_ids(id("c")),
            HashSet::from([id("a"), id("b"), id("c")])
        );
    }

    #[test]
    fn first_parent_reaches_follows_only_first_parents() {
        // m is a merge: first parent b, second parent c
        let (cg, id) = dag(&[("a", &[]), ("b", &["a"]), ("c", &["a"]), ("m", &["b", "c"])]);
        assert!(cg.first_parent_reaches(id("m"), id("m"))); // self
        assert!(cg.first_parent_reaches(id("m"), id("b"))); // first parent
        assert!(cg.first_parent_reaches(id("m"), id("a"))); // first-parent chain m→b→a
        assert!(!cg.first_parent_reaches(id("m"), id("c"))); // reachable only via the second parent
    }

    #[test]
    fn commits_reachable_from_a_not_b_excludes_b_ancestry() {
        let (cg, id) = dag(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        // Reachable from c but not from b: just c (b and a are b's ancestry).
        assert_eq!(
            cg.commits_reachable_from_a_not_b(id("c"), id("b"), false),
            vec![id("c")]
        );
    }
}
