//! The commit graph: the raw traversal flattened into nodes — the single arena the
//! build authors its ref layout onto and every consumer reads (see `build`).
//!
//! Pipeline: `gix traversal → CommitGraph (+ ref layout) → projection`. The traversal
//! accumulates the arena directly ([`CommitGraph::from_walk`]), the build stores the ref
//! layout on it, the projection derives the [`Workspace`](crate::Workspace) from it, and
//! but-rebase's editor adopts the carried copy as its mutable arena. Both consumers are
//! commit/ref-granular: the build's segments never leave the builder.
//!
//! A node IS a commit ([`crate::Commit`]); every per-node attribute — parent edges, children,
//! tombstones, the topological `generation` — rides a parallel array. An edge is
//! `commit → parent` from `parent_ids` (first-parent at index 0). A nice consequence: the
//! workspace commit's `parent_ids` array IS the stack order, so no order-stacks post-pass.

use gix::hashtable::{HashMap, HashSet};

use crate::{Commit, CommitFlags};

mod merge_base;

/// One `commit → parent` edge, HANDLE-based: parallel to one entry of the commit's `parent_ids`.
/// The raw id in `parent_ids` is payload; this is the graph structure.
#[derive(Debug, Clone, Copy)]
struct ParentEdge {
    /// The parent's node, when it is present in the arena. `None` = the raw parent id points
    /// outside the arena (partial traversal — this subgraph roots here).
    target_node_idx: Option<usize>,
    /// Whether the traversal actually FOLLOWED this link. A parent can be present in the arena
    /// via another path while this specific edge was severed (limits, integrated stop-early).
    connected: bool,
}

/// The commit graph: an arena of commit nodes with HANDLE-based `commit → parent` edges (one
/// `ParentEdge` per raw `parent_ids` entry) and the reverse (`parent → child`) adjacency derived
/// for downward walks. `ObjectId` is pure payload; `by_id` is a rebuildable lookup index.
#[derive(Debug, Clone, Default)]
pub struct CommitGraph {
    nodes: Vec<Commit>,
    /// Per node: distance from a root; higher means deeper in history. Used where the
    /// projection picks "the lowest of several tips". Creation-time data
    /// (see `compute_generations`), not maintained by the editor mutation surface.
    generations: Vec<u32>,
    by_id: HashMap<gix::ObjectId, usize>,
    /// Ref name → node holding it. Like `by_id` a rebuildable lookup: rebuilt whenever refs
    /// are written (construction, [`Self::refresh_refs`]) or indices move ([`Self::compact`]).
    by_ref: std::collections::HashMap<gix::refs::FullName, usize>,
    /// Per node, one edge per `parent_ids` entry — presence and connectivity, in parent-number order.
    parent_edges: Vec<Vec<ParentEdge>>,
    /// Rows the EDITOR removed (see the mutation surface). A tombstoned node keeps its arena
    /// index and (stale) payload; id-based reads skip it. Walk-built graphs have none.
    tombstoned: Vec<bool>,
    /// `parent → children` adjacency, derived at build time for branch-point detection and
    /// downward walks (the projection walks tip toward base).
    children: Vec<Vec<usize>>,
    /// Where traversal/HEAD started; the projection uses it as a focus boundary.
    entrypoint: Option<gix::ObjectId>,
    /// The ref the entrypoint was checked out as, if any. When set, it names the entrypoint segment
    /// (overriding disambiguation), mirroring `from_tip(id, Some(ref))`.
    entrypoint_ref: Option<gix::refs::FullName>,
    /// Commits whose message marks them as a GitButler-managed workspace commit. Kept out of
    /// [`CommitFlags`](crate::CommitFlags) so it neither perturbs the walk's goal bits nor the
    /// segment fingerprint; used to tell a real managed merge from a ws ref advanced past it.
    managed_ws_commits: HashSet<gix::ObjectId>,
    /// When built [from the walk](Self::from_walk): whether the traversal stopped queueing after
    /// hitting the hard limit. Derived graphs must carry it forward.
    pub(crate) hard_limit_hit: bool,
    /// When built [from the walk](Self::from_walk): the traversal's normalized seed tips. Graphs
    /// built from EXPLICIT tips must carry them forward — the projection reads tip roles
    /// (e.g. integrated tips) for such graphs.
    pub(crate) seeds: Vec<crate::walk::Seed>,
    /// Built from EXPLICIT tips ([`Self::from_walk_seeds`]): every tip must start (or get) its own
    /// segment. Workspace-discovered builds must NOT carve boundaries at their normalized tips —
    /// there they are ordinary interior commits unless the plan makes them boundaries.
    pub(crate) explicit_seeds: bool,
    /// When built [from the walk](Self::from_walk): the extra target commit the traversal was
    /// seeded with (`Options::extra_target_commit_id`). Carried like [`Self::hard_limit_hit`] so
    /// the projection can surface it as a seed without consulting the walk options.
    pub(crate) extra_target: Option<gix::ObjectId>,
    /// The ref placement table authored at build time (see [`ref_layout`](crate::ref_layout));
    /// `None` until the commit graph goes through the builder. Mirrors build-time refs — like
    /// [`Commit::refs`](crate::Commit) it goes stale under editor mutation and is re-authored
    /// by the write-through projection.
    pub(crate) layout: Option<crate::ref_layout::RefLayout>,
}

