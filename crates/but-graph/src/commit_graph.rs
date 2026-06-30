//! SPIKE (commit-graph-experiment): a commit-first graph that both the display projection and
//! but-rebase's `StepGraph` could be built from directly, so the segment graph can be deleted.
//!
//! # Why
//!
//! Today the pipeline is `gix traversal → SegmentGraph (segments own commit ranges) → projection`,
//! and but-rebase builds its `StepGraph` from that same segment graph. The segment layer turned out
//! to be an *artifact of incremental construction*, not something either consumer fundamentally
//! needs:
//!
//! * **StepGraph** is already commit/ref-granular — its nodes are `Pick(commit)` / `Reference(ref)`
//!   and its edges carry only the parent-array `order`. It re-derives parent order from
//!   `commit.parent_ids` and even *corrects* but-graph when they disagree. It needs: commit id,
//!   parent ids (first-parent at `[0]`), the refs on each commit, an entrypoint, and parent-walk
//!   reachability. No segment boundaries, no segment ids.
//!
//! * **Projection** emits segment-shaped output (`Stack`/`StackSegment`), but the segmentation is
//!   recomputable: a segment is a maximal first-parent run, split where a local-branch ref appears,
//!   at branch/merge points, and at the projection's own stops (entrypoint, merge-base, target).
//!   `generation`, merge-base, and remote-reachability are commit-level. `sibling_segment_id` /
//!   `remote_tracking_branch_segment_id` are just cached pointers — recomputable by ref-name match.
//!
//! So both can build straight from the commit DAG, and segments become a *view* produced during
//! projection rather than a stored graph.
//!
//! # The model
//!
//! A node is a commit (we reuse [`crate::Commit`]) plus its topological `generation`. An edge is
//! simply `commit → parent`, taken from `parent_ids` (first-parent at index 0) — there is no
//! `src`/`dst` within-segment payload to carry, because there are no segments to index into.
//!
//! A nice consequence: the **workspace commit's `parent_ids` array is the stack order**, so the
//! order-stacks machinery ([`crate::Graph`] post-pass) disappears — the order is read straight off
//! the merge commit's parents.
//!
//! This module is a standalone spike: it is not wired into traversal or the projection yet. It
//! defines the structure and proves the load-bearing derivation (first-parent segmentation) so we
//! can validate the shape before swapping any behavior.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::{Commit, CommitFlags};

/// An index into a [`CommitGraph`]'s node arena.
pub type CommitIdx = usize;

/// A node in the commit graph: a commit, plus where it sits topologically.
#[derive(Debug, Clone)]
pub struct CommitNode {
    /// The commit itself — `id`, `parent_ids` (first-parent at `[0]`), `flags`, and the `refs`
    /// pointing at it. This is exactly the data a segment used to hold per-commit.
    pub commit: Commit,
    /// Distance from a root (a commit with no parents in the graph). Higher means deeper in history.
    /// Used where the projection picks "the lowest of several tips"; cheap to compute during build.
    pub generation: u32,
}

/// A commit-first graph: an arena of commits keyed by id, with `commit → parent` edges read from
/// each node's `parent_ids` and the reverse (`parent → child`) adjacency derived for downward walks.
#[derive(Debug, Clone, Default)]
pub struct CommitGraph {
    nodes: Vec<CommitNode>,
    by_id: HashMap<gix::ObjectId, CommitIdx>,
    /// `parent → children` adjacency, derived at build time so we can detect branch points and walk
    /// downward (the projection walks from the workspace tip toward the base).
    children: Vec<Vec<CommitIdx>>,
    /// Where traversal/HEAD started; the projection uses it as a focus boundary.
    entrypoint: Option<gix::ObjectId>,
}

impl CommitGraph {
    /// Build from a set of commits (as produced by the gix traversal). Commits whose parents are
    /// outside the set are simply roots of this subgraph (a partial graph), mirroring how the
    /// StepGraph handles missing parents via `preserved_parents`.
    pub fn from_commits(commits: impl IntoIterator<Item = Commit>, entrypoint: Option<gix::ObjectId>) -> Self {
        let nodes: Vec<CommitNode> = commits
            .into_iter()
            .map(|commit| CommitNode {
                commit,
                generation: 0,
            })
            .collect();
        let by_id: HashMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(idx, n)| (n.commit.id, idx))
            .collect();

