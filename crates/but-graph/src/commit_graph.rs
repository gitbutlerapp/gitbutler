//! A commit-first graph flattened out of the raw traversal — the substrate every
//! [`Graph`](crate::Graph) is built from (see `commit_graph_to_segment_graph`).
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
//! Historically this was a standalone spike toward deleting the segment graph outright; today the
//! production builders source it from the REAL traversal ([`CommitGraph::from_walk`]) and rebuild
//! the full segment graph on top, so downstream consumers are unchanged. The commit-first model
//! remains the intended shape for the eventual but-graph/but-rebase unification.

use std::collections::{HashMap, HashSet};

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
    /// The ref the entrypoint was checked out as, if any. When set, it names the entrypoint segment
    /// (overriding disambiguation), mirroring `from_commit_traversal(id, Some(ref))`.
    entrypoint_ref: Option<gix::refs::FullName>,
    /// Commits whose message marks them as a GitButler-managed workspace commit. Kept out of
    /// [`CommitFlags`](crate::CommitFlags) so it neither perturbs the walk's goal bits nor the
    /// segment fingerprint; used to tell a real managed merge from a ws ref advanced past it.
    managed_ws_commits: HashSet<gix::ObjectId>,
    /// `(child, parent)` pairs the traversal actually CONNECTED, when built
    /// [from the walk](Self::from_walk). A commit's raw `parent_ids` can point past a traversal
    /// cut (limit, integrated stop-early); connectivity accessors must not rejoin what the walk
    /// severed. `None` for graphs built directly from commits (all raw parents count).
    connected: Option<HashSet<(gix::ObjectId, gix::ObjectId)>>,
    /// When built [from the walk](Self::from_walk): whether the traversal stopped queueing after
    /// hitting the hard limit. Derived graphs must carry it onto the final `Graph`.
    pub(crate) hard_limit_hit: bool,
    /// When built [from the walk](Self::from_walk): the traversal's normalized seed tips. Graphs
    /// built from EXPLICIT tips must carry them onto the final `Graph` — the projection reads tip
    /// roles (e.g. integrated tips) for such graphs.
    pub(crate) traversal_tips: Vec<crate::init::Tip>,
    /// Built from EXPLICIT tips ([`Self::from_walk_tips`]): every tip must start (or get) its own
    /// segment. Workspace-discovered builds must NOT carve boundaries at their normalized tips —
    /// there they are ordinary interior commits unless the plan makes them boundaries.
    pub(crate) explicit_tips: bool,
}

