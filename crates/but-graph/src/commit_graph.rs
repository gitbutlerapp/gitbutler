//! The commit graph: the raw traversal flattened into nodes — the single arena the
//! build authors its ref layout onto and every consumer reads (see `build`).
//!
//! Pipeline: `gix traversal → CommitGraph (+ ref layout) → projection`. The traversal
//! accumulates the arena directly ([`CommitGraph::from_walk`]), the build stores the ref
//! layout on it, the projection derives the [`Workspace`](crate::Workspace) from it, and
//! but-rebase's editor adopts the carried copy as its mutable arena. Both consumers are
//! commit/ref-granular: the build's segments never leave the builder.
//!
//! A node IS a commit ([`crate::Commit`]); every per-node attribute — parent entries, children,
//! tombstones, the topological `generation` — rides a parallel array. An parent entry is
//! `commit → parent` from `parent_ids` (first-parent at index 0). A nice consequence: the
//! workspace commit's `parent_ids` array IS the stack order, so no order-stacks post-pass.

use gix::hashtable::{HashMap, HashSet};

use crate::{Commit, CommitFlags};

mod merge_base;

/// One `commit → parent` parent entry, INDEX-based: parallel to one entry of the commit's `parent_ids`.
/// The raw id in `parent_ids` is data; this is the graph structure.
#[derive(Debug, Clone, Copy)]
struct ResolvedParent {
    /// The parent's node, when it is present in the arena. `None` = the raw parent id points
    /// outside the arena (partial traversal — this subgraph roots here).
    commit: Option<usize>,
    /// Whether the traversal actually FOLLOWED this link. A parent can be present in the arena
    /// via another path while this specific parent entry was severed (limits, integrated stop-early).
    connected: bool,
}

/// The commit graph: an arena of commit nodes with INDEX-based `commit → parent` parent entries (one
/// `ResolvedParent` per raw `parent_ids` entry) and the reverse (`parent → child`) adjacency derived
/// for downward walks. `ObjectId` is pure data; `by_id` is a rebuildable lookup index.
#[derive(Debug, Clone, Default)]
pub struct CommitGraph {
    // ── The commit table: one row per commit, every column below indexed by the same
    // commit index and kept in lockstep. Appends go through `add_commit`/`add_tombstone`;
    // `compact` — the one operation that moves rows — rebuilds all columns together. ──
    commits: Vec<Commit>,
    /// Per commit: distance from a root; higher means deeper in history. Used where the
    /// projection picks "the lowest of several tips". Creation-time data
    /// (see `compute_generations`), not maintained by the editor mutation surface.
    generations: Vec<u32>,
    /// Per commit, one parent entry per `parent_ids` entry — an INNER parallel array, in
    /// parent-number order: presence and connectivity.
    parent_resolutions: Vec<Vec<ResolvedParent>>,
    /// Per commit: whether the EDITOR removed it (see the mutation surface). A tombstoned
    /// commit keeps its index and (stale) id; id-based reads skip it. Walk-built
    /// graphs have none.
    tombstoned: Vec<bool>,
    /// Per commit: the `parent → children` adjacency, derived at build time for
    /// branch-point detection and downward walks (the projection walks tip toward base).
    children: Vec<Vec<usize>>,

    // ── Rebuildable lookups over the table — they follow the data, never define it. ──
    by_id: HashMap<gix::ObjectId, usize>,
    /// Ref name → commit holding it. Like `by_id` a rebuildable lookup: rebuilt whenever refs
    /// are written (construction, [`Self::refresh_refs`]) or indices move ([`Self::compact`]).
    by_ref: std::collections::HashMap<gix::refs::FullName, usize>,