        // Reverse adjacency: for each node, record it as a child of every parent that is present.
        let mut children = vec![Vec::new(); nodes.len()];
        for (idx, n) in nodes.iter().enumerate() {
            for parent in &n.commit.parent_ids {
                if let Some(&pidx) = by_id.get(parent) {
                    children[pidx].push(idx);
                }
            }
        }

        let mut graph = CommitGraph {
            nodes,
            by_id,
            children,
            entrypoint,
        };
        graph.recompute_generations();
        graph
    }

    /// The node at `id`, if present.
    pub fn node(&self, id: gix::ObjectId) -> Option<&CommitNode> {
        self.by_id.get(&id).map(|&idx| &self.nodes[idx])
    }

    /// The parents of `id` that are present in this graph, first-parent first.
    pub fn parents(&self, id: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.node(id)
            .into_iter()
            .flat_map(|n| n.commit.parent_ids.iter().copied())
            .filter(|p| self.by_id.contains_key(p))
    }

    /// The first parent of `id` (the next commit walking down first-parent), if present.
    pub fn first_parent(&self, id: gix::ObjectId) -> Option<gix::ObjectId> {
        let n = self.node(id)?;
        n.commit.parent_ids.first().copied().filter(|p| self.by_id.contains_key(p))
    }

    /// The children of `id` (commits that list `id` as a parent). More than one means a branch point.
    pub fn children(&self, id: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.by_id
            .get(&id)
            .into_iter()
            .flat_map(move |&idx| self.children[idx].iter().map(|&c| self.nodes[c].commit.id))
    }

    /// Whether walking first-parent should *stop* before entering `id` — i.e. `id` begins a new
    /// segment. Structural boundaries only (the projection layers its own: entrypoint, merge-base,
    /// target). A new segment begins where:
    /// * a local branch ref points at the commit (a named segment starts), or
    /// * the commit is a merge (more than one parent), or
    /// * the commit is a branch point (more than one child) — the paths are distinct segments.
    pub fn is_segment_boundary(&self, id: gix::ObjectId) -> bool {
        let Some(n) = self.node(id) else {
            return false;
        };
        let has_local_branch_ref = n.commit.ref_name_iter().any(|rn| {
            rn.category() == Some(gix::reference::Category::LocalBranch)
        });
        let is_merge = n.commit.parent_ids.len() > 1;
        let is_branch_point = self.children(id).take(2).count() > 1;
        has_local_branch_ref || is_merge || is_branch_point
    }

    /// Derive one segment's commits: the maximal first-parent run starting at `start` and continuing
    /// while the next first-parent commit is not itself a boundary. This is the grouping the segment
    /// graph used to store, recomputed on demand — the proof that segments are a *view*.
    pub fn first_parent_run(&self, start: gix::ObjectId) -> Vec<gix::ObjectId> {
        let mut run = Vec::new();
        let mut cur = Some(start);
        while let Some(id) = cur {
            run.push(id);
            match self.first_parent(id) {
                Some(next) if !self.is_segment_boundary(next) => cur = Some(next),
                _ => break,
            }
        }
        run
    }

    /// Recompute `generation` for every node (longest path from a root, by Kahn order). Cheap; the
    /// graph is small.
    fn recompute_generations(&mut self) {
        // Process in topological order (parents before children) so a child's generation is the max
        // over its present parents + 1.
        let order = self.toposort_parents_first();
        for id in order {
            let idx = self.by_id[&id];
            let generation = self.nodes[idx]
                .commit
                .parent_ids
                .iter()
                .filter_map(|p| self.by_id.get(p))
                .map(|&pidx| self.nodes[pidx].generation + 1)
                .max()
                .unwrap_or(0);
            self.nodes[idx].generation = generation;
        }
    }

    /// Topological order with parents before children (history order).
    fn toposort_parents_first(&self) -> Vec<gix::ObjectId> {
        let mut indegree = vec![0usize; self.nodes.len()];
        for (idx, n) in self.nodes.iter().enumerate() {
            indegree[idx] = n
                .commit
                .parent_ids
                .iter()
                .filter(|p| self.by_id.contains_key(*p))
                .count();
        }
        let mut queue: std::collections::VecDeque<CommitIdx> = (0..self.nodes.len())
            .filter(|&i| indegree[i] == 0)
            .collect();
        let mut out = Vec::with_capacity(self.nodes.len());
        while let Some(idx) = queue.pop_front() {
            out.push(self.nodes[idx].commit.id);
            for &child in &self.children[idx] {
                indegree[child] -= 1;
                if indegree[child] == 0 {
                    queue.push_back(child);
                }
            }
        }
        out
    }

    /// Commits carrying the in-workspace flag — a stand-in for the kind of flag-based query both
    /// consumers do instead of asking "which segment owns this".
    pub fn in_workspace(&self) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.nodes
            .iter()
            .filter(|n| n.commit.flags.contains(CommitFlags::InWorkspace))
            .map(|n| n.commit.id)
    }
}

