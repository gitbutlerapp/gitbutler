//! The commit-graph native position store: runs minted from the plan data alone, mirroring the
//! segment writer's mechanics (allocation order, connection order, adjusted endpoints) without
//! touching a segment. It authors the stored positions through the shared
//! [`Ir`](super::ref_positions); debug-only oracles compare it against the segment graph at each
//! frontier and re-derive the layout through the demoted segment walk.
//!
//! Parity is gated by `BUT_POSITIONS_PARITY`: unset or `trace` logs mismatches, `assert` panics
//! with the diff, `off` skips the checks (the native build itself is production).

use std::collections::{BTreeMap, HashMap, HashSet};

use gix::reference::Category;

use super::chains::AdvancedOutside;
use super::materialize::{ParentDst, commit_run, parent_edge_dsts, tip_run_and_name};
use super::plan::ChainPlan;
use super::remotes::{AheadRegion, region_tips, remote_name_in_play, unique_plain_local};
use super::{IdMap, IdSet};
use crate::ref_arrangement::{GroupPlacement, RefArrangement};
use crate::segment_graph::SegmentGraph;
use crate::{Commit, CommitGraph, Direction};

/// One native run, mirroring a segment's positional payload: its naming ref, its commits
/// (top → bottom), and its ordered outgoing edges.
pub(super) struct NativeRun {
    pub name: Option<gix::refs::FullName>,
    pub commits: Vec<Commit>,
    pub edges: Vec<NativeEdge>,
}

/// An outgoing edge with the writer's adjusted endpoints: the source's last commit and the
/// target's first, `None` on the empty side.
#[derive(PartialEq, Eq)]
pub(super) struct NativeEdge {
    pub target: usize,
    pub src_id: Option<gix::ObjectId>,
    pub dst_id: Option<gix::ObjectId>,
}

/// The run arena. Indices are allocation-ordered like segment ids, so run `i` is segment `i`
/// for as long as no pass removes segments.
#[derive(Default)]
pub(super) struct NativeStore {
    pub runs: Vec<NativeRun>,
}

impl NativeStore {
    fn add_run(&mut self, name: Option<gix::refs::FullName>, commits: Vec<Commit>) -> usize {
        self.runs.push(NativeRun {
            name,
            commits,
            edges: Vec::new(),
        });
        self.runs.len() - 1
    }

    /// Append `src` → `dst` with adjusted endpoints, like the segment writer's `connect`.
    fn connect(&mut self, src: usize, dst: usize) {
        let edge = self.adjusted_edge(src, dst);
        self.runs[src].edges.push(edge);
    }

    fn adjusted_edge(&self, src: usize, dst: usize) -> NativeEdge {
        NativeEdge {
            target: dst,
            src_id: self.runs[src].commits.last().map(|c| c.id),
            dst_id: self.runs[dst].commits.first().map(|c| c.id),
        }
    }

    /// Insert `src` → `dst` at `slot` among `src`'s edges (clamped), like the writer's
    /// `insert_edge_at`.
    fn insert_connect_at(&mut self, src: usize, slot: usize, dst: usize) {
        let edge = self.adjusted_edge(src, dst);
        let edges = &mut self.runs[src].edges;
        let slot = slot.min(edges.len());
        edges.insert(slot, edge);
    }

    /// Re-point `src`'s edges at `old_target` to `new_target`, clearing their destination
    /// endpoint like the writer's `retarget_edges` (surgery invalidates it).
    fn retarget_edges(&mut self, src: usize, old_target: usize, new_target: usize) -> usize {
        let mut retargeted = 0;
        for edge in &mut self.runs[src].edges {
            if edge.target == old_target {
                edge.target = new_target;
                edge.dst_id = None;
                retargeted += 1;
            }
        }
        retargeted
    }

    fn run_by_commit(&self, commit: gix::ObjectId) -> Option<usize> {
        (0..self.runs.len()).find(|&run| self.runs[run].commits.iter().any(|c| c.id == commit))
    }

    /// Like [`Self::run_by_commit`] but ignoring `exclude`d runs — the pre-chain coverage view,
    /// mirroring the writer's `segment_by_commit_excluding`.
    fn run_by_commit_excluding(
        &self,
        commit: gix::ObjectId,
        exclude: &HashSet<usize>,
    ) -> Option<usize> {
        (0..self.runs.len()).find(|&run| {
            !exclude.contains(&run) && self.runs[run].commits.iter().any(|c| c.id == commit)
        })
    }

    /// The first run (in id order) named `name`, like the writer's `segment_by_ref`.
    fn run_by_ref(&self, name: &gix::refs::FullName) -> Option<usize> {
        (0..self.runs.len()).find(|&run| self.runs[run].name.as_ref() == Some(name))
    }

    /// The first run (in id order) whose FIRST commit is `commit` — the writer's region-reuse
    /// and existing-remote-tip probe.
    fn run_with_first_commit(&self, commit: gix::ObjectId) -> Option<usize> {
        (0..self.runs.len()).find(|&run| {
            self.runs[run]
                .commits
                .first()
                .is_some_and(|c| c.id == commit)
        })
    }