impl CommitGraph {
    fn parent_edges_of(&self, id: gix::ObjectId) -> Option<&[ParentEdge]> {
        self.by_id
            .get(&id)
            .map(|&idx| self.parent_edges[idx].as_slice())
    }

    /// The id → node lookup for `nodes` (payloads are unique).
    fn index_by_id(nodes: &[Commit]) -> HashMap<gix::ObjectId, usize> {
        nodes
            .iter()
            .enumerate()
            .map(|(idx, n)| (n.id, idx))
            .collect()
    }

    /// The parent → children adjacency derived from `parent_edges`. `connected_only`
    /// mirrors the walk (a severed edge yields no child); [`Self::compact`] keeps every
    /// PRESENT target — see the note there.
    fn derive_children(parent_edges: &[Vec<ParentEdge>], connected_only: bool) -> Vec<Vec<usize>> {
        let mut children = vec![Vec::new(); parent_edges.len()];
        for (idx, edges) in parent_edges.iter().enumerate() {
            for edge in edges {
                if let Some(pidx) = edge.target_node_idx
                    && (edge.connected || !connected_only)
                {
                    children[pidx].push(idx);
                }
            }
        }
        children
    }

    /// Assemble an arena from its authoritative parts — children, generations, and the ref
    /// lookup are derived; the walk-carried fields stay at their defaults for the caller.
    fn from_parts(
        nodes: Vec<Commit>,
        by_id: HashMap<gix::ObjectId, usize>,
        parent_edges: Vec<Vec<ParentEdge>>,
    ) -> Self {
        let mut table = CommitGraph {
            generations: vec![0; nodes.len()],
            children: Self::derive_children(&parent_edges, true),
            tombstoned: vec![false; nodes.len()],
            nodes,
            by_id,
            parent_edges,
            ..Default::default()
        };
        table.recompute_generations();
        table.rebuild_by_ref();
        table
    }

    /// Assemble from the walk's outcome (see `walk::walker`): the walk's `by_id` is adopted
    /// as-is (collection order IS node order), and parent-edge connectivity comes straight from the
    /// followed edges rather than being re-derived.
    #[tracing::instrument(name = "CommitGraph::from_walk_outcome", level = "trace", skip_all)]
    pub(crate) fn from_walk_outcome(o: crate::walk::walker::WalkOutcome) -> Self {
        let nodes: Vec<Commit> = o.commits;
        let by_id = o.by_id;
        let parent_edges: Vec<Vec<ParentEdge>> = nodes
            .iter()
            .map(|n| {
                let followed = o.parents_followed.get(&n.id);
                n.parent_ids
                    .iter()
                    .map(|parent| ParentEdge {
                        target_node_idx: by_id.get(parent).copied(),
                        connected: followed.is_some_and(|ps| ps.contains(parent)),
                    })
                    .collect()
            })
            .collect();
        let mut table = Self::from_parts(nodes, by_id, parent_edges);
        table.entrypoint = o.entrypoint;
        table.entrypoint_ref = o.entrypoint_ref;
        table.hard_limit_hit = o.hard_limit_hit;
        table.seeds = o.seeds;
        // Goal bits stop mattering once the traversal ends, and their numbering depends on tip
        // processing order — carrying them would break cross-build comparison.
        table.strip_goal_flags();
        table
    }

    /// Build by running the real traversal, accumulating commits directly — extents (limit cuts,
    /// integrated stop-early) and flags keep the walk's battle-tested semantics.
    pub(crate) fn from_walk<T: but_core::RefMetadata>(
        overlay_repo: &crate::walk::overlay::OverlayRepo<'_>,
        overlay_meta: &crate::walk::overlay::OverlayMetadata<'_, T>,
        tip: gix::ObjectId,
        ref_name: Option<gix::refs::FullName>,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::walk::Options,
    ) -> anyhow::Result<Self> {
        let extra_target = options.extra_target_commit_id;
        let seeds = crate::walk::initial_seeds_from_workspace_metadata(
            overlay_repo,
            overlay_meta,
            tip,
            ref_name.as_ref(),
            &project_meta,
            extra_target,
        )?;
        let walked = crate::walk::walker::traverse(
            overlay_repo,
            seeds,
            overlay_meta,
            project_meta,
            options,
            ref_name,
        )?;
        let mut outcome = Self::from_walk_outcome(walked);
        outcome.extra_target = extra_target;
        outcome.apply_posthoc_flags();
        Ok(outcome)
    }

