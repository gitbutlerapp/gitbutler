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

use std::collections::{HashMap, HashSet};

use bstr::ByteSlice;
use gix::reference::Category;

use crate::{Commit, CommitFlags};

/// An index into a [`CommitGraph`]'s node arena.
pub type CommitIdx = usize;

/// A plain (non-`gitbutler/*`) local branch ref — a "non-remote" tip for flag seeding.
fn is_plain_local_branch(rn: &gix::refs::FullName) -> bool {
    let rn = rn.as_ref();
    rn.category() == Some(Category::LocalBranch)
        && !rn.as_bstr().starts_with_str("refs/heads/gitbutler/")
}

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
}

impl CommitGraph {
    /// Build from a set of commits (as produced by the gix traversal). Commits whose parents are
    /// outside the set are simply roots of this subgraph (a partial graph), mirroring how the
    /// StepGraph handles missing parents via `preserved_parents`.
    pub fn from_commits(
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
        };
        graph.recompute_generations();
        graph
    }

    /// Bridge (spike): build a commit graph from the existing segment graph, so the StepGraph and
    /// projection builders can be exercised against it without first rewriting traversal. Every
    /// segment's commits become nodes; their `parent_ids` are the edges, and the entrypoint commit
    /// carries over.
    pub fn from_segment_graph(graph: &crate::Graph) -> Self {
        let ep = graph.entrypoint().ok();
        let entrypoint = ep
            .as_ref()
            .and_then(|ep| ep.commit_and_owner.map(|(c, _)| c.id));
        // The entrypoint segment's own ref names it (e.g. a checkout of a specific branch inside a
        // stack); the owner is the segment holding the entrypoint commit.
        let entrypoint_ref = ep
            .as_ref()
            .and_then(|ep| ep.commit_and_owner)
            .and_then(|(_, owner)| owner.ref_info.as_ref().map(|ri| ri.ref_name.clone()));
        let mut commits = Vec::new();
        for s in graph.node_weights() {
            for (i, c) in s.commits.iter().enumerate() {
                let mut c = c.clone();
                // The segment graph hoists the tip ref onto `segment.ref_info`; in a commit graph a
                // ref belongs on the commit it points at — the segment's first (tip) commit.
                if i == 0
                    && let Some(ri) = &s.ref_info
                    && !c.refs.iter().any(|r| r.ref_name == ri.ref_name)
                {
                    c.refs.insert(0, ri.clone());
                }
                commits.push(c);
            }
        }
        let mut cg = CommitGraph::from_commits(commits, entrypoint);
        cg.entrypoint_ref = entrypoint_ref;
        cg
    }

    /// KEYSTONE SPIKE: build a commit graph straight from git — no segment graph at all. Resolves the
    /// workspace ref, walks from the workspace commit and every ref tip, and attaches the refs
    /// pointing at each commit. This is what lets the segment graph eventually be deleted: the
    /// `CommitGraph` no longer needs to come *from* it (cf. [`Self::from_segment_graph`]).
    ///
    /// Spike scope: walks the full reachable history (no bounding at the base) and leaves flags empty;
    /// both are fine for the projection (which only reads above the base) and for proving the build.
    pub fn from_repository(repo: &gix::Repository) -> anyhow::Result<Self> {
        Self::from_repository_with_limit(repo, None)
    }

    /// Like [`Self::from_repository`], but bounding the LOCAL walk to about `commits_limit_hint`
    /// commits below each local tip. Remote tips stay unbounded — they must be able to find their
    /// local counterparts independently of the limit, exactly like the walk.
    pub fn from_repository_with_limit(
        repo: &gix::Repository,
        commits_limit_hint: Option<usize>,
    ) -> anyhow::Result<Self> {
        let ws_ref_name: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
        let ws_commit = repo
            .find_reference(&ws_ref_name)?
            .peel_to_commit()?
            .id()
            .detach();
        Self::from_repository_seeded(repo, Some(ws_commit), Some(ws_commit), commits_limit_hint)
    }

    /// Like [`Self::from_repository`], but for a non-managed checkout: there is no workspace ref, so no
    /// commit carries [`InWorkspace`](crate::CommitFlags::InWorkspace). `head_tip` seeds the walk (and
    /// `NotInRemote`) so the checked-out history is included even if no local branch names it.
    pub fn from_repository_unmanaged(
        repo: &gix::Repository,
        head_tip: Option<gix::ObjectId>,
    ) -> anyhow::Result<Self> {
        Self::from_repository_seeded(repo, head_tip, None, None)
    }

    /// Shared builder. `head_seed` roots the walk / `NotInRemote` / entrypoint (the workspace octopus
    /// merge when managed, else the checked-out tip). `ws_commit` marks
    /// [`InWorkspace`](crate::CommitFlags::InWorkspace) — `None` for a non-managed checkout.
    fn from_repository_seeded(
        repo: &gix::Repository,
        head_seed: Option<gix::ObjectId>,
        ws_commit: Option<gix::ObjectId>,
        commits_limit_hint: Option<usize>,
    ) -> anyhow::Result<Self> {
        // Refs pointing at each commit (heads + remotes, peeled).
        let mut refs_by_commit: HashMap<gix::ObjectId, Vec<gix::refs::FullName>> = HashMap::new();
        for mut reference in repo.references()?.all()?.filter_map(Result::ok) {
            if let Ok(id) = reference.peel_to_id() {
                refs_by_commit
                    .entry(id.detach())
                    .or_default()
                    .push(reference.name().to_owned());
            }
        }

        // Walk from the workspace/head commit AND every ref tip, so commits a remote-tracking branch is
        // ahead by (not reachable from the workspace commit) are included too.
        let seeds: Vec<gix::ObjectId> = head_seed
            .into_iter()
            .chain(refs_by_commit.keys().copied())
            .collect();
        // With a limit, restrict the LOCAL side to about `limit` commits below the head/workspace
        // commit (a budgeted BFS, taking the best budget a commit is reached with). Other local tips
        // are not seeded at all — the walk only walks its traversal tips under a limit. Remote tips
        // are unbounded so they can find their local counterparts, matching the walk's limit
        // semantics.
        let include: Option<HashSet<gix::ObjectId>> = match commits_limit_hint {
            None => None,
            Some(limit) => {
                let mut best: HashMap<gix::ObjectId, usize> = HashMap::new();
                let mut queue: std::collections::VecDeque<(gix::ObjectId, usize)> =
                    std::collections::VecDeque::new();
                for (&id, refs) in &refs_by_commit {
                    let is_remote = refs
                        .iter()
                        .any(|r| r.category() == Some(gix::reference::Category::RemoteBranch));
                    if is_remote {
                        queue.push_back((id, usize::MAX));
                    }
                }
                if let Some(head) = head_seed {
                    queue.push_back((head, limit));
                }
                while let Some((id, budget)) = queue.pop_front() {
                    match best.get(&id) {
                        Some(&b) if b >= budget => continue,
                        _ => {
                            best.insert(id, budget);
                        }
                    }
                    if budget == 0 {
                        continue;
                    }
                    let next = if budget == usize::MAX {
                        usize::MAX
                    } else {
                        budget - 1
                    };
                    if let Ok(commit) = repo.find_commit(id) {
                        for p in commit.parent_ids() {
                            queue.push_back((p.detach(), next));
                        }
                    }
                }
                Some(best.into_keys().collect())
            }
        };
        let mut commits = Vec::new();
        let mut managed_ws_commits = HashSet::new();
        for info in repo.rev_walk(seeds).all()? {
            let id = info?.id;
            if let Some(include) = &include
                && !include.contains(&id)
            {
                continue;
            }
            let commit = repo.find_commit(id)?;
            // Collapse EXACT duplicate parents (a GitButler workspace merge encodes empty lanes as
            // repeated parents, e.g. `[base, base]`). Lanes are derived from workspace metadata here,
            // so the repeated edge is pure redundancy — dropping it at the source avoids emitting
            // duplicate connections downstream. Distinct parents (real merges) are preserved in order.
            let mut parent_ids: Vec<gix::ObjectId> = Vec::new();
            for p in commit.parent_ids() {
                let p = p.detach();
                // A limit-excluded parent is cut off entirely, like a shallow boundary.
                if let Some(include) = &include
                    && !include.contains(&p)
                {
                    continue;
                }
                if !parent_ids.contains(&p) {
                    parent_ids.push(p);
                }
            }
            let refs = refs_by_commit
                .get(&id)
                .into_iter()
                .flatten()
                .map(|ref_name| crate::RefInfo {
                    ref_name: ref_name.clone(),
                    commit_id: None,
                    worktree: None,
                })
                .collect();
            // A GitButler-managed workspace commit is recognised by its message; a workspace ref that
            // has advanced past it points at a normal commit that is not in this set.
            if let Ok(message) = commit.message_raw()
                && crate::workspace::commit::is_managed_workspace_by_message(message)
            {
                managed_ws_commits.insert(id);
            }
            commits.push(crate::Commit {
                id,
                parent_ids,
                flags: crate::CommitFlags::empty(),
                refs,
            });
        }
        let mut cg = CommitGraph::from_commits(commits, head_seed);
        cg.managed_ws_commits = managed_ws_commits;

        // Reachability flags (each seeded on a tip, propagated to its ancestors — a commit carries a
        // flag iff it is an ancestor-or-self of a seed of that kind). See `CommitFlags`.
        // InWorkspace: reachable from the workspace tip (managed checkout only).
        if let Some(ws_commit) = ws_commit {
            cg.mark_ancestors([ws_commit], crate::CommitFlags::InWorkspace);
        }
        // NotInRemote (negative): reachable from any NON-remote tip — the workspace/head commit and
        // every local branch. A commit reachable only from remote-tracking tips stays remote-only.
        let local_tips: Vec<gix::ObjectId> = refs_by_commit
            .iter()
            .filter(|(_, refs)| refs.iter().any(|r| is_plain_local_branch(r)))
            .map(|(id, _)| *id)
            .collect();
        cg.mark_ancestors(
            head_seed.into_iter().chain(local_tips),
            crate::CommitFlags::NotInRemote,
        );
        // ShallowBoundary: the repository's shallow (grafted) commits.
        if let Ok(shallow) = repo.shallow_commits()
            && let Some(shallow) = shallow
        {
            for id in shallow.iter() {
                cg.set_flag(*id, crate::CommitFlags::ShallowBoundary);
            }
        }
        Ok(cg)
    }

    /// Set `flag` on every ancestor (inclusive) of any present `seed`, walking `parent_ids`.
    fn mark_ancestors(
        &mut self,
        seeds: impl IntoIterator<Item = gix::ObjectId>,
        flag: crate::CommitFlags,
    ) {
        let mut seen = std::collections::HashSet::new();
        let mut stack: Vec<gix::ObjectId> = seeds.into_iter().collect();
        while let Some(id) = stack.pop() {
            let Some(&idx) = self.by_id.get(&id) else {
                continue;
            };
            if !seen.insert(id) {
                continue;
            }
            self.nodes[idx].commit.flags |= flag;
            stack.extend(self.nodes[idx].commit.parent_ids.iter().copied());
        }
    }

    /// Set `flag` on a single commit, if present.
    fn set_flag(&mut self, id: gix::ObjectId, flag: crate::CommitFlags) {
        if let Some(&idx) = self.by_id.get(&id) {
            self.nodes[idx].commit.flags |= flag;
        }
    }

    /// Mark commits `Integrated` — reachable from the target (e.g. `origin/main`). Separate from
    /// [`Self::from_repository`] because the target comes from workspace metadata, not the repo alone.
    pub fn mark_integrated(&mut self, target: Option<gix::ObjectId>) {
        if let Some(target) = target {
            self.mark_ancestors([target], crate::CommitFlags::Integrated);
        }
    }

    /// Recompute `NotInRemote` from the given seeds only. The walk seeds it from its traversal TIPS
    /// (workspace commit, metadata stack branches, tracked locals, the entrypoint) — a stray local
    /// branch that is only reachable inside a remote's ahead region does NOT make those commits local.
    pub fn remark_not_in_remote(&mut self, seeds: impl IntoIterator<Item = gix::ObjectId>) {
        for node in &mut self.nodes {
            node.commit.flags.remove(crate::CommitFlags::NotInRemote);
        }
        self.mark_ancestors(seeds, crate::CommitFlags::NotInRemote);
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
    pub fn is_managed_ws_commit(&self, id: gix::ObjectId) -> bool {
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

    /// The commit's full parent list, first-parent first, INCLUDING parents not present in this
    /// graph (a partial traversal) — callers preserve those rather than re-pointing them.
    pub fn all_parent_ids(&self, id: gix::ObjectId) -> Vec<gix::ObjectId> {
        self.node(id)
            .map(|n| n.commit.parent_ids.clone())
            .unwrap_or_default()
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
    pub fn refs_at(&self, id: gix::ObjectId) -> Vec<gix::refs::FullName> {
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
            .filter(|p| self.by_id.contains_key(p))
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
        let has_local_branch_ref = n
            .commit
            .ref_name_iter()
            .any(|rn| rn.category() == Some(gix::reference::Category::LocalBranch));
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
        // No boundaries on a plain linear chain → the whole thing is one run.
        assert_eq!(g.first_parent_run(id(3)), vec![id(3), id(2), id(1)]);
    }

    #[test]
    fn bridge_from_segment_graph_captures_commits_and_parents() {
        // Build a tiny real segment graph: segment A (a2 -> a1) on base segment B (b0).
        let mut graph = crate::Graph::default();
        let a = graph.insert_segment_set_entrypoint(crate::Segment {
            commits: vec![commit(0xA2, &[0xA1]), commit(0xA1, &[0xB0])],
            ..Default::default()
        });
        graph.connect_new_segment(
            a,
            1, // from a1 (A's second commit)
            crate::Segment {
                commits: vec![commit(0xB0, &[])],
                ..Default::default()
            },
            0,
            id(0xB0),
        );

        let cg = CommitGraph::from_segment_graph(&graph);
        // All three commits made it across, with their parent edges intact.
        assert!(
            cg.node(id(0xA2)).is_some()
                && cg.node(id(0xA1)).is_some()
                && cg.node(id(0xB0)).is_some()
        );
        assert_eq!(cg.first_parent(id(0xA2)), Some(id(0xA1)));
        assert_eq!(cg.first_parent(id(0xA1)), Some(id(0xB0)));
        assert_eq!(cg.first_parent(id(0xB0)), None);
        // Reverse adjacency derived correctly.
        assert_eq!(cg.children(id(0xB0)).collect::<Vec<_>>(), vec![id(0xA1)]);
        // Entrypoint commit carried over (A is the entrypoint segment; its tip is a2).
        assert_eq!(cg.entrypoint, Some(id(0xA2)));
    }

    #[test]
    fn merge_is_a_segment_boundary_so_the_run_stops() {
        // 4 is a merge of 2 and 3; both descend from 1.
        //   4 -> [2, 3] ; 2 -> 1 ; 3 -> 1
        let g = CommitGraph::from_commits(
            [
                commit(4, &[2, 3]),
                commit(2, &[1]),
                commit(3, &[1]),
                commit(1, &[]),
            ],
            Some(id(4)),
        );
        assert!(
            g.is_segment_boundary(id(4)),
            "merge commit starts its own segment"
        );
        assert!(
            g.is_segment_boundary(id(1)),
            "1 has two children (2 and 3) → branch point, a boundary"
        );
        // First-parent run from the merge: 4, then first-parent 2, then stop before boundary 1.
        assert_eq!(g.first_parent_run(id(4)), vec![id(4), id(2)]);
    }
}