    fn is_remote_run(&self, run: usize) -> bool {
        self.runs[run]
            .name
            .as_ref()
            .is_some_and(|name| name.as_ref().category() == Some(Category::RemoteBranch))
    }
}

/// The native twin of `mint_tip_segments` + `mint_float_placeholders` + `connect_parents`:
/// tip runs in facts order, float placeholders, then the parent edges — every decision read
/// from the shared plan data.
#[allow(clippy::too_many_arguments)]
fn native_mint(
    cg: &CommitGraph,
    tips: &[gix::ObjectId],
    in_set: &IdSet,
    boundaries: &IdSet,
    owner_of: &IdMap<gix::ObjectId>,
    plan: &ChainPlan,
    workspace_commit: gix::ObjectId,
) -> (NativeStore, IdMap<usize>) {
    let mut store = NativeStore::default();
    let mut run_of_tip: IdMap<usize> = IdMap::default();
    for &tip in tips {
        let (commits, named) = tip_run_and_name(cg, tip, in_set, boundaries, plan);
        let run = store.add_run(named.map(|(name, _)| name), commits);
        run_of_tip.insert(tip, run);
    }
    let mut placeholder_of: IdMap<usize> = IdMap::default();
    for float in &plan.floats {
        let run = store.add_run(Some(float.name.clone()), Vec::new());
        placeholder_of.insert(float.tip, run);
    }
    let float_tips: IdSet = plan.floats.iter().map(|fl| fl.tip).collect();
    for &tip in tips {
        let src = run_of_tip[&tip];
        let bottom = store.runs[src].commits.last().map(|c| c.id).unwrap_or(tip);
        for dst in parent_edge_dsts(cg, tip, bottom, workspace_commit, owner_of, &float_tips) {
            let dst = match dst {
                ParentDst::Tip(owner) => run_of_tip[&owner],
                ParentDst::Placeholder(float_tip) => placeholder_of[&float_tip],
            };
            store.connect(src, dst);
        }
    }
    for float in &plan.floats {
        let (Some(&ph), Some(&tip_run)) =
            (placeholder_of.get(&float.tip), run_of_tip.get(&float.tip))
        else {
            continue;
        };
        store.connect(ph, tip_run);
    }
    (store, run_of_tip)
}

/// The native twin of `build_chain_structure`: the empty-workspace splice, the decided
/// advanced-outside branches, then the table's chains — all consumed as data, the splices
/// mirroring the segment writer's mechanics.
fn native_chain_structure(
    store: &mut NativeStore,
    run_of_tip: &IdMap<usize>,
    workspace_commit: gix::ObjectId,
    ws_empty_ref: Option<&gix::refs::FullName>,
    advanced_outside: &[AdvancedOutside],
    arrangement: &RefArrangement,
) {
    let mut ws_empty_run = None;
    if let Some(ws_ref) = ws_empty_ref
        && let Some(&stack) = run_of_tip.get(&workspace_commit)
    {
        let run = store.add_run(Some(ws_ref.clone()), Vec::new());
        store.connect(run, stack);
        ws_empty_run = Some(run);
    }
    for decision in advanced_outside {
        let Some(owner) = store.run_by_commit(decision.rejoin) else {
            continue;
        };
        let run = store.add_run(decision.name.clone(), decision.commits.clone());
        store.connect(run, owner);
    }
    let ws_run = ws_empty_run.or_else(|| run_of_tip.get(&workspace_commit).copied());
    native_insert_empty_branches(store, ws_run, arrangement);
}

/// The native twin of `insert_empty_branches`: demotions clear names, group namers take their
/// anchors, empties splice above in metadata order.
fn native_insert_empty_branches(
    store: &mut NativeStore,
    ws_run: Option<usize>,
    arrangement: &RefArrangement,
) {
    for &tip in &arrangement.demoted {
        let Some(anchor) = store.run_by_commit(tip) else {
            continue;
        };
        store.runs[anchor].name = None;
    }
    for (li, chain) in arrangement.chains.iter().enumerate() {
        let mut from_run = ws_run;
        for &(commit, gi) in &chain.anchors {
            let group = &arrangement.at_commit[&commit][gi];
            if group.placement == GroupPlacement::Skipped {
                continue;
            }
            let Some(anchor) = store.run_by_commit(commit) else {
                continue;
            };
            if let Some(namer) = &group.namer {
                store.runs[anchor].name = Some(namer.name.clone());
            }
            if !group.empties.is_empty() {
                if group.placement == GroupPlacement::Passive {
                    from_run = Some(anchor);
                    continue;
                }
                let dependent = group.placement == GroupPlacement::Dependent;
                native_insert_empty_chain_above(
                    store,
                    from_run,
                    anchor,
                    &group.empties,
                    dependent,
                    dependent,
                    (from_run == ws_run).then_some(li),
                );
            }
            from_run = Some(anchor);
        }
    }
}