    /// Like [`Self::from_walk`], but seeded from explicit `tips`.
    pub(crate) fn from_walk_seeds<T: but_core::RefMetadata>(
        overlay_repo: &crate::walk::overlay::OverlayRepo<'_>,
        overlay_meta: &crate::walk::overlay::OverlayMetadata<'_, T>,
        tips: Vec<crate::walk::Seed>,
        project_meta: but_core::ref_metadata::ProjectMeta,
        options: crate::walk::Options,
    ) -> anyhow::Result<Self> {
        let extra_target = options.extra_target_commit_id;
        let walked = crate::walk::walker::traverse(
            overlay_repo,
            tips,
            overlay_meta,
            project_meta,
            options,
            None,
        )?;
        let mut outcome = Self::from_walk_outcome(walked);
        outcome.explicit_seeds = true;
        outcome.extra_target = extra_target;
        outcome.apply_posthoc_flags();
        Ok(outcome)
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

    /// Recompute the three workspace flags from the carried seeds: mark everything reachable
    /// over the connected arena (the same sweeps the write-through seam runs). The flags the
    /// walk set while traversing only steered the walk — these sweeps produce the
    /// authoritative values.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn apply_posthoc_flags(&mut self) {
        use crate::CommitFlags;
        let mut integrated = Vec::new();
        let mut ws = Vec::new();
        let mut not_in_remote = Vec::new();
        for s in &self.seeds {
            if s.role.is_integrated() {
                integrated.push(s.id);
            } else {
                not_in_remote.push(s.id);
            }
            if matches!(s.role, crate::walk::SeedRole::Workspace) {
                ws.push(s.id);
            }
        }
        integrated.extend(self.extra_target);
        // A fresh walk's cut edges stay cut: the sweep follows only the parents the
        // traversal connected (the seam reconciles all edges before its sweeps, so
        // both views coincide there).
        self.set_flag_on_ancestors(CommitFlags::Integrated, integrated);
        self.set_flag_on_ancestors(CommitFlags::InWorkspace, ws);
        self.set_flag_on_ancestors(CommitFlags::NotInRemote, not_in_remote);
    }

    /// The worktrees referenced by any commit's refs.
    pub(crate) fn ref_worktrees(&self) -> impl Iterator<Item = &crate::Worktree> {
        self.nodes
            .iter()
            .flat_map(|n| n.refs.iter())
            .filter_map(|ri| ri.worktree.as_ref())
    }

    /// The ref placement table authored at build time, when this graph went through the builder.
    pub fn layout(&self) -> Option<&crate::ref_layout::RefLayout> {
        self.layout.as_ref()
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
        refs_by_id: &mut crate::walk::utils::RefsById,
        worktree_by_branch: &crate::walk::utils::WorktreeByBranch,
    ) {
        for (idx, node) in self.nodes.iter_mut().enumerate() {
            if self.tombstoned[idx] {
                node.refs.clear();
                continue;
            }
            let id = node.id;
            let mut refs: Vec<crate::RefInfo> = refs_by_id
                .remove(&id)
                .unwrap_or_default()
                .into_iter()
                .map(|rn| crate::RefInfo::from_ref(rn, id, worktree_by_branch))
                .collect();
            refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
            node.refs = refs;
        }
        self.rebuild_by_ref();
    }

    /// Mutable access to the refs attached to `id`, for anonymization. Callers must
    /// rebuild the ref lookup afterwards.
    pub(crate) fn commit_refs_mut(
        &mut self,
        id: gix::ObjectId,
    ) -> Option<&mut Vec<crate::RefInfo>> {
        let idx = self.index_of(id)?;
        Some(&mut self.nodes[idx].refs)
    }

    /// Mutable access to the entrypoint ref, for anonymization.
    pub(crate) fn entrypoint_ref_mut(&mut self) -> Option<&mut gix::refs::FullName> {
        self.entrypoint_ref.as_mut()
    }

    /// Set the ref the entrypoint was checked out as — for graphs assembled
    /// without a walk (unborn refs).
    pub(crate) fn set_entrypoint_ref(&mut self, name: gix::refs::FullName) {
        self.entrypoint_ref = Some(name);
    }

    /// Rebuild the ref-name lookup from the (live) nodes' attached refs.
    pub(crate) fn rebuild_by_ref(&mut self) {
        self.by_ref.clear();
        for (idx, node) in self.nodes.iter().enumerate() {
            if self.tombstoned[idx] {
                continue;
            }
            for r in &node.refs {
                self.by_ref.insert(r.ref_name.clone(), idx);
            }
        }
    }

    /// Overwrite the node's flags — the write-through seam flags re-added tip regions
    /// Integrated, the walk's convention for target-seeded tips.
    pub(crate) fn set_flags(&mut self, idx: usize, flags: crate::CommitFlags) {
        self.nodes[idx].flags = flags;
    }