impl CommitGraph {
    /// Build from a set of commits (as produced by the gix traversal). Commits whose parents are
    /// outside the set are simply roots of this subgraph (a partial graph), mirroring how the
    /// StepGraph handles missing parents via `preserved_parents`.
    pub(crate) fn from_commits(
        commits: impl IntoIterator<Item = Commit>,
        entrypoint: Option<gix::ObjectId>,
    ) -> Self {
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
            entrypoint_ref: None,
            managed_ws_commits: HashSet::new(),
            connected: None,
            hard_limit_hit: false,
            traversal_tips: Vec::new(),
            explicit_tips: false,
        };
        graph.recompute_generations();
        graph
    }

    /// Restrict connectivity to the given `(child, parent)` pairs and rebuild the child adjacency
    /// accordingly. See the `connected` field.
    fn set_connected(&mut self, connected: HashSet<(gix::ObjectId, gix::ObjectId)>) {
        for children in &mut self.children {
            children.clear();
        }
        for idx in 0..self.nodes.len() {
            let id = self.nodes[idx].commit.id;
            for pos in 0..self.nodes[idx].commit.parent_ids.len() {
                let parent = self.nodes[idx].commit.parent_ids[pos];
                if connected.contains(&(id, parent))
                    && let Some(&pidx) = self.by_id.get(&parent)
                {
                    self.children[pidx].push(idx);
                }
            }
        }
        self.connected = Some(connected);
        self.recompute_generations();
    }

    /// Is the `child → parent` link one the traversal actually followed?
    fn is_connected(&self, child: gix::ObjectId, parent: gix::ObjectId) -> bool {
        self.connected
            .as_ref()
            .is_none_or(|c| c.contains(&(child, parent)))
    }

    /// Assemble from the NATIVE traversal outcome (see `init::native_walk`).
    pub(crate) fn from_native_outcome(o: crate::init::native_walk::NativeOutcome) -> Self {
        let mut cg = CommitGraph::from_commits(o.commits, o.entrypoint);
        cg.entrypoint_ref = o.entrypoint_ref;
        cg.set_connected(o.connected);
        cg.hard_limit_hit = o.hard_limit_hit;
        cg.traversal_tips = o.tips;
        cg
    }

    /// Build by running the real traversal (queue, goals, limits, flag propagation), accumulating
    /// commits directly. This keeps the battle-tested traversal semantics — extents (limit cuts,
    /// integrated stop-early) and flags — while segments remain a derived view built on top.
    pub(crate) fn from_walk<T: but_core::RefMetadata>(
        repo: &gix::Repository,
        meta: &T,
        tip: gix::ObjectId,
        ref_name: Option<gix::refs::FullName>,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::init::Options,
        overlay: crate::init::Overlay,
    ) -> anyhow::Result<Self> {
        let native =
            Self::from_native_outcome(crate::Graph::native_from_commit_traversal_with_overlay(
                repo,
                tip,
                ref_name.clone(),
                meta,
                project_meta.clone(),
                options,
                overlay,
            )?);
        Ok(native)
    }

    /// Like [`Self::from_walk`], but seeded from explicit `tips`.
    pub(crate) fn from_walk_tips<T: but_core::RefMetadata>(
        repo: &gix::Repository,
        meta: &T,
        tips: Vec<crate::init::Tip>,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::init::Options,
        overlay: crate::init::Overlay,
    ) -> anyhow::Result<Self> {
        let mut native = Self::from_native_outcome(
            crate::Graph::native_from_commit_traversal_tips_with_overlay(
                repo,
                tips.clone(),
                meta,
                project_meta.clone(),
                options,
                overlay,
            )?,
        );
        native.explicit_tips = true;
        Ok(native)
    }

    /// Mark `id` as a GitButler-managed workspace commit when its message says so.
    pub(crate) fn mark_managed_ws_commit_by_message(
        &mut self,
        repo: &gix::Repository,
        id: gix::ObjectId,
    ) {
        if let Ok(commit) = repo.find_commit(id)
            && let Ok(message) = commit.message_raw()
            && crate::workspace::commit::is_managed_workspace_by_message(message)
        {
            self.managed_ws_commits.insert(id);
        }
    }

    /// Where traversal/HEAD started (a checkout inside a stack), if any. The projection forces a
    /// segment boundary here — there is always a segment starting at the entrypoint.
    pub fn entrypoint(&self) -> Option<gix::ObjectId> {
        self.entrypoint
    }

    /// The ref the entrypoint was checked out as, if any — it names the entrypoint segment.
    pub fn entrypoint_ref(&self) -> Option<&gix::refs::FullName> {
        self.entrypoint_ref.as_ref()
    }

    /// Whether `id` is a GitButler-managed workspace commit (recognised by its message).
    pub(crate) fn is_managed_ws_commit(&self, id: gix::ObjectId) -> bool {
        self.managed_ws_commits.contains(&id)
    }

    /// The node at `id`, if present.
    pub fn node(&self, id: gix::ObjectId) -> Option<&CommitNode> {
        self.by_id.get(&id).map(|&idx| &self.nodes[idx])
    }

    /// Every commit id in the graph, in node order.
    pub fn commit_ids(&self) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.nodes.iter().map(|n| n.commit.id)
    }

    /// The commit's CONNECTED parent list, first-parent first — parents the traversal severed
    /// (limits, integrated stop-early, display cuts) are omitted.
    pub(crate) fn all_parent_ids(&self, id: gix::ObjectId) -> Vec<gix::ObjectId> {
        self.node(id)
            .map(|n| {
                n.commit
                    .parent_ids
                    .iter()
                    .copied()
                    .filter(|p| self.is_connected(id, *p))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// All ancestors of `tip` (inclusive), following CONNECTED parent edges — history the
    /// traversal severed is not rejoined. Bounded by the graph, which is the traversal-limited
    /// window, not the repository.
    pub fn ancestor_set(&self, tip: gix::ObjectId) -> HashSet<gix::ObjectId> {
        let mut set = HashSet::new();
        let mut queue = std::collections::VecDeque::from([tip]);
        while let Some(c) = queue.pop_front() {
            if set.insert(c) {
                queue.extend(self.all_parent_ids(c));
            }
        }
        set
    }

    /// Return `true` if any of `id`'s recorded parents is not CONNECTED in this graph — the
    /// traversal cut history here (limits, integrated stop-early), so ancestry continues
    /// beyond what the graph can see.
    pub fn has_cut_parents(&self, id: gix::ObjectId) -> bool {
        self.node(id).is_some_and(|n| {
            n.commit
                .parent_ids
                .iter()
                .any(|p| !self.is_connected(id, *p))
        })
    }

    /// The commit that `ref_name` points at, if present in the graph.
    pub fn commit_by_ref(&self, ref_name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        self.nodes
            .iter()
            .find(|n| {
                n.commit
                    .refs
                    .iter()
                    .any(|r| r.ref_name.as_ref() == ref_name)
            })
            .map(|n| n.commit.id)
    }

    /// The reference names pointing at `id`.
    pub(crate) fn refs_at(&self, id: gix::ObjectId) -> Vec<gix::refs::FullName> {
        self.node(id)
            .map(|n| n.commit.refs.iter().map(|r| r.ref_name.clone()).collect())
            .unwrap_or_default()
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
        n.commit
            .parent_ids
            .first()
            .copied()
            .filter(|p| self.by_id.contains_key(p) && self.is_connected(id, *p))
    }

    /// The children of `id` (commits that list `id` as a parent). More than one means a branch point.
    pub fn children(&self, id: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.by_id
            .get(&id)
            .into_iter()
            .flat_map(move |&idx| self.children[idx].iter().map(|&c| self.nodes[c].commit.id))
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
        let g = CommitGraph::from_commits(
            [commit(3, &[2]), commit(2, &[1]), commit(1, &[])],
            Some(id(3)),
        );
        assert_eq!(g.first_parent(id(3)), Some(id(2)));
        assert_eq!(g.first_parent(id(1)), None);
        assert_eq!(g.children(id(1)).collect::<Vec<_>>(), vec![id(2)]);
        // Generation increases with history depth.
        assert_eq!(g.node(id(1)).unwrap().generation, 0);
        assert_eq!(g.node(id(3)).unwrap().generation, 2);
    }
}