/// The native twin of `insert_empty_chain_above` — same redirect/splice/fresh-edge mechanics
/// over runs.
fn native_insert_empty_chain_above(
    store: &mut NativeStore,
    from_run: Option<usize>,
    anchor: usize,
    empties: &[gix::refs::FullName],
    dependent: bool,
    redirect_all: bool,
    fresh_connection_slot: Option<usize>,
) {
    let run_ids: Vec<usize> = empties
        .iter()
        .map(|b| store.add_run(Some(b.clone()), Vec::new()))
        .collect();
    let Some(&top) = run_ids.first() else {
        return;
    };
    if let Some(from_run) = from_run {
        let mut redirected = false;
        let redirect_sources: Vec<usize> = if redirect_all {
            (0..store.runs.len())
                .filter(|&run| !run_ids.contains(&run) && !store.is_remote_run(run))
                .collect()
        } else {
            vec![from_run]
        };
        for source in redirect_sources {
            redirected |= store.retarget_edges(source, anchor, top) > 0;
        }
        if !redirected {
            let find_parent = |require_commits: bool| {
                (0..store.runs.len()).find(|&run| {
                    run != from_run
                        && !store.is_remote_run(run)
                        && (!require_commits || !store.runs[run].commits.is_empty())
                        && store.runs[run].edges.iter().any(|e| e.target == anchor)
                })
            };
            let chain_parent = dependent
                .then(|| find_parent(true).or_else(|| find_parent(false)))
                .flatten();
            match chain_parent {
                Some(parent) => {
                    store.retarget_edges(parent, anchor, top);
                }
                None => match fresh_connection_slot {
                    Some(slot) => store.insert_connect_at(from_run, slot, top),
                    None => store.connect(from_run, top),
                },
            }
        }
    }
    for i in 0..run_ids.len() {
        let next = run_ids.get(i + 1).copied().unwrap_or(anchor);
        store.connect(run_ids[i], next);
    }
}

/// The native twin of `add_remote_segments`: locals keyed on the plan's pre-chain names in
/// link order, behind/at remotes as empty roots (skipped when the plan's rename already named
/// the owner), ahead remotes segmented region by region. Sibling/tracking links are invisible
/// to positions and add nothing here.
#[allow(clippy::too_many_arguments)]
fn native_add_remote_segments(
    cg: &CommitGraph,
    store: &mut NativeStore,
    run_of_tip: &IdMap<usize>,
    in_set: &IdSet,
    owner_of: &IdMap<gix::ObjectId>,
    symbolic_remotes: &[String],
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    region_pinned: &IdSet,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    plan: &ChainPlan,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(usize, gix::ObjectId)>,
) {
    let mut locals: Vec<(usize, gix::refs::FullName)> = run_of_tip
        .iter()
        .filter_map(|(&tip, &run)| {
            let name = plan.base_name_of.get(&tip)?;
            let rt = remote_tracking.get(name).cloned()?;
            Some((store.run_by_ref(name).unwrap_or(run), rt))
        })
        .collect();
    locals.sort_by_key(|&(run, ..)| run);
    for (_link_run, remote_ref) in locals {
        let Some(remote_tip) = cg.commit_by_ref(remote_ref.as_ref()) else {
            continue;
        };
        if in_set.contains(&remote_tip) {
            let owner = owner_of.get(&remote_tip).copied().unwrap_or(remote_tip);
            let owner_run = run_of_tip[&owner];
            let named_by_this = plan
                .renames
                .get(&owner)
                .is_some_and(|(name, _)| name == &remote_ref);
            if !named_by_this {
                let remote_run = store.add_run(Some(remote_ref.clone()), Vec::new());
                store.connect(remote_run, owner_run);
            }
            continue;
        }
        let in_play = remote_name_in_play(&remote_ref, symbolic_remotes);
        let is_metadata_stack_branch = stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| *b == remote_ref);
        if !in_play || is_metadata_stack_branch {
            continue;
        }
        native_segment_ahead_region(
            cg,
            store,
            Some(&remote_ref),
            remote_tip,
            in_set,
            run_of_tip,
            owner_of,
            region_pinned,
            claimed_remote_names,
            pending_edges,
        );
    }
}

