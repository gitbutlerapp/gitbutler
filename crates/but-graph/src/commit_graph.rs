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

/// One `commit → parent` edge, HANDLE-based: parallel to a slot in the commit's `parent_ids`.
/// The raw id in `parent_ids` is payload; this is the graph structure.
#[derive(Debug, Clone, Copy)]
struct ParentSlot {
    /// The parent's node, when it is present in the graph. `None` = the raw parent id points
    /// outside the graph (partial traversal — this subgraph roots here).
    target: Option<CommitIdx>,
    /// Whether the traversal actually FOLLOWED this link. A parent can be present in the graph
    /// via another path while this specific edge was severed (limits, integrated stop-early).
    connected: bool,
}

/// A commit-first graph: an arena of commits with HANDLE-based `commit → parent` edges (one
/// [`ParentSlot`] per raw `parent_ids` entry) and the reverse (`parent → child`) adjacency derived
/// for downward walks. `ObjectId` is pure payload; `by_id` is a rebuildable lookup index.
#[derive(Debug, Clone, Default)]
pub struct CommitGraph {
    nodes: Vec<CommitNode>,
    by_id: HashMap<gix::ObjectId, CommitIdx>,
    /// Per node, one slot per `parent_ids` entry — presence and connectivity of that edge.
    parent_slots: Vec<Vec<ParentSlot>>,
    /// Nodes the EDITOR removed (see the mutation surface). A tombstoned node keeps its arena
    /// index and (stale) payload; id-based reads skip it. Walk-built graphs have none.
    tombstoned: Vec<bool>,
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

        // The handle-based edges: one slot per raw parent entry, presence from the index, every
        // edge connected until a walk restricts it. Reverse adjacency mirrors the present slots.
        let parent_slots: Vec<Vec<ParentSlot>> = nodes
            .iter()
            .map(|n| {
                n.commit
                    .parent_ids
                    .iter()
                    .map(|parent| ParentSlot {
                        target: by_id.get(parent).copied(),
                        connected: true,
                    })
                    .collect()
            })
            .collect();
        let mut children = vec![Vec::new(); nodes.len()];
        for (idx, slots) in parent_slots.iter().enumerate() {
            for slot in slots {
                if let Some(pidx) = slot.target {
                    children[pidx].push(idx);
                }
            }
        }