// ---------------------------------------------------------------------------------------------
// Builder sketches (not implemented in this spike — they live in but-graph and but-rebase
// respectively and would replace the segment-graph-based construction).
//
// Projection (in but-graph, replaces projection/workspace/init.rs's segment walk):
//
//   fn project(g: &CommitGraph, meta: &Workspace) -> Workspace {
//       // 1. entrypoint = g.entrypoint; resolve workspace tip commit.
//       // 2. stack tops = g.parents(workspace_commit) IN ORDER  ← stack order is free here.
//       // 3. for each top: segments = repeatedly first_parent_run(...) down to lower_bound,
//       //    splitting at is_segment_boundary plus projection stops (entrypoint, merge-base, target).
//       // 4. StackSegment fields come straight off the run's commits + their refs; sibling /
//       //    remote-tracking links are looked up by ref name instead of stored segment ids.
//       // 5. enrich: prune integrated/archived, mark remote-reachable via flag walks.
//   }
//
// StepGraph (in but-rebase, replaces graph_rebase/creation.rs's segment iteration):
//
//   fn build_steps(g: &CommitGraph, entry: ObjectId) -> StepGraph {
//       // 1. walk reachable commits from entry via g.parents().
//       // 2. one Pick node per commit, one Reference node per ref on it.
//       // 3. edges: for each commit, connect to each parent with order = parent index — read
//       //    directly off commit.parent_ids (no Connection.src_id/dst_id lookup needed).
//   }
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommitFlags;

    fn id(b: u8) -> gix::ObjectId {
        let mut bytes = [0u8; 20];
        bytes[0] = b;
        gix::ObjectId::from_bytes_or_panic(&bytes)
    }

    fn commit(b: u8, parents: &[u8]) -> Commit {
        Commit {
            id: id(b),
            parent_ids: parents.iter().map(|&p| id(p)).collect(),
            flags: CommitFlags::empty(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn children_generation_and_first_parent_walk() {
        // Linear: 3 -> 2 -> 1 (child -> parent).
        let g = CommitGraph::from_commits([commit(3, &[2]), commit(2, &[1]), commit(1, &[])], Some(id(3)));
        assert_eq!(g.first_parent(id(3)), Some(id(2)));
        assert_eq!(g.first_parent(id(1)), None);
        assert_eq!(g.children(id(1)).collect::<Vec<_>>(), vec![id(2)]);
        // Generation increases with history depth.
        assert_eq!(g.node(id(1)).unwrap().generation, 0);
        assert_eq!(g.node(id(3)).unwrap().generation, 2);
        // No boundaries on a plain linear chain → the whole thing is one run.
        assert_eq!(g.first_parent_run(id(3)), vec![id(3), id(2), id(1)]);
    }

    #[test]
    fn merge_is_a_segment_boundary_so_the_run_stops() {
        // 4 is a merge of 2 and 3; both descend from 1.
        //   4 -> [2, 3] ; 2 -> 1 ; 3 -> 1
        let g = CommitGraph::from_commits(
            [commit(4, &[2, 3]), commit(2, &[1]), commit(3, &[1]), commit(1, &[])],
            Some(id(4)),
        );
        assert!(g.is_segment_boundary(id(4)), "merge commit starts its own segment");
        assert!(
            g.is_segment_boundary(id(1)),
            "1 has two children (2 and 3) → branch point, a boundary"
        );
        // First-parent run from the merge: 4, then first-parent 2, then stop before boundary 1.
        assert_eq!(g.first_parent_run(id(4)), vec![id(4), id(2)]);
    }
}