/// The native twin of `segment_ahead_region`: the shared [`AheadRegion`] shape, the interior
/// cut/stop scan mirrored on the store, then runs and edges in the writer's order.
#[allow(clippy::too_many_arguments)]
fn native_segment_ahead_region(
    cg: &CommitGraph,
    store: &mut NativeStore,
    remote_ref: Option<&gix::refs::FullName>,
    remote_tip: gix::ObjectId,
    in_set: &IdSet,
    run_of_tip: &IdMap<usize>,
    owner_of: &IdMap<gix::ObjectId>,
    pinned_commits: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(usize, gix::ObjectId)>,
) {
    let region = AheadRegion::compute(cg, remote_tip, in_set);
    let ahead_set = &region.set;
    let is_boundary =
        |c: gix::ObjectId| region.is_shape_boundary(cg, remote_tip, pinned_commits, c);

    let root_is_remote =
        remote_ref.is_some_and(|r| r.as_ref().category() == Some(Category::RemoteBranch));
    let mut interior_cuts: IdMap<gix::refs::FullName> = IdMap::default();
    let mut stop: Option<gix::ObjectId> = None;
    if root_is_remote {
        let existing_remote_tip = |c: gix::ObjectId| {
            (0..store.runs.len()).any(|run| {
                store.runs[run].commits.first().is_some_and(|f| f.id == c)
                    && store.is_remote_run(run)
            })
        };
        let mut id = cg
            .first_parent(remote_tip)
            .filter(|p| ahead_set.contains(p));
        while let Some(c) = id {
            if is_boundary(c) {
                break;
            }
            if cg
                .refs_at(c)
                .iter()
                .any(|r| claimed_remote_names.contains(r))
                || existing_remote_tip(c)
            {
                stop = Some(c);
                break;
            }
            if let Some(r) = cg.refs_at(c).into_iter().find(|r| {
                r.as_ref().category() == Some(Category::RemoteBranch)
                    && !claimed_remote_names.contains(r)
                    && store.run_by_ref(r).is_none()
            }) {
                interior_cuts.insert(c, r);
            }
            id = cg.first_parent(c).filter(|p| ahead_set.contains(p));
        }
    }
    let is_boundary =
        |c: gix::ObjectId| is_boundary(c) || interior_cuts.contains_key(&c) || stop == Some(c);

    let tips = region_tips(cg, &region, remote_tip, &is_boundary);
    let mut ahead_owner: IdMap<gix::ObjectId> = IdMap::default();
    let mut ahead_run: IdMap<usize> = IdMap::default();
    let mut reused: IdSet = IdSet::default();
    for &tip in &tips {
        if stop == Some(tip) {
            continue;
        }
        let commits = commit_run(cg, tip, ahead_set, &is_boundary);
        for c in &commits {
            ahead_owner.insert(c.id, tip);
        }
        let is_root = tip == remote_tip;
        if !is_root && let Some(existing) = store.run_with_first_commit(tip) {
            ahead_run.insert(tip, existing);
            reused.insert(tip);
            continue;
        }
        let name = if is_root {
            remote_ref
                .cloned()
                .or_else(|| unique_plain_local(cg, remote_tip))
        } else {
            interior_cuts
                .get(&tip)
                .cloned()
                .or_else(|| unique_plain_local(cg, tip))
        };
        let run = store.add_run(name, commits);
        ahead_run.insert(tip, run);
    }

    for &tip in &tips {
        if reused.contains(&tip) || stop == Some(tip) {
            continue;
        }
        let src = ahead_run[&tip];
        let bottom = store.runs[src].commits.last().map(|c| c.id).unwrap_or(tip);
        for parent in cg.all_parent_ids(bottom) {
            let dst = if ahead_set.contains(&parent) {
                ahead_owner
                    .get(&parent)
                    .and_then(|o| ahead_run.get(o))
                    .copied()
            } else {
                owner_of
                    .get(&parent)
                    .and_then(|o| run_of_tip.get(o))
                    .copied()
            };
            if let Some(dst) = dst {
                store.connect(src, dst);
            } else if ahead_set.contains(&parent) {
                pending_edges.push((src, parent));
            }
        }
    }
}

/// The native twin of `add_untracked_remote_segments`: unclaimed remote refs whose local
/// counterpart shares the tip become empty roots into the owning run (in-set case only).
fn native_add_untracked_remote_segments(
    cg: &CommitGraph,
    store: &mut NativeStore,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    run_of_tip: &IdMap<usize>,
    in_set: &IdSet,
    owner_of: &IdMap<gix::ObjectId>,
) {
    let mut remote_refs: std::collections::BTreeSet<gix::refs::FullName> =
        std::collections::BTreeSet::new();
    for c in cg.commit_ids() {
        for r in cg.refs_at(c) {
            if r.as_ref().category() == Some(Category::RemoteBranch) {
                remote_refs.insert(r);
            }
        }
    }
    for r in remote_refs {
        if store.run_by_ref(&r).is_some() {
            continue;
        }
        let Some(tip) = cg.commit_by_ref(r.as_ref()) else {
            continue;
        };
        let has_local_counterpart = cg
            .refs_at(tip)
            .iter()
            .any(|l| remote_tracking.get(l) == Some(&r));
        if !has_local_counterpart {
            continue;
        }
        if in_set.contains(&tip)
            && let Some(&owner) = owner_of.get(&tip)
            && let Some(&owner_run) = run_of_tip.get(&owner)
        {
            let remote_run = store.add_run(Some(r.clone()), Vec::new());
            store.connect(remote_run, owner_run);
        }
    }
}