        let tombstoned = vec![false; nodes.len()];
        let mut graph = CommitGraph {
            nodes,
            by_id,
            parent_slots,
            children,
            tombstoned,
            entrypoint,
            entrypoint_ref: None,
            managed_ws_commits: HashSet::new(),
            hard_limit_hit: false,
            traversal_tips: Vec::new(),
            explicit_tips: false,
        };
        graph.recompute_generations();
        graph
    }

    /// Restrict connectivity to the given `(child, parent)` pairs — flag every other slot as
    /// severed — and rebuild the child adjacency from the connected, present slots.
    fn set_connected(&mut self, connected: HashSet<(gix::ObjectId, gix::ObjectId)>) {
        for children in &mut self.children {
            children.clear();
        }
        for idx in 0..self.nodes.len() {
            let id = self.nodes[idx].commit.id;
            for pos in 0..self.parent_slots[idx].len() {
                let parent = self.nodes[idx].commit.parent_ids[pos];
                let slot = &mut self.parent_slots[idx][pos];
                slot.connected = connected.contains(&(id, parent));
                if slot.connected
                    && let Some(pidx) = slot.target
                {
                    self.children[pidx].push(idx);
                }
            }
        }
        self.recompute_generations();
    }

    /// The slots of `id`'s node, when present.
    fn slots_of(&self, id: gix::ObjectId) -> Option<&[ParentSlot]> {
        self.by_id
            .get(&id)
            .map(|&idx| self.parent_slots[idx].as_slice())
    }

    /// Assemble from the NATIVE traversal outcome (see `init::native_walk`).
    pub(crate) fn from_native_outcome(o: crate::init::native_walk::NativeOutcome) -> Self {
        let mut cg = CommitGraph::from_commits(o.commits, o.entrypoint);
        cg.entrypoint_ref = o.entrypoint_ref;
        cg.set_connected(o.connected);
        cg.hard_limit_hit = o.hard_limit_hit;
        cg.traversal_tips = o.tips;
        // Goal bits stop mattering the moment the traversal ends — and their numbering depends
        // on tip processing order, so graphs from different builds could never compare equal
        // while carrying them.
        cg.strip_goal_flags();
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

    /// Replace every node's attached refs with the CURRENT `refs_by_id` state — the
    /// write-through seam's enrichment refresh: an editor-mutated graph still carries
    /// walk-time refs, but materialization has moved them. Matched entries are consumed.
    pub(crate) fn refresh_refs(
        &mut self,
        refs_by_id: &mut crate::init::walk::RefsById,
        worktree_by_branch: &crate::init::walk::WorktreeByBranch,
    ) {
        for (idx, node) in self.nodes.iter_mut().enumerate() {
            if self.tombstoned[idx] {
                node.commit.refs.clear();
                continue;
            }
            let id = node.commit.id;
            let mut refs: Vec<crate::RefInfo> = refs_by_id
                .remove(&id)
                .unwrap_or_default()
                .into_iter()
                .map(|rn| crate::RefInfo::from_ref(rn, id, worktree_by_branch))
                .collect();
            refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
            node.commit.refs = refs;
        }
    }

    /// Overwrite the node's flags — the write-through seam flags re-added anchor regions
    /// Integrated, the walk's convention for target-seeded tips.
    pub(crate) fn set_flags(&mut self, idx: CommitIdx, flags: crate::CommitFlags) {
        self.nodes[idx].commit.flags = flags;
    }

    /// Recompute the `Integrated` flag on every live node as target-reachability from `tips` —
    /// the write-through seam's flag refresh: an editor-mutated graph carries walk-time flags
    /// (empty on editor-added nodes), while the rewalk derives integration fresh.
    pub(crate) fn recompute_integrated(&mut self, tips: impl IntoIterator<Item = gix::ObjectId>) {
        let mut integrated: HashSet<gix::ObjectId> = HashSet::new();
        for tip in tips {
            if self.by_id.contains_key(&tip) && !integrated.contains(&tip) {
                integrated.extend(self.ancestor_set(tip));
            }
        }
        for (idx, node) in self.nodes.iter_mut().enumerate() {
            if self.tombstoned[idx] {
                continue;
            }
            node.commit.flags.set(
                crate::CommitFlags::Integrated,
                integrated.contains(&node.commit.id),
            );
        }
    }

    /// Set [`CommitFlags::InWorkspace`](crate::CommitFlags::InWorkspace) on exactly the
    /// ancestors of `ws_commit` (`None` clears the flag everywhere) — the walk's rule, where
    /// only the workspace tip seeds the flag and it propagates to everything reachable.
    pub(crate) fn recompute_in_workspace(&mut self, ws_commit: Option<gix::ObjectId>) {
        let in_workspace = ws_commit
            .filter(|tip| self.by_id.contains_key(tip))
            .map(|tip| self.ancestor_set(tip))
            .unwrap_or_default();
        for (idx, node) in self.nodes.iter_mut().enumerate() {
            if self.tombstoned[idx] {
                continue;
            }
            node.commit.flags.set(
                crate::CommitFlags::InWorkspace,
                in_workspace.contains(&node.commit.id),
            );
        }
    }

    /// Set [`CommitFlags::NotInRemote`](crate::CommitFlags::NotInRemote) on exactly the
    /// ancestors of `local_tips` (the walk's rule: every LOCAL branch tip seeds the flag,
    /// remote tips don't) — clearing it elsewhere, e.g. on a commit the editor dropped from
    /// local history that a remote ref still holds.
    pub(crate) fn recompute_not_in_remote(
        &mut self,
        local_tips: impl IntoIterator<Item = gix::ObjectId>,
    ) {
        let mut not_in_remote: HashSet<gix::ObjectId> = HashSet::new();
        for tip in local_tips {
            if self.by_id.contains_key(&tip) && !not_in_remote.contains(&tip) {
                not_in_remote.extend(self.ancestor_set(tip));
            }
        }
        for (idx, node) in self.nodes.iter_mut().enumerate() {
            if self.tombstoned[idx] {
                continue;
            }
            node.commit.flags.set(
                crate::CommitFlags::NotInRemote,
                not_in_remote.contains(&node.commit.id),
            );
        }
    }

    /// Bring a TOMBSTONED node holding `id` back to life — the write-through seam's anchor
    /// revival: a stored/extra target the editor dropped from workspace history is still
    /// external context on disk, and the walk always seeds it as an integrated tip.
    ///
    /// Returns `true` if a tombstone actually came back to life — a revival flips the
    /// effective parents of every child that was tombstone-substituting through this node,
    /// so callers may need to re-validate them.
    pub(crate) fn revive(&mut self, id: gix::ObjectId) -> bool {
        if self.by_id.contains_key(&id) {
            return false;
        }
        let Some(idx) =
            (0..self.nodes.len()).find(|&i| self.tombstoned[i] && self.nodes[i].commit.id == id)
        else {
            return false;
        };
        self.tombstoned[idx] = false;
        self.by_id.insert(id, idx);
        true
    }

    /// The node at `id`, if present.
    pub fn node(&self, id: gix::ObjectId) -> Option<&CommitNode> {
        self.by_id.get(&id).map(|&idx| &self.nodes[idx])
    }

    /// The arena index of `id`, if present.
    pub fn index_of(&self, id: gix::ObjectId) -> Option<CommitIdx> {
        self.by_id.get(&id).copied()
    }

    /// Every commit id in the graph, in node order.
    pub fn commit_ids(&self) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.nodes.iter().map(|n| n.commit.id)
    }

    /// The commit's CONNECTED parent list, first-parent first — parents the traversal severed
    /// (limits, integrated stop-early, display cuts) are omitted.
    ///
    /// An editor-dropped (TOMBSTONED) parent is substituted in place by its own parents,
    /// recursively — the same descent the editor's parent collection performs when it turns
    /// the structure into real commits — deduplicated along the descent, while plain
    /// duplicate slots are all kept (dup-parent workspace commits).
    pub(crate) fn all_parent_ids(&self, id: gix::ObjectId) -> Vec<gix::ObjectId> {
        let Some(&idx) = self.by_id.get(&id) else {
            return Vec::new();
        };
        // The CONNECTED `(raw parent id, slot target)` pairs of `idx`, in slot order.
        let connected = |idx: CommitIdx| {
            self.nodes[idx]
                .commit
                .parent_ids
                .iter()
                .copied()
                .zip(&self.parent_slots[idx])
                .filter_map(|(p, slot)| slot.connected.then_some((p, slot.target)))
                .collect::<Vec<_>>()
        };
        let mut potential: Vec<(gix::ObjectId, Option<CommitIdx>)> =
            connected(idx).into_iter().rev().collect();
        let mut seen_idx: HashSet<CommitIdx> = potential.iter().filter_map(|(_, t)| *t).collect();
        let mut seen_raw: HashSet<gix::ObjectId> = potential
            .iter()
            .filter_map(|(p, t)| t.is_none().then_some(*p))
            .collect();
        let mut parents = Vec::new();
        while let Some((raw, target)) = potential.pop() {
            match target {
                Some(t) if self.tombstoned[t] => {
                    for (p_raw, p_target) in connected(t).into_iter().rev() {
                        let unseen = match p_target {
                            Some(pt) => seen_idx.insert(pt),
                            None => seen_raw.insert(p_raw),
                        };
                        if unseen {
                            potential.push((p_raw, p_target));
                        }
                    }
                }
                // A live target's CURRENT payload id is authoritative (raw agrees via
                // set_commit_id's child patching; this needs no such guarantee).
                Some(t) => parents.push(self.nodes[t].commit.id),
                None => parents.push(raw),
            }
        }
        parents
    }

    /// The RAW recorded parent ids of `idx` — the payload array, cut slots included. Unlike
    /// [`Self::all_parent_ids`] this does NOT substitute through tombstones, so a slot whose
    /// target was editor-dropped still shows the dropped commit's id.
    pub(crate) fn raw_parent_ids(&self, idx: CommitIdx) -> &[gix::ObjectId] {
        &self.nodes[idx].commit.parent_ids
    }

    /// `true` if any CONNECTED parent slot of `idx` targets a tombstone — traversal would
    /// substitute through it, so the raw recorded parents disagree with what a walk sees.
    pub(crate) fn has_tombstoned_parent(&self, idx: CommitIdx) -> bool {
        self.parent_slots[idx]
            .iter()
            .any(|slot| slot.connected && slot.target.is_some_and(|t| self.tombstoned[t]))
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
        self.slots_of(id)
            .is_some_and(|slots| slots.iter().any(|slot| !slot.connected))
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
        self.slots_of(id)
            .unwrap_or_default()
            .iter()
            .filter_map(|slot| slot.target.map(|idx| self.nodes[idx].commit.id))
    }

    /// The first parent of `id` (the next commit walking down first-parent), if present.
    pub fn first_parent(&self, id: gix::ObjectId) -> Option<gix::ObjectId> {
        let slot = self.slots_of(id)?.first()?;
        let target = slot.target.filter(|_| slot.connected)?;
        Some(self.nodes[target].commit.id)
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
    pub(crate) fn recompute_generations(&mut self) {
        // Process in topological order (parents before children) so a child's generation is the max
        // over its present parents + 1.
        let order = self.toposort_parents_first();
        for idx in order {
            let generation = self.parent_slots[idx]
                .iter()
                .filter_map(|slot| slot.target)
                .map(|pidx| self.nodes[pidx].generation + 1)
                .max()
                .unwrap_or(0);
            self.nodes[idx].generation = generation;
        }
    }

    /// Topological order with parents before children (history order).
    fn toposort_parents_first(&self) -> Vec<CommitIdx> {
        let mut indegree = vec![0usize; self.nodes.len()];
        for (idx, slots) in self.parent_slots.iter().enumerate() {
            indegree[idx] = slots.iter().filter(|slot| slot.target.is_some()).count();
        }
        let mut queue: std::collections::VecDeque<CommitIdx> = (0..self.nodes.len())
            .filter(|&i| indegree[i] == 0)
            .collect();
        let mut out = Vec::with_capacity(self.nodes.len());
        while let Some(idx) = queue.pop_front() {
            out.push(idx);
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

    // --- The EDITOR MUTATION SURFACE ---
    //
    // but-rebase's editor mutates a CommitGraph in place: arena indices are the stable node
    // ids, `ObjectId` is payload. These writes maintain `by_id`, the children adjacency, and
    // the raw `parent_ids` payload. `generation` is creation-time data and NOT maintained.

    /// Append a fresh node with `id` as payload (`None` = born tombstoned) and no parents.
    pub fn add_node(&mut self, id: Option<gix::ObjectId>) -> CommitIdx {
        let idx = self.nodes.len();
        self.nodes.push(CommitNode {
            commit: Commit {
                id: id.unwrap_or_else(|| gix::ObjectId::null(gix::hash::Kind::Sha1)),
                parent_ids: Vec::new(),
                flags: CommitFlags::empty(),
                refs: Vec::new(),
            },
            generation: 0,
        });
        self.parent_slots.push(Vec::new());
        self.children.push(Vec::new());
        self.tombstoned.push(id.is_none());
        if let Some(id) = id {
            self.by_id.insert(id, idx);
        }
        idx
    }

    /// Overwrite the node's payload id — `None` tombstones it in place (the stale payload is
    /// retained, id-based lookups stop finding it), `Some` (re)vitalizes it.
    pub fn set_node_id(&mut self, idx: CommitIdx, id: Option<gix::ObjectId>) {
        match id {
            Some(id) => {
                self.tombstoned[idx] = false;
                self.set_commit_id(idx, id);
            }
            None => {
                let old = self.nodes[idx].commit.id;
                if self.by_id.get(&old) == Some(&idx) {
                    self.by_id.remove(&old);
                }
                self.tombstoned[idx] = true;
            }
        }
    }

    /// Rewrite the commit id at `idx` IN PLACE — THE rebase write. The node index, its slots,
    /// and its children survive; `by_id`, the children's raw `parent_ids` entries, and the
    /// id-addressed markers (entrypoint, managed-ws) follow the payload.
    pub fn set_commit_id(&mut self, idx: CommitIdx, id: gix::ObjectId) {
        let old = self.nodes[idx].commit.id;
        self.nodes[idx].commit.id = id;
        if self.by_id.get(&old) == Some(&idx) {
            self.by_id.remove(&old);
        }
        self.by_id.insert(id, idx);
        for child in self.children[idx].clone() {
            for (slot_pos, slot) in self.parent_slots[child].iter().enumerate() {
                if slot.target == Some(idx) {
                    self.nodes[child].commit.parent_ids[slot_pos] = id;
                }
            }
        }
        if self.entrypoint == Some(old) {
            self.entrypoint = Some(id);
        }
        if self.managed_ws_commits.remove(&old) {
            self.managed_ws_commits.insert(id);
        }
    }

    /// Overwrite `idx`'s whole parent array — the editor's ordered-slot write. Every new slot
    /// is PRESENT and CONNECTED; the raw `parent_ids` payload derives from the targets and the
    /// children adjacency follows.
    pub fn set_parents(&mut self, idx: CommitIdx, parents: Vec<CommitIdx>) {
        for slot in std::mem::take(&mut self.parent_slots[idx]) {
            if let Some(t) = slot.target
                && let Some(pos) = self.children[t].iter().position(|&c| c == idx)
            {
                self.children[t].remove(pos);
            }
        }
        self.nodes[idx].commit.parent_ids =
            parents.iter().map(|&p| self.nodes[p].commit.id).collect();
        self.parent_slots[idx] = parents
            .iter()
            .map(|&p| ParentSlot {
                target: Some(p),
                connected: true,
            })
            .collect();
        for &p in &parents {
            self.children[p].push(idx);
        }
    }

    /// Clear the walk's GOAL bits (the bits beyond [`CommitFlags::all()`](crate::CommitFlags))
    /// from every node. Goal numbering is traversal-order ephemera — a node that survived a
    /// rewrite carries whatever bits the ORIGINAL walk assigned, which a fresh walk would
    /// number differently — and nothing after the walk reads goals.
    pub(crate) fn strip_goal_flags(&mut self) {
        for node in &mut self.nodes {
            node.commit.flags &= crate::CommitFlags::all();
        }
    }

    /// Drop tombstoned nodes and reindex, leaving a graph indistinguishable from one built
    /// without them — what the write-through seam hands to projection, so a carried graph
    /// never leaks editor tombstones to the next consumer. The caller must first ensure no
    /// live slot targets a tombstone (the seam's odb reconciliation guarantees it); such a
    /// slot would degrade to "parent outside the graph" here.
    pub fn compact(&mut self) {
        if !self.tombstoned.iter().any(|&t| t) {
            return;
        }
        let mut remap: Vec<Option<CommitIdx>> = vec![None; self.nodes.len()];
        let mut next = 0;
        for (idx, remapped) in remap.iter_mut().enumerate() {
            if !self.tombstoned[idx] {
                *remapped = Some(next);
                next += 1;
            }
        }
        let mut nodes = Vec::with_capacity(next);
        let mut parent_slots = Vec::with_capacity(next);
        for idx in 0..self.nodes.len() {
            if remap[idx].is_none() {
                continue;
            }
            nodes.push(self.nodes[idx].clone());
            parent_slots.push(
                self.parent_slots[idx]
                    .iter()
                    .map(|slot| ParentSlot {
                        target: slot.target.and_then(|t| remap[t]),
                        connected: slot.connected,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let by_id: HashMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(idx, n): (_, &CommitNode)| (n.commit.id, idx))
            .collect();
        let mut children = vec![Vec::new(); nodes.len()];
        for (idx, slots) in parent_slots.iter().enumerate() {
            for slot in slots {
                if let Some(pidx) = slot.target {
                    children[pidx].push(idx);
                }
            }
        }
        self.managed_ws_commits.retain(|id| by_id.contains_key(id));
        self.nodes = nodes;
        self.by_id = by_id;
        self.parent_slots = parent_slots;
        self.children = children;
        self.tombstoned = vec![false; next];
    }

    /// Arena length, tombstones included.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The payload id at `idx` — `None` for tombstones.
    pub fn node_payload(&self, idx: CommitIdx) -> Option<gix::ObjectId> {
        (!self.tombstoned[idx]).then(|| self.nodes[idx].commit.id)
    }

    /// The parent TARGETS of `idx` in slot order. Only meaningful once every slot is
    /// editor-authored (present) — walk-built graphs can have absent slots.
    pub fn parent_indices(&self, idx: CommitIdx) -> Vec<CommitIdx> {
        self.parent_slots[idx]
            .iter()
            .map(|slot| slot.target.expect("editor-authored slots are present"))
            .collect()
    }

    /// The PRESENT parent targets of `idx` in slot order — absent (walk-cut) slots are
    /// skipped, unlike [`Self::parent_indices`] which requires every slot to be present.
    pub fn present_parent_indices(&self, idx: CommitIdx) -> Vec<CommitIdx> {
        self.parent_slots[idx]
            .iter()
            .filter_map(|slot| slot.target)
            .collect()
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