    // ── Walk-carried context: how this graph was built, carried forward by derived graphs. ──
    /// Where traversal/HEAD started; the projection uses it as a focus boundary.
    entrypoint: Option<gix::ObjectId>,
    /// The ref the entrypoint was checked out as, if any. When set, it names the entrypoint segment
    /// (overriding disambiguation), mirroring `from_tip(id, Some(ref))`.
    entrypoint_ref: Option<gix::refs::FullName>,
    /// Commits whose message marks them as a GitButler-managed workspace commit. Kept out of
    /// [`CommitFlags`](crate::CommitFlags) so it neither perturbs the walk's goal bits nor
    /// propagates to ancestors the way every flag there does; used to tell a real managed merge from a ws ref advanced past it.
    managed_ws_commits: HashSet<gix::ObjectId>,
    /// When built [from the walk](Self::from_walk): whether the traversal stopped queueing after
    /// hitting the hard limit. Derived graphs must carry it forward.
    pub(crate) hard_limit_hit: bool,
    /// When built [from the walk](Self::from_walk): whether the commit budget
    /// (`Options::commits_limit_hint`) cut an extent short with history left below it.
    /// Carried like [`Self::hard_limit_hit`] so the projection can tell a view that is EMPTY
    /// from one that was merely CUT — the two are indistinguishable from the stacks alone.
    pub(crate) limit_hint_hit: bool,
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
    /// The branches linked worktrees have checked out (`Options::worktree_tips`), carried
    /// like [`Self::extra_target`]. The build treats them like declared branches when
    /// carving segments, so a branch a worktree follows starts its own segment and stays
    /// addressable inside the workspace. (A worktree branch OUTSIDE the layout is the
    /// rebase editor's concern; it registers such refs itself.)
    pub(crate) worktree_refs: Vec<gix::refs::FullName>,
    /// The subset of [`worktree_refs`](Self::worktree_refs) another worktree has checked out
    /// — classified by the build for its own naming passes. Being checked out somewhere is
    /// transient state: it decides where the branch is drawn and which checkout follows a
    /// rewrite, never what the lanes contain. Such a branch never names a segment and never
    /// splices into a lane as a declared chain member; it stays a ref riding on its commit,
    /// presented through the worktree listing, so rewriting history relative to it never
    /// rewrites the lane it points into. Exempt is only the subject of this graph's view:
    /// the branch checked out by the repository that built the graph, and the entrypoint ref.
    pub(crate) foreign_worktree_refs: Vec<gix::refs::FullName>,
    /// The ref placement table authored at build time (see [`ref_layout`](crate::ref_layout));
    /// `None` until the commit graph goes through the builder. Mirrors build-time refs — like
    /// [`Commit::refs`](crate::Commit) it goes stale under editor mutation and is re-authored
    /// by the write-through projection.
    pub(crate) layout: Option<crate::ref_layout::RefLayout>,
}

/// The branches the caller's linked worktrees follow, in walk-option order.
fn worktree_ref_names(options: &crate::walk::Options) -> Vec<gix::refs::FullName> {
    options
        .worktree_tips
        .iter()
        .filter_map(|tip| tip.ref_name.clone())
        .collect()
}

impl CommitGraph {
    fn parent_resolutions_of(&self, id: gix::ObjectId) -> Option<&[ResolvedParent]> {
        self.by_id
            .get(&id)
            .map(|&idx| self.parent_resolutions[idx].as_slice())
    }

    /// The id → node lookup for `nodes` (ids are unique).
    fn index_by_id(nodes: &[Commit]) -> HashMap<gix::ObjectId, usize> {
        nodes
            .iter()
            .enumerate()
            .map(|(idx, n)| (n.id, idx))
            .collect()
    }