/// The native twin of `surface_target_remote`'s POSITIONAL half: the in-set case only adds a
/// sibling link (invisible here); an outside target grows its region, and a local tracking
/// branch on the region's tip takes the name — the remote becomes an empty run above it.
#[allow(clippy::too_many_arguments)]
fn native_surface_target_remote(
    cg: &CommitGraph,
    store: &mut NativeStore,
    target_ref: Option<&gix::refs::FullName>,
    in_set: &IdSet,
    run_of_tip: &IdMap<usize>,
    owner_of: &IdMap<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    region_pinned: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(usize, gix::ObjectId)>,
) {
    let Some(tr) = target_ref else { return };
    if tr.as_ref().category() != Some(Category::RemoteBranch) {
        return;
    }
    let Some(tip) = cg.commit_by_ref(tr.as_ref()) else {
        return;
    };
    if in_set.contains(&tip) || store.run_by_ref(tr).is_some() {
        return;
    }
    native_segment_ahead_region(
        cg,
        store,
        Some(tr),
        tip,
        in_set,
        run_of_tip,
        owner_of,
        region_pinned,
        claimed_remote_names,
        pending_edges,
    );
    let local_on_tip = remote_tracking
        .iter()
        .find(|(local, r)| *r == tr && cg.commit_by_ref(local.as_ref()) == Some(tip))
        .map(|(local, _)| local.clone());
    if let Some(local) = local_on_tip
        && let Some(owner) = store.run_by_commit(tip)
        && store.runs[owner].name.as_ref() == Some(tr)
        && store.runs[owner]
            .commits
            .first()
            .is_some_and(|c| c.id == tip)
    {
        store.runs[owner].name = Some(local);
        let remote_run = store.add_run(Some(tr.clone()), Vec::new());
        store.connect(remote_run, owner);
    }
}

/// The native twin of `cover_explicit_tips`: an uncovered explicit tip grows its own region;
/// a covered one whose ref names no run gets an empty tip-named run above its owner.
#[allow(clippy::too_many_arguments)]
fn native_cover_explicit_tips(
    cg: &CommitGraph,
    store: &mut NativeStore,
    chain_created: &HashSet<usize>,
    in_set: &IdSet,
    run_of_tip: &IdMap<usize>,
    owner_of: &IdMap<gix::ObjectId>,
    region_pinned: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(usize, gix::ObjectId)>,
) {
    for t in cg.traversal_tips.iter().filter(|_| cg.explicit_tips) {
        if cg.row(t.id).is_none() {
            continue;
        }
        match store.run_by_commit_excluding(t.id, chain_created) {
            None => native_segment_ahead_region(
                cg,
                store,
                t.ref_name.as_ref(),
                t.id,
                in_set,
                run_of_tip,
                owner_of,
                region_pinned,
                claimed_remote_names,
                pending_edges,
            ),
            Some(owner_run) => {
                let Some(ref_name) = t.ref_name.clone() else {
                    continue;
                };
                if but_core::is_workspace_ref_name(ref_name.as_ref()) {
                    continue;
                }
                if store.run_by_ref(&ref_name).is_some()
                    || store.runs[owner_run].name.as_ref() == Some(&ref_name)
                {
                    continue;
                }
                // The writer asserts the plan named every anonymous tip-started segment;
                // mirror its skip.
                if store.runs[owner_run]
                    .commits
                    .first()
                    .is_some_and(|c| c.id == t.id)
                    && store.runs[owner_run].name.is_none()
                {
                    continue;
                }
                let empty = store.add_run(Some(ref_name), Vec::new());
                store.connect(empty, owner_run);
            }
        }
    }
}

/// The native twin of `add_co_located_remote_empties`: every further remote ref on a remote
/// run's first commit becomes an empty run pointing at it.
fn native_add_co_located_remote_empties(store: &mut NativeStore) {
    let existing = store.runs.len();
    for run in 0..existing {
        if !store.is_remote_run(run) {
            continue;
        }
        let Some(first) = store.runs[run].commits.first().cloned() else {
            continue;
        };
        for ri in &first.refs {
            if ri.ref_name.as_ref().category() != Some(Category::RemoteBranch)
                || store.run_by_ref(&ri.ref_name).is_some()
            {
                continue;
            }
            let empty = store.add_run(Some(ri.ref_name.clone()), Vec::new());
            store.connect(empty, run);
        }
    }
}

/// The native twin of `wire_pending_edges`: connect each stopped run to the run owning its
/// parent commit — every creator has run, so the owner exists.
fn native_wire_pending_edges(store: &mut NativeStore, pending_edges: Vec<(usize, gix::ObjectId)>) {
    for (src, parent) in pending_edges {
        let Some(dst) = store.run_by_commit(parent) else {
            continue;
        };
        store.connect(src, dst);
    }
}

/// The native twin of `float_remote_named_checkout`: a ref-less checkout at a remote-named
/// run's tip moves the name to a fresh empty run above it; edges follow.
fn native_float_remote_named_checkout(
    store: &mut NativeStore,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    workspace_commit: gix::ObjectId,
) {
    if entrypoint_ref.is_some() || entrypoint == workspace_commit {
        return;
    }
    let Some(ep_run) = store.run_by_commit(entrypoint) else {
        return;
    };
    if !store.is_remote_run(ep_run)
        || store.runs[ep_run].commits.first().map(|c| c.id) != Some(entrypoint)
    {
        return;
    }
    let name = store.runs[ep_run].name.take();
    let floated = store.add_run(name, Vec::new());
    for run in 0..store.runs.len() {
        if run == floated {
            continue;
        }
        store.retarget_edges(run, ep_run, floated);
    }
    store.connect(floated, ep_run);
}