    /// Set `flag` on exactly the (live) ancestors of `tips`, clearing it elsewhere — the
    /// write-through seam's flag refresh: an editor-mutated graph carries walk-time flags
    /// (empty on editor-added nodes), while the rewalk derives each flag fresh. Which tips
    /// seed which flag is the walk's rule, spelled at the call sites.
    pub(crate) fn set_flag_on_ancestors(
        &mut self,
        flag: crate::CommitFlags,
        tips: impl IntoIterator<Item = gix::ObjectId>,
    ) {
        let mut marked: HashSet<gix::ObjectId> = HashSet::default();
        let mut queue: Vec<gix::ObjectId> = tips
            .into_iter()
            .filter(|tip| self.by_id.contains_key(tip))
            .collect();
        while let Some(id) = queue.pop() {
            if marked.insert(id) {
                queue.extend(self.all_parent_ids(id));
            }
        }
        for (idx, node) in self.nodes.iter_mut().enumerate() {
            if self.tombstoned[idx] {
                continue;
            }
            node.flags.set(flag, marked.contains(&node.id));
        }
    }

    /// Bring a TOMBSTONED node holding `id` back to life — a target the editor dropped is still
    /// external context on disk, and the walk always seeds it. Returns `true` on actual revival:
    /// that flips the effective parents of children substituting through this node, so callers
    /// may need to re-validate them.
    pub(crate) fn revive(&mut self, id: gix::ObjectId) -> bool {
        if self.by_id.contains_key(&id) {
            return false;
        }
        let Some(idx) =
            (0..self.nodes.len()).find(|&i| self.tombstoned[i] && self.nodes[i].id == id)
        else {
            return false;
        };
        self.tombstoned[idx] = false;
        self.by_id.insert(id, idx);
        true
    }

    /// The commit holding `id`, if present.
    pub fn node(&self, id: gix::ObjectId) -> Option<&Commit> {
        self.by_id.get(&id).map(|&idx| &self.nodes[idx])
    }

    /// The topological generation of the commit with `id`, if present — distance from a
    /// root, higher is deeper in history. Creation-time data: the editor's mutation
    /// surface does not maintain it.
    pub fn generation_of(&self, id: gix::ObjectId) -> Option<u32> {
        self.by_id.get(&id).map(|&idx| self.generations[idx])
    }

    /// The topological generation of the node at `idx`.
    pub(crate) fn generation_at(&self, idx: usize) -> u32 {
        self.generations[idx]
    }

    /// The commit holding `name` — the ref's target in the arena.
    pub(crate) fn ref_target(&self, name: &gix::refs::FullName) -> Option<gix::ObjectId> {
        self.by_ref.get(name).map(|&idx| self.nodes[idx].id)
    }

    /// The node index of `id`, if present.
    pub fn index_of(&self, id: gix::ObjectId) -> Option<usize> {
        self.by_id.get(&id).copied()
    }