    /// The parent → children adjacency derived from `parent_resolutions`. `connected_only`
    /// mirrors the walk (a severed parent entry yields no child); [`Self::compact`] keeps every
    /// PRESENT target — see the note there.
    fn derive_children(
        parent_resolutions: &[Vec<ResolvedParent>],
        connected_only: bool,
    ) -> Vec<Vec<usize>> {
        let mut children = vec![Vec::new(); parent_resolutions.len()];
        for (idx, entries) in parent_resolutions.iter().enumerate() {
            for entry in entries {
                if let Some(pidx) = entry.commit
                    && (entry.connected || !connected_only)
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
        commits: Vec<Commit>,
        by_id: HashMap<gix::ObjectId, usize>,
        parent_resolutions: Vec<Vec<ResolvedParent>>,
    ) -> Self {
        let mut table = CommitGraph {
            generations: vec![0; commits.len()],
            children: Self::derive_children(&parent_resolutions, true),
            tombstoned: vec![false; commits.len()],
            commits,
            by_id,
            parent_resolutions,
            ..Default::default()
        };
        table.recompute_generations();
        table.rebuild_by_ref();
        table
    }

    /// Assemble from the walk's outcome (see `walk::walker`): the walk's `by_id` is adopted
    /// as-is (collection order IS node order), and parent-parent entry connectivity comes straight from the
    /// followed parent entries rather than being re-derived.
    #[tracing::instrument(name = "CommitGraph::from_walk_outcome", level = "trace", skip_all)]
    pub(crate) fn from_walk_outcome(o: crate::walk::walker::WalkOutcome) -> Self {
        let commits: Vec<Commit> = o.commits;
        let by_id = o.by_id;
        let parent_resolutions: Vec<Vec<ResolvedParent>> = commits
            .iter()
            .map(|n| {
                let followed = o.parents_followed.get(&n.id);
                n.parent_ids
                    .iter()
                    .map(|parent| ResolvedParent {
                        commit: by_id.get(parent).copied(),
                        connected: followed.is_some_and(|ps| ps.contains(parent)),
                    })
                    .collect()
            })
            .collect();
        let mut table = Self::from_parts(commits, by_id, parent_resolutions);
        table.entrypoint = o.entrypoint;
        table.entrypoint_ref = o.entrypoint_ref;
        table.hard_limit_hit = o.hard_limit_hit;
        table.limit_hint_hit = o.limit_hint_hit;
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
        let worktree_refs = worktree_ref_names(&options);
        let seeds = crate::walk::assemble::initial_seeds_from_workspace_metadata(
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
        outcome.worktree_refs = worktree_refs;
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
        let worktree_refs = worktree_ref_names(&options);
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
        outcome.worktree_refs = worktree_refs;
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
            && let Ok(decoded) = commit.decode()
            && crate::workspace::commit::is_managed_workspace_by_message(decoded.message)
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
        // The extra target is already among the seeds whenever the walk kept it — an
        // auxiliary seed another seed owns is dropped, and must not flag anything here.
        // A fresh walk's cut parent entries stay cut: the sweep follows only the parents the
        // traversal connected (the seam reconciles all parent entries before its sweeps, so
        // both views coincide there).
        self.set_flag_on_ancestors(CommitFlags::Integrated, integrated);
        self.set_flag_on_ancestors(CommitFlags::InWorkspace, ws);
        self.set_flag_on_ancestors(CommitFlags::NotInRemote, not_in_remote);
    }

    /// The worktrees referenced by any commit's refs.
    pub(crate) fn ref_worktrees(&self) -> impl Iterator<Item = &crate::Worktree> {
        self.commits
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

    /// The first managed workspace commit STRICTLY BELOW `from` along first parents,
    /// if any — the merge a workspace ref advanced past.
    pub(crate) fn first_parent_managed_ws_commit_below(
        &self,
        from: gix::ObjectId,
    ) -> Option<gix::ObjectId> {
        let mut cursor = from;
        for _ in 0..self.commits.len() {
            cursor = *self.node(cursor)?.parent_ids.first()?;
            if self.is_managed_ws_commit(cursor) {
                return Some(cursor);
            }
        }
        None
    }

    /// Replace every node's attached refs with the CURRENT `refs_by_id` state — the
    /// write-through seam's enrichment refresh: an editor-mutated graph still carries
    /// walk-time refs, but materialization has moved them. Matched entries are consumed.
    pub(crate) fn refresh_refs(
        &mut self,
        refs_by_id: &mut crate::walk::utils::RefsById,
        worktree_by_branch: &crate::walk::utils::WorktreeByBranch,
    ) {
        for (idx, node) in self.commits.iter_mut().enumerate() {
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
        Some(&mut self.commits[idx].refs)
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
        for (idx, node) in self.commits.iter().enumerate() {
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
        self.commits[idx].flags = flags;
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
        for (idx, node) in self.commits.iter_mut().enumerate() {
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
            (0..self.commits.len()).find(|&i| self.tombstoned[i] && self.commits[i].id == id)
        else {
            return false;
        };
        self.tombstoned[idx] = false;
        self.by_id.insert(id, idx);
        true
    }

    /// The commit holding `id`, if present.
    pub fn node(&self, id: gix::ObjectId) -> Option<&Commit> {
        self.by_id.get(&id).map(|&idx| &self.commits[idx])
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
        self.by_ref.get(name).map(|&idx| self.commits[idx].id)
    }

    /// The node index of `id`, if present.
    pub fn index_of(&self, id: gix::ObjectId) -> Option<usize> {
        self.by_id.get(&id).copied()
    }

    /// Every commit id in the arena, in node order.
    pub fn commit_ids(&self) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.commits.iter().map(|n| n.id)
    }

    /// The commit's CONNECTED parents, first-parent first — parents the traversal severed
    /// (limits, integrated stop-early, shallow boundaries) are omitted. A TOMBSTONED parent is
    /// substituted by its own parents, recursively (the editor's descent), deduplicated along
    /// the way; plain duplicate parents are all kept (dup-parent workspace commits).
    pub(crate) fn all_parent_ids(&self, id: gix::ObjectId) -> Vec<gix::ObjectId> {
        let Some(&idx) = self.by_id.get(&id) else {
            return Vec::new();
        };
        // Fast path: no tombstoned parent to substitute through (always true for walk-built
        // graphs), so the connected parent entries map straight to parent ids.
        if !self.has_tombstoned_parent(idx) {
            return self.commits[idx]
                .parent_ids
                .iter()
                .copied()
                .zip(&self.parent_resolutions[idx])
                .filter(|(_, entry)| entry.connected)
                .map(|(p, entry)| match entry.commit {
                    Some(t) => self.commits[t].id,
                    None => p,
                })
                .collect();
        }
        // The CONNECTED `(raw parent id, parent entry target)` pairs of `idx`, in parent-number order.
        let connected = |idx: usize| {
            self.commits[idx]
                .parent_ids
                .iter()
                .copied()
                .zip(&self.parent_resolutions[idx])
                .filter_map(|(p, entry)| entry.connected.then_some((p, entry.commit)))
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
                // A live target's CURRENT stored id is authoritative (raw agrees via
                // set_commit_id's child patching; this needs no such guarantee).
                Some(t) => parents.push(self.commits[t].id),
                None => parents.push(raw),
            }
        }
        parents
    }

    /// The RAW recorded parent ids of `idx` — the data array, cut parent entries included, no
    /// tombstone substitution (unlike [`Self::all_parent_ids`]).
    pub(crate) fn raw_parent_ids(&self, idx: usize) -> &[gix::ObjectId] {
        &self.commits[idx].parent_ids
    }

    /// `true` if any CONNECTED parent entry of `idx` targets a tombstone — traversal would
    /// substitute through it, so the raw recorded parents disagree with what a walk sees.
    pub(crate) fn has_tombstoned_parent(&self, idx: usize) -> bool {
        self.parent_resolutions[idx]
            .iter()
            .any(|entry| entry.connected && entry.commit.is_some_and(|t| self.tombstoned[t]))
    }

    /// All ancestors of `tip` (inclusive), following CONNECTED parent entries — history the
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

    /// [`Self::ancestor_set`] in index space: `marks[idx]` for every reachable LIVE node. The
    /// walk passes THROUGH tombstones without marking them, matching the id-space substitution.
    /// Query membership via [`Self::index_of`].
    pub fn ancestor_marks(&self, tip: gix::ObjectId) -> Vec<bool> {
        let mut marks = vec![false; self.commits.len()];
        let mut queue: Vec<usize> = self.index_of(tip).into_iter().collect();
        while let Some(c) = queue.pop() {
            if std::mem::replace(&mut marks[c], true) {
                continue;
            }
            for entry in &self.parent_resolutions[c] {
                if entry.connected
                    && let Some(p) = entry.commit
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
        self.parent_resolutions_of(id)
            .is_some_and(|entries| entries.iter().any(|entry| !entry.connected))
    }

    /// [`Self::has_cut_parents`] by index.
    pub fn has_cut_parents_at(&self, idx: usize) -> bool {
        self.parent_resolutions[idx]
            .iter()
            .any(|entry| !entry.connected)
    }

    /// The commit that `ref_name` points at, if present in the arena.
    pub fn commit_by_ref(&self, ref_name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        self.by_ref
            .get(ref_name)
            .filter(|&&idx| !self.tombstoned[idx])
            .map(|&idx| self.commits[idx].id)
    }

    /// The traversal's normalized seed tips, carried onto the graph — the walk's input as
    /// data. A seed need not have a node in the arena: a target's local proven behind the
    /// target is deliberately carried unwalked (see [`Self::behind_target_local_tip`]).
    pub fn seeds(&self) -> &[crate::walk::Seed] {
        &self.seeds
    }

    /// The recorded tip of the target's local branch when it was proven behind the target
    /// and therefore never walked to: the seed carries the fact instead of the arena.
    /// `None` whenever the ref was walked normally — [`Self::commit_by_ref`] answers then.
    pub fn behind_target_local_tip(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> Option<gix::ObjectId> {
        self.seeds.iter().find_map(|s| match &s.role {
            crate::walk::SeedRole::TargetLocal {
                local_ref_name,
                behind_target: true,
            } if local_ref_name.as_ref() == ref_name => Some(s.id),
            _ => None,
        })
    }

    /// The reference names pointing at `id`.
    pub(crate) fn refs_at(&self, id: gix::ObjectId) -> Vec<gix::refs::FullName> {
        self.node(id)
            .map(|n| n.refs.iter().map(|r| r.ref_name.clone()).collect())
            .unwrap_or_default()
    }

    /// The parents of `id` that are present in the arena, first-parent first.
    pub fn parents(&self, id: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.parent_resolutions_of(id)
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| entry.commit.map(|idx| self.commits[idx].id))
    }

    /// The parents of `id` the traversal actually followed, first-parent first — present
    /// parents minus severed parent entries (limits, integrated stop-early).
    pub fn connected_parents(&self, id: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.parent_resolutions_of(id)
            .unwrap_or_default()
            .iter()
            .filter(|entry| entry.connected)
            .filter_map(|entry| entry.commit.map(|idx| self.commits[idx].id))
    }

    /// The first parent of `id` (the next commit walking down first-parent), if present.
    pub fn first_parent(&self, id: gix::ObjectId) -> Option<gix::ObjectId> {
        let entry = self.parent_resolutions_of(id)?.first()?;
        let target = entry.commit.filter(|_| entry.connected)?;
        Some(self.commits[target].id)
    }

    /// The commits that have `id` as a parent — the reverse of [`Self::parents`], and the
    /// only way to ask whether history forks at a commit.
    pub fn children(&self, id: gix::ObjectId) -> Vec<gix::ObjectId> {
        self.index_of(id)
            .map(|idx| {
                self.children_at(idx)
                    .iter()
                    .map(|&child| self.commits[child].id)
                    .collect()
            })
            .unwrap_or_default()
    }

    // --- INDEX-based reads: the builder's hot loops speak node indices (no `by_id` hashing, no
    // per-call allocation). No tombstone substitution — the builder only sees walk-built or
    // compacted graphs (the write-through seam compacts before building).

    /// The id of the (live) node at `idx`.
    pub(crate) fn id_at(&self, idx: usize) -> gix::ObjectId {
        self.commits[idx].id
    }

    /// The commit at `idx`.
    pub(crate) fn node_at(&self, idx: usize) -> &Commit {
        &self.commits[idx]
    }

    /// The CONNECTED, PRESENT parents of `idx` in parent number order — the index-space read
    /// behind [`Self::connected_parents`].
    pub(crate) fn connected_parents_at(&self, idx: usize) -> impl Iterator<Item = usize> + '_ {
        debug_assert!(!self.has_tombstoned_parent(idx), "no substitution by index");
        self.parent_resolutions[idx]
            .iter()
            .filter(|entry| entry.connected)
            .filter_map(|entry| entry.commit)
    }

    /// [`Self::connected_parents`] counted without materializing the list — connected parent
    /// numbers, absent targets included.
    pub(crate) fn connected_parent_count_at(&self, idx: usize) -> usize {
        debug_assert!(!self.has_tombstoned_parent(idx), "no substitution by index");
        self.parent_resolutions[idx]
            .iter()
            .filter(|entry| entry.connected)
            .count()
    }

    /// [`Self::first_parent`] by index.
    pub(crate) fn first_parent_at(&self, idx: usize) -> Option<usize> {
        let entry = self.parent_resolutions[idx].first()?;
        entry.commit.filter(|_| entry.connected)
    }

    /// [`Self::children`] by index.
    pub(crate) fn children_at(&self, idx: usize) -> &[usize] {
        &self.children[idx]
    }

    /// `marks[idx]`: whether `target` is an ancestor of the node (itself included), following
    /// CONNECTED parent entries — one linear pass instead of one graph walk per query.
    pub(crate) fn reaches_marks(&self, target: gix::ObjectId) -> Vec<bool> {
        let mut marks = vec![false; self.commits.len()];
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
            let generation = self.parent_resolutions[idx]
                .iter()
                .filter_map(|entry| entry.commit)
                .map(|pidx| self.generations[pidx] + 1)
                .max()
                .unwrap_or(0);
            self.generations[idx] = generation;
        }
    }

    /// Topological order with parents before children (history order), over PRESENT parent numbers —
    /// connectivity ignored, like the generation formula. Propagating via the connected-only
    /// `children` adjacency would strand nodes behind severed parent entries.
    fn toposort_parents_first(&self) -> Vec<usize> {
        let mut children = vec![Vec::new(); self.commits.len()];
        let mut indegree = vec![0usize; self.commits.len()];
        for (idx, entries) in self.parent_resolutions.iter().enumerate() {
            for entry in entries {
                if let Some(pidx) = entry.commit {
                    children[pidx].push(idx);
                    indegree[idx] += 1;
                }
            }
        }
        let mut queue: std::collections::VecDeque<usize> = (0..self.commits.len())
            .filter(|&i| indegree[i] == 0)
            .collect();
        let mut out = Vec::with_capacity(self.commits.len());
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
    // ids, `ObjectId` is data. These writes maintain `by_id`, the children adjacency, and
    // the raw `parent_ids` data. `generation` is creation-time data and NOT maintained.

    /// Append a fresh node holding commit `id`, with no parents.
    pub fn add_commit(&mut self, id: gix::ObjectId) -> usize {
        let idx = self.push_commit(id);
        self.tombstoned.push(false);
        self.by_id.insert(id, idx);
        idx
    }

    /// Append a fresh node born tombstoned: a placeholder with no commit id and no parents.
    pub fn add_tombstone(&mut self) -> usize {
        let idx = self.push_commit(gix::ObjectId::null(gix::hash::Kind::Sha1));
        self.tombstoned.push(true);
        idx
    }

    fn push_commit(&mut self, id: gix::ObjectId) -> usize {
        let idx = self.commits.len();
        self.commits.push(Commit {
            id,
            parent_ids: Vec::new(),
            flags: CommitFlags::empty(),
            refs: Vec::new(),
        });
        self.generations.push(0);
        self.parent_resolutions.push(Vec::new());
        self.children.push(Vec::new());
        idx
    }

    /// Tombstone the node at `idx` in place: the stale id is retained, id-based
    /// lookups stop finding it.
    pub fn tombstone_commit(&mut self, idx: usize) {
        let old = self.commits[idx].id;
        if self.by_id.get(&old) == Some(&idx) {
            self.by_id.remove(&old);
        }
        self.tombstoned[idx] = true;
    }

    /// Revive the node at `idx` with commit `id`, undoing a tombstone.
    pub fn revive_commit(&mut self, idx: usize, id: gix::ObjectId) {
        self.tombstoned[idx] = false;
        self.set_commit_id(idx, id);
    }

    /// Rewrite the commit id at `idx` IN PLACE — THE rebase write. The node index, its parent numbers,
    /// and its children survive; `by_id`, the children's raw `parent_ids` entries, and the
    /// id-addressed markers (entrypoint, managed-ws) follow the id.
    pub fn set_commit_id(&mut self, idx: usize, id: gix::ObjectId) {
        let old = self.commits[idx].id;
        self.commits[idx].id = id;
        if self.by_id.get(&old) == Some(&idx) {
            self.by_id.remove(&old);
        }
        self.by_id.insert(id, idx);
        for child in self.children[idx].clone() {
            for (parent_number, entry) in self.parent_resolutions[child].iter().enumerate() {
                if entry.commit == Some(idx) {
                    self.commits[child].parent_ids[parent_number] = id;
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
    /// is PRESENT and CONNECTED; the raw `parent_ids` data derives from the targets and the
    /// children adjacency follows.
    pub fn set_parents(&mut self, idx: usize, parents: Vec<usize>) {
        for entry in std::mem::take(&mut self.parent_resolutions[idx]) {
            if let Some(t) = entry.commit
                && let Some(pos) = self.children[t].iter().position(|&c| c == idx)
            {
                self.children[t].remove(pos);
            }
        }
        self.commits[idx].parent_ids = parents.iter().map(|&p| self.commits[p].id).collect();
        self.parent_resolutions[idx] = parents
            .iter()
            .map(|&p| ResolvedParent {
                commit: Some(p),
                connected: true,
            })
            .collect();
        for &p in &parents {
            self.children[p].push(idx);
        }
    }

    /// Reconcile every live node's parents with the odb — the write-through seam's parent entry refresh;
    /// after materialization the odb is authoritative (as-is commits carry no arena parent entries,
    /// `preserved_parents` commits were written with overridden parents). A node is kept only when
    /// its RAW parents equal its odb parents AND no connected parent entry targets a tombstone — the
    /// projection reads the raw data, so a stale dropped id is as divergent as a wrong parent entry.
    /// Walk cuts (absent parent entry targets with odb-true ids) survive; anything else is rewired to the
    /// odb parents, adding or reviving missing commits recursively.
    pub(crate) fn complete_parents_from_odb(
        &mut self,
        repo: &crate::walk::overlay::OverlayRepo<'_>,
    ) -> anyhow::Result<()> {
        let mut queue: Vec<gix::ObjectId> = (0..self.commit_count())
            .filter_map(|idx| self.commit_id(idx))
            .collect();
        // Tombstone id → smallest arena index, consumed on revival — replaces an
        // arena scan per odb parent. Stays exact: this loop only revives or adds nodes.
        let mut tombstones_by_id: HashMap<gix::ObjectId, usize> = HashMap::default();
        for idx in (0..self.commits.len()).rev() {
            if self.tombstoned[idx] {
                tombstones_by_id.insert(self.commits[idx].id, idx);
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
                        self.add_commit(p)
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
        for node in &mut self.commits {
            node.flags &= crate::CommitFlags::all();
        }
    }

    /// Drop tombstoned nodes and reindex, leaving an arena indistinguishable from one built
    /// without them — a carried graph must not leak editor tombstones to the next consumer.
    /// The caller must first ensure no live parent entry targets a tombstone (the seam's odb
    /// reconciliation guarantees it); such an parent entry would degrade to "parent outside the graph".
    pub fn compact(&mut self) {
        if !self.tombstoned.iter().any(|&t| t) {
            return;
        }
        let mut remap: Vec<Option<usize>> = vec![None; self.commits.len()];
        let mut next = 0;
        for (idx, remapped) in remap.iter_mut().enumerate() {
            if !self.tombstoned[idx] {
                *remapped = Some(next);
                next += 1;
            }
        }
        let mut generations = Vec::with_capacity(next);
        let mut parent_resolutions = Vec::with_capacity(next);
        for idx in 0..self.commits.len() {
            if remap[idx].is_none() {
                continue;
            }
            generations.push(self.generations[idx]);
            parent_resolutions.push(
                self.parent_resolutions[idx]
                    .iter()
                    .map(|entry| ResolvedParent {
                        commit: entry.commit.and_then(|t| remap[t]),
                        connected: entry.connected,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let commits: Vec<Commit> = std::mem::take(&mut self.commits)
            .into_iter()
            .enumerate()
            .filter_map(|(idx, n)| remap[idx].is_some().then_some(n))
            .collect();
        let by_id = Self::index_by_id(&commits);
        // PRESENT-target children: unlike the walk, a severed-but-present parent entry keeps its
        // child entry here — callers of `children_at` on compacted graphs see it.
        let children = Self::derive_children(&parent_resolutions, false);
        self.managed_ws_commits.retain(|id| by_id.contains_key(id));
        self.commits = commits;
        self.generations = generations;
        self.by_id = by_id;
        self.parent_resolutions = parent_resolutions;
        self.children = children;
        self.tombstoned = vec![false; next];
        self.rebuild_by_ref();
    }

    /// Arena length, tombstones included.
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    /// The commit id at `idx` — `None` for tombstones: a removed node has no commit id.
    pub fn commit_id(&self, idx: usize) -> Option<gix::ObjectId> {
        (!self.tombstoned[idx]).then(|| self.commits[idx].id)
    }

    /// The parent TARGETS of `idx` in parent number order. Only meaningful once every parent number is
    /// editor-authored (present) — walk-built graphs can have absent parent numbers.
    pub fn parent_indices(&self, idx: usize) -> Vec<usize> {
        self.parent_resolutions[idx]
            .iter()
            .map(|entry| entry.commit.expect("editor-authored entries are present"))
            .collect()
    }

    /// The PRESENT parent targets of `idx` in parent number order — absent (walk-cut) parent numbers are
    /// skipped, unlike [`Self::parent_indices`] which requires every target to be present.
    pub fn present_parent_indices(&self, idx: usize) -> Vec<usize> {
        self.parent_resolutions[idx]
            .iter()
            .filter_map(|entry| entry.commit)
            .filter(|&pidx| !self.tombstoned[pidx])
            .collect()
    }

    /// Build from a set of commits, every raw parent entry connected; commits whose parents fall
    /// outside the set are roots of this partial subgraph.
    #[cfg(test)]
    fn from_commits(
        commits: impl IntoIterator<Item = Commit>,
        entrypoint: Option<gix::ObjectId>,
    ) -> Self {
        let commits: Vec<Commit> = commits.into_iter().collect();
        let by_id = Self::index_by_id(&commits);
        let parent_resolutions: Vec<Vec<ResolvedParent>> = commits
            .iter()
            .map(|n| {
                n.parent_ids
                    .iter()
                    .map(|parent| ResolvedParent {
                        commit: by_id.get(parent).copied(),
                        connected: true,
                    })
                    .collect()
            })
            .collect();
        let mut table = Self::from_parts(commits, by_id, parent_resolutions);
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