/// The native twin of `name_entrypoint_segment`'s POSITIONAL half (reached when the checked-out
/// ref names no run yet): an anonymous run starting at the entrypoint takes the name; a run
/// named by ANOTHER ref gets an empty entrypoint-named run spliced in above.
fn native_name_entrypoint_segment(
    store: &mut NativeStore,
    entrypoint: gix::ObjectId,
    ep_ref: &gix::refs::FullName,
) {
    let Some((run, pos)) = (0..store.runs.len()).find_map(|run| {
        store.runs[run]
            .commits
            .iter()
            .position(|c| c.id == entrypoint)
            .map(|p| (run, p))
    }) else {
        return;
    };
    if pos != 0 {
        return;
    }
    match store.runs[run].name.clone() {
        None => store.runs[run].name = Some(ep_ref.clone()),
        Some(existing) if existing != *ep_ref => {
            let empty = store.add_run(Some(ep_ref.clone()), Vec::new());
            for other in 0..store.runs.len() {
                if other == empty {
                    continue;
                }
                store.retarget_edges(other, run, empty);
            }
            store.connect(empty, run);
        }
        Some(_) => {}
    }
}

/// The native twin of `strip_segment_named_refs`: a ref that names a run (or is a named run's
/// remote-tracking counterpart) leaves every commit's own ref list, as does every
/// remote-category ref.
fn native_strip_segment_named_refs(
    store: &mut NativeStore,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let mut names: HashSet<gix::refs::FullName> =
        store.runs.iter().filter_map(|r| r.name.clone()).collect();
    names.extend(store.runs.iter().filter_map(|r| {
        r.name
            .as_ref()
            .and_then(|n| remote_tracking.get(n))
            .cloned()
    }));
    for run in &mut store.runs {
        for commit in &mut run.commits {
            commit.refs.retain(|ri| {
                !names.contains(&ri.ref_name)
                    && ri.ref_name.as_ref().category() != Some(Category::RemoteBranch)
            });
        }
    }
}

/// The native twin of `annotate_worktrees`' commit-ref half — run names carry no worktree,
/// so only the refs riding commits are annotated.
fn native_annotate_worktrees(
    store: &mut NativeStore,
    worktree_by_branch: &BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
) {
    for run in &mut store.runs {
        for commit in &mut run.commits {
            for ri in &mut commit.refs {
                if ri.worktree.is_none()
                    && let Some(wt) = worktree_by_branch.get(&ri.ref_name).and_then(|w| w.first())
                {
                    ri.worktree = Some(wt.clone());
                }
            }
        }
    }
}

/// Everything the native build reads: the commit graph plus the decisions the segment writer
/// consumes (facts fields, the plan, the arrangement, and the chain-structure decisions).
pub(super) struct NativeBuildInputs<'a> {
    pub cg: &'a CommitGraph,
    pub tips: &'a [gix::ObjectId],
    pub in_set: &'a IdSet,
    pub boundaries: &'a IdSet,
    pub owner_of: &'a IdMap<gix::ObjectId>,
    pub plan: &'a ChainPlan,
    pub arrangement: &'a RefArrangement,
    pub workspace_commit: gix::ObjectId,
    pub managed: bool,
    pub ws_empty_ref: Option<&'a gix::refs::FullName>,
    pub advanced_outside: &'a [AdvancedOutside],
    pub remote_tracking: &'a HashMap<gix::refs::FullName, gix::refs::FullName>,
    pub symbolic_remotes: &'a [String],
    pub stack_branches: Option<&'a [Vec<gix::refs::FullName>]>,
    pub region_pinned: &'a IdSet,
    pub claimed_remote_names: &'a HashSet<gix::refs::FullName>,
    pub entrypoint: gix::ObjectId,
    pub entrypoint_ref: Option<&'a gix::refs::FullName>,
    pub target_ref: Option<&'a gix::refs::FullName>,
    pub extra_target: Option<gix::ObjectId>,
    pub worktree_by_branch: &'a BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
}