    /// Every commit id in the arena, in node order.
    pub fn commit_ids(&self) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.nodes.iter().map(|n| n.id)
    }

    /// The commit's CONNECTED parents, first-parent first — parents the traversal severed
    /// (limits, integrated stop-early, display cuts) are omitted. A TOMBSTONED parent is
    /// substituted by its own parents, recursively (the editor's descent), deduplicated along
    /// the way; plain duplicate parents are all kept (dup-parent workspace commits).
    pub(crate) fn all_parent_ids(&self, id: gix::ObjectId) -> Vec<gix::ObjectId> {
        let Some(&idx) = self.by_id.get(&id) else {
            return Vec::new();
        };
        // Fast path: no tombstoned parent to substitute through (always true for walk-built
        // graphs), so the connected edges map straight to parent ids.
        if !self.has_tombstoned_parent(idx) {
            return self.nodes[idx]
                .parent_ids
                .iter()
                .copied()
                .zip(&self.parent_edges[idx])
                .filter(|(_, edge)| edge.connected)
                .map(|(p, edge)| match edge.target_node_idx {
                    Some(t) => self.nodes[t].id,
                    None => p,
                })
                .collect();
        }
        // The CONNECTED `(raw parent id, edge target)` pairs of `idx`, in parent-number order.
        let connected = |idx: usize| {
            self.nodes[idx]
                .parent_ids
                .iter()
                .copied()
                .zip(&self.parent_edges[idx])
                .filter_map(|(p, edge)| edge.connected.then_some((p, edge.target_node_idx)))
                .collect::<Vec<_>>()
        };
        let mut potential: Vec<(gix::ObjectId, Option<usize>)> =
            connected(idx).into_iter().rev().collect();
        let mut seen_idx: std::collections::HashSet<usize> =
            potential.iter().filter_map(|(_, t)| *t).collect();
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
                Some(t) => parents.push(self.nodes[t].id),
                None => parents.push(raw),
            }
        }
        parents
    }

    /// The RAW recorded parent ids of `idx` — the payload array, cut edges included, no
    /// tombstone substitution (unlike [`Self::all_parent_ids`]).
    pub(crate) fn raw_parent_ids(&self, idx: usize) -> &[gix::ObjectId] {
        &self.nodes[idx].parent_ids
    }

    /// `true` if any CONNECTED parent edge of `idx` targets a tombstone — traversal would
    /// substitute through it, so the raw recorded parents disagree with what a walk sees.
    pub(crate) fn has_tombstoned_parent(&self, idx: usize) -> bool {
        self.parent_edges[idx]
            .iter()
            .any(|edge| edge.connected && edge.target_node_idx.is_some_and(|t| self.tombstoned[t]))
    }

    /// All ancestors of `tip` (inclusive), following CONNECTED parent edges — history the
    /// traversal severed is not rejoined. Bounded by the arena, which is the traversal-limited
    /// window, not the repository.
    pub fn ancestor_set(&self, tip: gix::ObjectId) -> HashSet<gix::ObjectId> {
        let mut set = HashSet::default();
        let mut queue = std::collections::VecDeque::from([tip]);
        while let Some(c) = queue.pop_front() {
            if set.insert(c) {
                queue.extend(self.all_parent_ids(c));
            }
        }
        set
    }

    /// The first commit along `tip`'s first-parent spine for which `is_in_set` holds — where an
    /// outside line (a remote, the target, an outside checkout, a branch that advanced past the
    /// workspace) rejoins a marked region of the graph.
    pub(crate) fn first_on_spine(
        &self,
        tip: gix::ObjectId,
        is_in_set: impl Fn(usize) -> bool,
    ) -> Option<gix::ObjectId> {
        let mut cursor = self.index_of(tip);
        while let Some(c) = cursor {
            if is_in_set(c) {
                return Some(self.id_at(c));
            }
            cursor = self.first_parent_at(c);
        }
        None
    }

    /// [`Self::ancestor_set`] in handle space: `marks[idx]` for every reachable LIVE node. The
    /// walk passes THROUGH tombstones without marking them, matching the id-space substitution.
    /// Query membership via [`Self::index_of`].
    pub fn ancestor_marks(&self, tip: gix::ObjectId) -> Vec<bool> {
        let mut marks = vec![false; self.nodes.len()];
        let mut queue: Vec<usize> = self.index_of(tip).into_iter().collect();
        while let Some(c) = queue.pop() {
            if std::mem::replace(&mut marks[c], true) {
                continue;
            }
            for edge in &self.parent_edges[c] {
                if edge.connected
                    && let Some(p) = edge.target_node_idx
                {
                    queue.push(p);
                }
            }
        }
        for (i, dead) in self.tombstoned.iter().enumerate() {
            if *dead {
                marks[i] = false;
            }
        }
        marks
    }

    /// `true` if any of `id`'s recorded parents is not CONNECTED — the traversal cut history
    /// here (limits, integrated stop-early), so ancestry continues beyond the arena.
    pub fn has_cut_parents(&self, id: gix::ObjectId) -> bool {
        self.parent_edges_of(id)
            .is_some_and(|edges| edges.iter().any(|edge| !edge.connected))
    }

    /// [`Self::has_cut_parents`] by handle.
    pub fn has_cut_parents_at(&self, idx: usize) -> bool {
        self.parent_edges[idx].iter().any(|edge| !edge.connected)
    }

    /// The commit that `ref_name` points at, if present in the arena.
    pub fn commit_by_ref(&self, ref_name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        self.by_ref
            .get(ref_name)
            .filter(|&&idx| !self.tombstoned[idx])
            .map(|&idx| self.nodes[idx].id)
    }

    /// The reference names pointing at `id`.
    pub(crate) fn refs_at(&self, id: gix::ObjectId) -> Vec<gix::refs::FullName> {
        self.node(id)
            .map(|n| n.refs.iter().map(|r| r.ref_name.clone()).collect())
            .unwrap_or_default()
    }

    /// The parents of `id` that are present in the arena, first-parent first.
    pub fn parents(&self, id: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.parent_edges_of(id)
            .unwrap_or_default()
            .iter()
            .filter_map(|edge| edge.target_node_idx.map(|idx| self.nodes[idx].id))
    }

    /// The parents of `id` the traversal actually followed, first-parent first — present
    /// parents minus severed edges (limits, integrated stop-early).
    pub fn connected_parents(&self, id: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.parent_edges_of(id)
            .unwrap_or_default()
            .iter()
            .filter(|edge| edge.connected)
            .filter_map(|edge| edge.target_node_idx.map(|idx| self.nodes[idx].id))
    }

    /// The first parent of `id` (the next commit walking down first-parent), if present.
    pub fn first_parent(&self, id: gix::ObjectId) -> Option<gix::ObjectId> {
        let edge = self.parent_edges_of(id)?.first()?;
        let target = edge.target_node_idx.filter(|_| edge.connected)?;
        Some(self.nodes[target].id)
    }

    // --- HANDLE-based reads: the builder's hot loops speak node indices (no `by_id` hashing, no
    // per-call allocation). No tombstone substitution — the builder only sees walk-built or
    // compacted graphs (the write-through seam compacts before building).

    /// The id of the (live) node at `idx`.
    pub(crate) fn id_at(&self, idx: usize) -> gix::ObjectId {
        self.nodes[idx].id
    }

    /// The commit at `idx`.
    pub(crate) fn node_at(&self, idx: usize) -> &Commit {
        &self.nodes[idx]
    }

    /// The CONNECTED, PRESENT parents of `idx` in parent number order — the handle-space read behind
    /// [`Self::all_parent_ids`]'s fast path.
    pub(crate) fn connected_parents_at(&self, idx: usize) -> impl Iterator<Item = usize> + '_ {
        debug_assert!(
            !self.has_tombstoned_parent(idx),
            "no substitution by handle"
        );
        self.parent_edges[idx]
            .iter()
            .filter(|edge| edge.connected)
            .filter_map(|edge| edge.target_node_idx)
    }

    /// `all_parent_ids(id_at(idx)).len()` without materializing the list — connected parent numbers,
    /// absent targets included.
    pub(crate) fn connected_parent_count_at(&self, idx: usize) -> usize {
        debug_assert!(
            !self.has_tombstoned_parent(idx),
            "no substitution by handle"
        );
        self.parent_edges[idx]
            .iter()
            .filter(|edge| edge.connected)
            .count()
    }

    /// [`Self::first_parent`] by handle.
    pub(crate) fn first_parent_at(&self, idx: usize) -> Option<usize> {
        let edge = self.parent_edges[idx].first()?;
        edge.target_node_idx.filter(|_| edge.connected)
    }

    /// [`Self::children`] by handle.
    pub(crate) fn children_at(&self, idx: usize) -> &[usize] {
        &self.children[idx]
    }

    /// `marks[idx]`: whether `target` is an ancestor of the node (itself included), following
    /// CONNECTED parent edges — one linear pass instead of one graph walk per query.
    pub(crate) fn reaches_marks(&self, target: gix::ObjectId) -> Vec<bool> {
        let mut marks = vec![false; self.nodes.len()];
        let Some(t) = self.index_of(target) else {
            return marks;
        };
        marks[t] = true;
        for idx in self.toposort_parents_first() {
            if !marks[idx] {
                marks[idx] = self.connected_parents_at(idx).any(|p| marks[p]);
            }
        }
        marks
    }

    /// Recompute `generation` for every node (longest path from a root, by Kahn order). Cheap; the
    /// arena is small.
    pub(crate) fn recompute_generations(&mut self) {
        // Parents-first order so a child's generation is max(parent generations) + 1.
        let order = self.toposort_parents_first();
        for idx in order {
            let generation = self.parent_edges[idx]
                .iter()
                .filter_map(|edge| edge.target_node_idx)
                .map(|pidx| self.generations[pidx] + 1)
                .max()
                .unwrap_or(0);
            self.generations[idx] = generation;
        }
    }

    /// Topological order with parents before children (history order), over PRESENT parent numbers —
    /// connectivity ignored, like the generation formula. Propagating via the connected-only
    /// `children` adjacency would strand nodes behind severed edges.
    fn toposort_parents_first(&self) -> Vec<usize> {
        let mut children = vec![Vec::new(); self.nodes.len()];
        let mut indegree = vec![0usize; self.nodes.len()];
        for (idx, edges) in self.parent_edges.iter().enumerate() {
            for edge in edges {
                if let Some(pidx) = edge.target_node_idx {
                    children[pidx].push(idx);
                    indegree[idx] += 1;
                }
            }
        }
        let mut queue: std::collections::VecDeque<usize> = (0..self.nodes.len())
            .filter(|&i| indegree[i] == 0)
            .collect();
        let mut out = Vec::with_capacity(self.nodes.len());
        while let Some(idx) = queue.pop_front() {
            out.push(idx);
            for &child in &children[idx] {
                indegree[child] -= 1;
                if indegree[child] == 0 {
                    queue.push_back(child);
                }
            }
        }
        out
    }

    // --- The EDITOR MUTATION SURFACE ---
    //
    // but-rebase's editor mutates a CommitGraph in place: node indices are the stable
    // ids, `ObjectId` is payload. These writes maintain `by_id`, the children adjacency, and
    // the raw `parent_ids` payload. `generation` is creation-time data and NOT maintained.

    /// Append a fresh node with `id` as payload (`None` = born tombstoned) and no parents.
    pub fn add_node(&mut self, id: Option<gix::ObjectId>) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(Commit {
            id: id.unwrap_or_else(|| gix::ObjectId::null(gix::hash::Kind::Sha1)),
            parent_ids: Vec::new(),
            flags: CommitFlags::empty(),
            refs: Vec::new(),
        });
        self.generations.push(0);
        self.parent_edges.push(Vec::new());
        self.children.push(Vec::new());
        self.tombstoned.push(id.is_none());
        if let Some(id) = id {
            self.by_id.insert(id, idx);
        }
        idx
    }

    /// Overwrite the node's payload id — `None` tombstones it in place (the stale payload is
    /// retained, id-based lookups stop finding it), `Some` (re)vitalizes it.
    pub fn set_node_id(&mut self, idx: usize, id: Option<gix::ObjectId>) {
        match id {
            Some(id) => {
                self.tombstoned[idx] = false;
                self.set_commit_id(idx, id);
            }
            None => {
                let old = self.nodes[idx].id;
                if self.by_id.get(&old) == Some(&idx) {
                    self.by_id.remove(&old);
                }
                self.tombstoned[idx] = true;
            }
        }
    }

    /// Rewrite the commit id at `idx` IN PLACE — THE rebase write. The node index, its parent numbers,
    /// and its children survive; `by_id`, the children's raw `parent_ids` entries, and the
    /// id-addressed markers (entrypoint, managed-ws) follow the payload.
    pub fn set_commit_id(&mut self, idx: usize, id: gix::ObjectId) {
        let old = self.nodes[idx].id;
        self.nodes[idx].id = id;
        if self.by_id.get(&old) == Some(&idx) {
            self.by_id.remove(&old);
        }
        self.by_id.insert(id, idx);
        for child in self.children[idx].clone() {
            for (parent_number, edge) in self.parent_edges[child].iter().enumerate() {
                if edge.target_node_idx == Some(idx) {
                    self.nodes[child].parent_ids[parent_number] = id;
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

    /// Overwrite `idx`'s whole parent array — the editor's ordered-parent number write. Every new parent number
    /// is PRESENT and CONNECTED; the raw `parent_ids` payload derives from the targets and the
    /// children adjacency follows.
    pub fn set_parents(&mut self, idx: usize, parents: Vec<usize>) {
        for edge in std::mem::take(&mut self.parent_edges[idx]) {
            if let Some(t) = edge.target_node_idx
                && let Some(pos) = self.children[t].iter().position(|&c| c == idx)
            {
                self.children[t].remove(pos);
            }
        }
        self.nodes[idx].parent_ids = parents.iter().map(|&p| self.nodes[p].id).collect();
        self.parent_edges[idx] = parents
            .iter()
            .map(|&p| ParentEdge {
                target_node_idx: Some(p),
                connected: true,
            })
            .collect();
        for &p in &parents {
            self.children[p].push(idx);
        }
    }

    /// Reconcile every live node's parents with the odb — the write-through seam's edge refresh;
    /// after materialization the odb is authoritative (as-is picks carry no arena edges,
    /// `preserved_parents` picks were written with overridden parents). A node is kept only when
    /// its RAW parents equal its odb parents AND no connected parent edge targets a tombstone — the
    /// projection reads the raw payload, so a stale dropped id is as divergent as a wrong edge.
    /// Walk cuts (absent edge targets with odb-true payload) survive; anything else is rewired to the
    /// odb parents, adding or reviving missing commits recursively.
    pub(crate) fn complete_parents_from_odb(
        &mut self,
        repo: &crate::walk::overlay::OverlayRepo<'_>,
    ) -> anyhow::Result<()> {
        let mut queue: Vec<gix::ObjectId> = (0..self.node_count())
            .filter_map(|idx| self.node_payload(idx))
            .collect();
        // Tombstone payload → smallest arena index, consumed on revival — replaces an
        // arena scan per odb parent. Stays exact: this loop only revives or adds nodes.
        let mut tombstones_by_id: HashMap<gix::ObjectId, usize> = HashMap::default();
        for idx in (0..self.nodes.len()).rev() {
            if self.tombstoned[idx] {
                tombstones_by_id.insert(self.nodes[idx].id, idx);
            }
        }
        while let Some(id) = queue.pop() {
            let idx = self.index_of(id).expect("queued ids are live");
            let Ok(commit) = repo.find_commit(id) else {
                continue;
            };
            let mut odb_parents: Vec<_> = commit.parent_ids().map(|p| p.detach()).collect();
            // Collapse exact duplicate parents like the walk and the graph reader do (a
            // workspace merge encodes empty stacks as repeated parents).
            if odb_parents.len() > 1 {
                let mut deduped = Vec::with_capacity(odb_parents.len());
                for p in odb_parents {
                    if !deduped.contains(&p) {
                        deduped.push(p);
                    }
                }
                odb_parents = deduped;
            }
            if self.raw_parent_ids(idx) == odb_parents && !self.has_tombstoned_parent(idx) {
                continue;
            }
            let mut revived = Vec::new();
            let parent_indices = odb_parents
                .iter()
                .map(|&p| {
                    if self.index_of(p).is_none()
                        && let Some(idx) = tombstones_by_id.remove(&p)
                    {
                        self.tombstoned[idx] = false;
                        self.by_id.insert(p, idx);
                        revived.push(p);
                    }
                    self.index_of(p).unwrap_or_else(|| {
                        queue.push(p);
                        self.add_node(Some(p))
                    })
                })
                .collect();
            self.set_parents(idx, parent_indices);
            // A revived node wasn't in the initial sweep — validate it now. Its children need no
            // re-queue: any kept child already passed the tombstone-free check.
            queue.extend(revived);
        }
        Ok(())
    }

    /// Clear the walk's GOAL bits (bits beyond [`CommitFlags::all()`](crate::CommitFlags)).
    /// Goal numbering is traversal-order ephemera, and nothing after the walk reads goals.
    pub(crate) fn strip_goal_flags(&mut self) {
        for node in &mut self.nodes {
            node.flags &= crate::CommitFlags::all();
        }
    }

    /// Drop tombstoned nodes and reindex, leaving an arena indistinguishable from one built
    /// without them — a carried graph must not leak editor tombstones to the next consumer.
    /// The caller must first ensure no live parent edge targets a tombstone (the seam's odb
    /// reconciliation guarantees it); such an edge would degrade to "parent outside the graph".
    pub fn compact(&mut self) {
        if !self.tombstoned.iter().any(|&t| t) {
            return;
        }
        let mut remap: Vec<Option<usize>> = vec![None; self.nodes.len()];
        let mut next = 0;
        for (idx, remapped) in remap.iter_mut().enumerate() {
            if !self.tombstoned[idx] {
                *remapped = Some(next);
                next += 1;
            }
        }
        let mut generations = Vec::with_capacity(next);
        let mut parent_edges = Vec::with_capacity(next);
        for idx in 0..self.nodes.len() {
            if remap[idx].is_none() {
                continue;
            }
            generations.push(self.generations[idx]);
            parent_edges.push(
                self.parent_edges[idx]
                    .iter()
                    .map(|edge| ParentEdge {
                        target_node_idx: edge.target_node_idx.and_then(|t| remap[t]),
                        connected: edge.connected,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let nodes: Vec<Commit> = std::mem::take(&mut self.nodes)
            .into_iter()
            .enumerate()
            .filter_map(|(idx, n)| remap[idx].is_some().then_some(n))
            .collect();
        let by_id = Self::index_by_id(&nodes);
        // PRESENT-target children: unlike the walk, a severed-but-present edge keeps its
        // child entry here — callers of `children_at` on compacted graphs see it.
        let children = Self::derive_children(&parent_edges, false);
        self.managed_ws_commits.retain(|id| by_id.contains_key(id));
        self.nodes = nodes;
        self.generations = generations;
        self.by_id = by_id;
        self.parent_edges = parent_edges;
        self.children = children;
        self.tombstoned = vec![false; next];
        self.rebuild_by_ref();
    }

    /// Arena length, tombstones included.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The payload id at `idx` — `None` for tombstones.
    pub fn node_payload(&self, idx: usize) -> Option<gix::ObjectId> {
        (!self.tombstoned[idx]).then(|| self.nodes[idx].id)
    }

    /// The parent TARGETS of `idx` in parent number order. Only meaningful once every parent number is
    /// editor-authored (present) — walk-built graphs can have absent parent numbers.
    pub fn parent_indices(&self, idx: usize) -> Vec<usize> {
        self.parent_edges[idx]
            .iter()
            .map(|edge| {
                edge.target_node_idx
                    .expect("editor-authored edges are present")
            })
            .collect()
    }

    /// The PRESENT parent targets of `idx` in parent number order — absent (walk-cut) parent numbers are
    /// skipped, unlike [`Self::parent_indices`] which requires every target to be present.
    pub fn present_parent_indices(&self, idx: usize) -> Vec<usize> {
        self.parent_edges[idx]
            .iter()
            .filter_map(|edge| edge.target_node_idx)
            .filter(|&pidx| !self.tombstoned[pidx])
            .collect()
    }

    /// Build from a set of commits, every raw edge connected; commits whose parents fall
    /// outside the set are roots of this partial subgraph.
    #[cfg(test)]
    fn from_commits(
        commits: impl IntoIterator<Item = Commit>,
        entrypoint: Option<gix::ObjectId>,
    ) -> Self {
        let nodes: Vec<Commit> = commits.into_iter().collect();
        let by_id = Self::index_by_id(&nodes);
        let parent_edges: Vec<Vec<ParentEdge>> = nodes
            .iter()
            .map(|n| {
                n.parent_ids
                    .iter()
                    .map(|parent| ParentEdge {
                        target_node_idx: by_id.get(parent).copied(),
                        connected: true,
                    })
                    .collect()
            })
            .collect();
        let mut table = Self::from_parts(nodes, by_id, parent_edges);
        table.entrypoint = entrypoint;
        table
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
        let idx1 = g.index_of(id(1)).unwrap();
        assert_eq!(g.children_at(idx1), &[g.index_of(id(2)).unwrap()]);
        // Generation increases with history depth.
        assert_eq!(g.generation_of(id(1)), Some(0));
        assert_eq!(g.generation_of(id(3)), Some(2));
    }
}