/// THE NATIVE BUILD: every materializer pass (mint + connect + chain structure + the remote
/// passes + the coverage regions + the sweeps) replayed on the store from the decisions alone.
/// The store authors the stored positions; the segment graph is the debug oracle.
pub(super) fn build_through_sweeps(inputs: NativeBuildInputs<'_>) -> NativeStore {
    let (mut store, run_of_tip) = native_mint(
        inputs.cg,
        inputs.tips,
        inputs.in_set,
        inputs.boundaries,
        inputs.owner_of,
        inputs.plan,
        inputs.workspace_commit,
    );
    let before_chains = store.runs.len();
    if inputs.managed {
        native_chain_structure(
            &mut store,
            &run_of_tip,
            inputs.workspace_commit,
            inputs.ws_empty_ref,
            inputs.advanced_outside,
            inputs.arrangement,
        );
    }
    // The coverage gates evaluate the PRE-CHAIN view, like the writer's `chain_created`.
    let chain_created: HashSet<usize> = (before_chains..store.runs.len()).collect();
    let mut pending_edges: Vec<(usize, gix::ObjectId)> = Vec::new();
    native_add_remote_segments(
        inputs.cg,
        &mut store,
        &run_of_tip,
        inputs.in_set,
        inputs.owner_of,
        inputs.symbolic_remotes,
        inputs.stack_branches,
        inputs.region_pinned,
        inputs.remote_tracking,
        inputs.plan,
        inputs.claimed_remote_names,
        &mut pending_edges,
    );
    native_add_untracked_remote_segments(
        inputs.cg,
        &mut store,
        inputs.remote_tracking,
        &run_of_tip,
        inputs.in_set,
        inputs.owner_of,
    );
    native_surface_target_remote(
        inputs.cg,
        &mut store,
        inputs.target_ref,
        inputs.in_set,
        &run_of_tip,
        inputs.owner_of,
        inputs.remote_tracking,
        inputs.region_pinned,
        inputs.claimed_remote_names,
        &mut pending_edges,
    );
    // The extra-target twin: a stored target position uncovered by any pre-chain run grows
    // its own (nameless) region.
    if let Some(extra) = inputs.extra_target
        && inputs.cg.row(extra).is_some()
        && store
            .run_by_commit_excluding(extra, &chain_created)
            .is_none()
    {
        native_segment_ahead_region(
            inputs.cg,
            &mut store,
            None,
            extra,
            inputs.in_set,
            &run_of_tip,
            inputs.owner_of,
            inputs.region_pinned,
            inputs.claimed_remote_names,
            &mut pending_edges,
        );
    }
    // The outside-entrypoint twin: an adhoc checkout outside the workspace grows its region.
    if !inputs.in_set.contains(&inputs.entrypoint)
        && inputs.cg.row(inputs.entrypoint).is_some()
        && store
            .run_by_commit_excluding(inputs.entrypoint, &chain_created)
            .is_none()
    {
        native_segment_ahead_region(
            inputs.cg,
            &mut store,
            inputs.entrypoint_ref,
            inputs.entrypoint,
            inputs.in_set,
            &run_of_tip,
            inputs.owner_of,
            inputs.region_pinned,
            inputs.claimed_remote_names,
            &mut pending_edges,
        );
    }
    native_cover_explicit_tips(
        inputs.cg,
        &mut store,
        &chain_created,
        inputs.in_set,
        &run_of_tip,
        inputs.owner_of,
        inputs.region_pinned,
        inputs.claimed_remote_names,
        &mut pending_edges,
    );
    native_add_co_located_remote_empties(&mut store);
    native_wire_pending_edges(&mut store, pending_edges);
    native_float_remote_named_checkout(
        &mut store,
        inputs.entrypoint,
        inputs.entrypoint_ref,
        inputs.workspace_commit,
    );
    // `resolve_entrypoint_segment` only mutates through `name_entrypoint_segment`, and only
    // when the checked-out ref names no run yet — its other arms just pick a segment.
    if let Some(ep_ref) = inputs.entrypoint_ref
        && store.run_by_ref(ep_ref).is_none()
    {
        native_name_entrypoint_segment(&mut store, inputs.entrypoint, ep_ref);
    }
    native_strip_segment_named_refs(&mut store, inputs.remote_tracking);
    native_annotate_worktrees(&mut store, inputs.worktree_by_branch);
    store
}

/// Replay the ad-hoc same-tip rebuild on the store — the NON-managed path's post-materializer
/// pass. The decision (which persisted-order refs share the bottom tip) is repo+meta work the
/// writer records as `orders`; only the structural rewrite twins here.
pub(super) fn replay_ad_hoc(
    store: &mut NativeStore,
    cg: &CommitGraph,
    orders: &[Vec<gix::refs::FullName>],
) {
    for matching_refs in orders {
        native_ad_hoc_stack_rebuild(cg, store, matching_refs);
    }
}

/// The structural oracle: compare the store against the segment graph at `stage`, honoring
/// the parity mode.
pub(super) fn debug_check(store: &NativeStore, sg: &SegmentGraph, stage: &str) {
    let mode = std::env::var("BUT_POSITIONS_PARITY").unwrap_or_default();
    if mode == "off" {
        return;
    }
    check(store, sg, stage, &mode);
}

/// THE DEMOTED WALK: re-derive the layout from the finished segment graph and compare it
/// against the natively authored `actual`, honoring the parity mode.
pub(super) fn debug_walk_positions_parity(
    graph: &crate::Graph,
    repo: &gix::Repository,
    actual: &crate::ref_arrangement::RefPositions,
) {
    let mode = std::env::var("BUT_POSITIONS_PARITY").unwrap_or_default();
    if mode == "off" {
        return;
    }
    match super::ref_positions::ref_positions(graph, repo) {
        Ok(expected) => {
            if *actual == expected {
                return;
            }
            if mode == "assert" {
                panic!("positions parity (walk): native {actual:#?} vs walk {expected:#?}");
            }
            tracing::warn!(
                "positions parity (walk): the segment walk diverged from the native derivation"
            );
        }
        Err(err) => {
            if mode == "assert" {
                panic!("positions parity (walk): the walk derivation failed: {err}");
            }
            tracing::warn!(?err, "positions parity (walk): the walk derivation failed");
        }
    }
}

/// The native twin of `rebuild_same_tip_segment_chain_by_branch_order` (plus the writer's bottom
/// pick): the bottom ordered ref owns the shared tip, branches above become explicit empty runs
/// chained on top, and outside edges re-route through the (empty) top.
///
/// The writer resolves the bottom tip by peeling the repo ref; here `cg.commit_by_ref` stands in —
/// the traversal read the same refs, and the oracle holds the equivalence.
fn native_ad_hoc_stack_rebuild(
    cg: &CommitGraph,
    store: &mut NativeStore,
    matching_refs: &[gix::refs::FullName],
) {
    let Some((bottom_ref, empty_refs)) = matching_refs.split_last() else {
        return;
    };
    if empty_refs.is_empty() {
        return;
    }
    let Some(bottom_run) = store.run_by_ref(bottom_ref).or_else(|| {
        cg.commit_by_ref(bottom_ref.as_ref())
            .and_then(|id| store.run_with_first_commit(id))
    }) else {
        return;
    };
    store.runs[bottom_run].name = Some(bottom_ref.clone());
    if let Some(commit) = store.runs[bottom_run].commits.first_mut() {
        commit
            .refs
            .retain(|ri| !matching_refs.iter().any(|name| name == &ri.ref_name));
    }

    let mut empty_run_by_ref = BTreeMap::new();
    for (run, data) in store.runs.iter().enumerate() {
        if data.commits.is_empty()
            && let Some(name) = data.name.as_ref()
            && empty_refs.contains(name)
        {
            empty_run_by_ref.insert(name.clone(), run);
        }
    }
    let mut ordered_runs: Vec<usize> = empty_refs
        .iter()
        .map(|name| {
            empty_run_by_ref
                .get(name)
                .copied()
                .unwrap_or_else(|| store.add_run(Some(name.clone()), Vec::new()))
        })
        .collect();
    ordered_runs.push(bottom_run);
    let involved: HashSet<usize> = ordered_runs.iter().copied().collect();
    let top_run = ordered_runs[0];

    // Outside incoming edges re-point at the top in place, like the writer's retarget loop.
    let retargets: Vec<(usize, usize)> = store
        .runs
        .iter()
        .enumerate()
        .filter(|(src, _)| !involved.contains(src))
        .flat_map(|(src, data)| {
            data.edges
                .iter()
                .filter(|edge| involved.contains(&edge.target))
                .map(move |edge| (src, edge.target))
        })
        .collect();
    for (src, old_target) in retargets {
        store.retarget_edges(src, old_target, top_run);
    }
    // Involved runs drop their own outgoing edges — except the bottom's leading outside — then
    // re-chain top-to-bottom. Each source ends with a single edge, so no first-parent re-sort.
    for &run in &involved {
        if run == bottom_run {
            store.runs[run]
                .edges
                .retain(|edge| !involved.contains(&edge.target));
        } else {
            store.runs[run].edges.clear();
        }
    }
    for pair in ordered_runs.windows(2) {
        let [above, below] = pair else {
            continue;
        };
        store.connect(*above, *below);
    }
}

/// Compare the native store against the segment graph at the covered point — names, commits,
/// and ordered edges with endpoints must be identical, run `i` against segment `i` in id order.
fn check(store: &NativeStore, sg: &SegmentGraph, stage: &str, mode: &str) {
    let mut mismatches = Vec::new();
    let sids: Vec<crate::SegmentIndex> = sg.segment_ids().collect();
    if sids.len() != store.runs.len() {
        mismatches.push(format!(
            "run count: native {} vs segments {}",
            store.runs.len(),
            sids.len()
        ));
    }
    let ordinal_of_sid: std::collections::HashMap<crate::SegmentIndex, usize> = sids
        .iter()
        .enumerate()
        .map(|(ordinal, &sid)| (sid, ordinal))
        .collect();
    for (run, &sid) in store.runs.iter().zip(&sids) {
        let segment = &sg[sid];
        if run.name.as_ref().map(|n| n.as_ref()) != segment.ref_name() {
            mismatches.push(format!(
                "segment {sid}: native name {:?} vs {:?}",
                run.name,
                segment.ref_name()
            ));
        }
        if run.commits != segment.commits {
            mismatches.push(format!(
                "segment {sid}: native commits {:?} vs {:?}",
                run.commits.iter().map(|c| c.id).collect::<Vec<_>>(),
                segment.commits.iter().map(|c| c.id).collect::<Vec<_>>()
            ));
        }
        let segment_edges: Vec<NativeEdge> = sg
            .edges_directed(sid, Direction::Outgoing)
            .map(|edge| NativeEdge {
                target: ordinal_of_sid[&edge.target],
                src_id: edge.connection.src_id(),
                dst_id: edge.connection.dst_id(),
            })
            .collect();
        if run.edges != segment_edges {
            mismatches.push(format!(
                "segment {sid}: native edges {:?} vs {:?}",
                run.edges
                    .iter()
                    .map(|e| (e.target, e.src_id, e.dst_id))
                    .collect::<Vec<_>>(),
                segment_edges
                    .iter()
                    .map(|e| (e.target, e.src_id, e.dst_id))
                    .collect::<Vec<_>>()
            ));
        }
    }
    if mismatches.is_empty() {
        return;
    }
    if mode == "assert" {
        panic!("positions parity ({stage}): {mismatches:#?}");
    }
    tracing::warn!(stage, ?mismatches, "positions parity");
}
