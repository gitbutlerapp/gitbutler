//! The build authors [`Segment`]s directly: they are minted from the plan data alone
//! (allocation order, connection order), and the stored ref positions derive from them
//! ([`derive_ref_layout`](super::layout::derive_ref_layout)). The rows
//! never leave the build — position `i` is id `i`.

use std::collections::{BTreeMap, HashMap, HashSet};

use gix::reference::Category;

use super::materialize::AdvancedOutside;
use super::materialize::tip_run_and_name;
use super::plan::ChainPlan;
use super::plan::{GroupPlacement, LayoutPlan, RefChain};
use super::remote_segments::{
    add_co_located_remote_empties, add_remote_segments, add_untracked_remote_segments,
    segment_ahead_region, surface_target_remote,
};
use super::{IdMap, IdSet};
use crate::CommitGraph;

/// One authored row: a name, the commit run it owns and its ordered outgoing edges (both
/// in arena/row indices), plus the remote links enrichment reads.
#[derive(Debug, Default)]
pub(super) struct Segment {
    /// The row's (disambiguated) name, if any.
    pub name: Option<gix::refs::FullName>,
    /// The commit the name resolves to — `None` for synthetic (metadata-derived) empties,
    /// whose name has no resolved ref tip.
    pub tip: Option<gix::ObjectId>,
    /// The name of this row's remote tracking branch, if present.
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// Doubly-links remote and local tracking rows; also points an anonymous ancestor at
    /// the workspace-known named row it stands in for.
    pub sibling_segment_id: Option<usize>,
    /// The row of `remote_tracking_ref_name`, when that is set.
    pub remote_tracking_branch_segment_id: Option<usize>,
    /// The commit run this row owns, as handles into the arena.
    pub commits: Vec<usize>,
    /// Outgoing edges to other rows, in first-parent order. Edge semantics derive from
    /// the rows' commits: the source's last connects to the target's first.
    pub connections: Vec<usize>,
}

impl Segment {
    /// The name as a ref, for comparisons.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.name.as_ref().map(|n| n.as_ref())
    }
}

/// The row arena the build authors: indices are allocation-ordered and no pass removes
/// entries, so a row's position IS its id.
#[derive(Default)]
pub(super) struct SegmentData {
    pub segments: Vec<Segment>,
    /// Rows by CURRENT name — [`Self::sidx_by_ref`] answers from here in O(1) where a scan
    /// over all rows pays per lookup at scale. Every name write flows through
    /// [`Self::add_segment`], [`Self::set_name`], [`Self::clear_name`], and
    /// [`Self::take_name`]/[`Self::put_name`], which keep it exact; duplicate names order
    /// by row id, matching the scan's first-match.
    by_name: HashMap<gix::refs::FullName, std::collections::BTreeSet<usize>>,
    /// Passive refs the BUILD adds onto a commit (by arena handle) beyond what the arena
    /// carries — a float's displaced name. The position derivation merges them in.
    pub extra_refs: HashMap<usize, Vec<gix::refs::FullName>>,
    /// The entrypoint's segment — the root of the layout's reach computation, and its name marks
    /// the HEAD ordinals.
    pub entrypoint_sidx: Option<usize>,
    /// The commit the entrypoint rests on, `None` when unborn.
    pub entrypoint: Option<gix::ObjectId>,
}

impl SegmentData {
    /// The commit the entrypoint UNAMBIGUOUSLY points to, resolved through its empty
    /// chain: an empty segment resolves along its only connection; a fork or dead end
    /// yields `None`. The verdict the projection reads off the context.
    pub(super) fn resolve_entrypoint_commit(&self, cg: &CommitGraph) -> Option<gix::ObjectId> {
        let mut current = self.entrypoint_sidx?;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            if let Some(&commit) = self.segments[current].commits.first() {
                return Some(cg.id_at(commit));
            }
            let &[only] = self.segments[current].connections.as_slice() else {
                return None;
            };
            current = only;
        }
        None
    }

    pub(super) fn add_segment(
        &mut self,
        name: Option<gix::refs::FullName>,
        commits: Vec<usize>,
    ) -> usize {
        let id = self.segments.len();
        if let Some(name) = &name {
            self.by_name.entry(name.clone()).or_default().insert(id);
        }
        self.segments.push(Segment {
            name,
            commits,
            ..Default::default()
        });
        id
    }

    fn unindex_name(&mut self, sidx: usize) {
        if let Some(name) = &self.segments[sidx].name
            && let Some(rows) = self.by_name.get_mut(name)
        {
            rows.remove(&sidx);
            if rows.is_empty() {
                self.by_name.remove(name);
            }
        }
    }

    /// Detach `segment`'s name (index included), returning it. Everything else stays.
    pub(super) fn take_name(&mut self, sidx: usize) -> Option<gix::refs::FullName> {
        self.unindex_name(sidx);
        self.segments[sidx].name.take()
    }

    /// Attach `name` (if any) to `segment` (index included). Everything else stays.
    pub(super) fn put_name(&mut self, sidx: usize, name: Option<gix::refs::FullName>) {
        self.unindex_name(sidx);
        if let Some(name) = &name {
            self.by_name.entry(name.clone()).or_default().insert(sidx);
        }
        self.segments[sidx].name = name;
    }

    /// Anonymize `segment`: name and tip cleared.
    pub(super) fn clear_name(&mut self, sidx: usize) {
        self.unindex_name(sidx);
        self.segments[sidx].name = None;
        self.segments[sidx].tip = None;
    }

    /// Record the commit `segment`'s name resolves to. A no-op on nameless segments.
    pub(super) fn set_tip(&mut self, sidx: usize, id: gix::ObjectId) {
        if self.segments[sidx].name.is_some() {
            self.segments[sidx].tip = Some(id);
        }
    }

    /// Name (or rename) `segment`, with the tip its name resolves to.
    pub(super) fn set_name(
        &mut self,
        sidx: usize,
        ref_name: gix::refs::FullName,
        commit_id: Option<gix::ObjectId>,
    ) {
        self.put_name(sidx, Some(ref_name));
        self.segments[sidx].tip = commit_id;
    }

    /// Link a just-created remote-named segment to the local segment named by its tracking counterpart:
    /// the remote's sibling points at the local, and the local carries the remote's name and
    /// segment id. A no-op when no such local exists.
    pub(super) fn link_remote_to_local(
        &mut self,
        remote_sidx: usize,
        remote_ref: &gix::refs::FullName,
        remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    ) {
        let Some(local_name) = remote_tracking
            .iter()
            .find_map(|(l, r)| (r == remote_ref).then_some(l))
        else {
            return;
        };
        let Some(local_sidx) = self.sidx_by_ref(local_name) else {
            return;
        };
        self.segments[remote_sidx].sibling_segment_id = Some(local_sidx);
        self.segments[local_sidx].remote_tracking_ref_name = Some(remote_ref.clone());
        self.segments[local_sidx].remote_tracking_branch_segment_id = Some(remote_sidx);
    }

    /// Append `src` → `dst`.
    pub(super) fn connect(&mut self, src: usize, dst: usize) {
        self.segments[src].connections.push(dst);
    }

    /// Insert `src` → `dst` at `parent number` among `src`'s edges (clamped).
    fn insert_connect_at(&mut self, src: usize, parent_number: usize, dst: usize) {
        let edges = &mut self.segments[src].connections;
        let parent_number = parent_number.min(edges.len());
        edges.insert(parent_number, dst);
    }

    /// Re-point `src`'s edges at `old_target` to `new_target`.
    pub(super) fn retarget_edges(
        &mut self,
        src: usize,
        old_target: usize,
        new_target: usize,
    ) -> usize {
        let mut retargeted = 0;
        for target in &mut self.segments[src].connections {
            if *target == old_target {
                *target = new_target;
                retargeted += 1;
            }
        }
        retargeted
    }

    pub(super) fn sidx_by_commit(&self, cg: &CommitGraph, commit: gix::ObjectId) -> Option<usize> {
        self.sidx_by_commit_excluding(cg, commit, &HashSet::new())
    }

    /// Like [`Self::sidx_by_commit`] but ignoring `exclude`d segments — the pre-chain coverage view.
    fn sidx_by_commit_excluding(
        &self,
        cg: &CommitGraph,
        commit: gix::ObjectId,
        exclude: &HashSet<usize>,
    ) -> Option<usize> {
        (0..self.segments.len()).find(|&sidx| {
            !exclude.contains(&sidx)
                && self.segments[sidx]
                    .commits
                    .iter()
                    .any(|&h| cg.id_at(h) == commit)
        })
    }

    /// The first segment (in id order) named `name`.
    pub(super) fn sidx_by_ref(&self, name: &gix::refs::FullName) -> Option<usize> {
        self.by_name
            .get(name)
            .and_then(|rows| rows.first().copied())
    }

    pub(super) fn is_remote_segment(&self, sidx: usize) -> bool {
        self.segments[sidx]
            .ref_name()
            .is_some_and(|name| name.category() == Some(Category::RemoteBranch))
    }

    /// Every segment's outgoing edges as target ordinals in FINAL parent order: real parents by
    /// their index in the source commit's parent array, commit-less edges after them in edge
    /// order, ordinals compacted by push order.
    pub(super) fn parent_ordered_targets(&self, cg: &CommitGraph) -> OrderedTargets {
        let mut start = Vec::with_capacity(self.segments.len() + 1);
        let mut flat = Vec::new();
        let mut ordered: Vec<(usize, usize)> = Vec::new();
        start.push(0);
        for sidx in &self.segments {
            let mut empty_branch_count = 0usize;
            ordered.clear();
            // An edge's endpoints derive from the segments' commits: the source's LAST
            // commit connects to the target's FIRST (a stored edge record would only
            // duplicate this).
            let edge_parents = sidx
                .commits
                .last()
                .map(|&h| cg.node_at(h).parent_ids.as_slice());
            for &target in &sidx.connections {
                let dst_id = self.segments[target].commits.first().map(|&h| cg.id_at(h));
                let real_parent_index = edge_parents
                    .zip(dst_id)
                    .and_then(|(parents, dst)| parents.iter().position(|p| *p == dst));
                let ordinal = match real_parent_index {
                    Some(idx) => idx,
                    None => {
                        let o = edge_parents.map_or(0, |p| p.len()) + empty_branch_count;
                        empty_branch_count += 1;
                        o
                    }
                };
                ordered.push((ordinal, target));
            }
            ordered.sort_by_key(|(ordinal, _)| *ordinal);
            flat.extend(ordered.iter().map(|&(_, t)| t));
            start.push(flat.len());
        }
        OrderedTargets { start, flat }
    }
}

/// [`SegmentData::parent_ordered_targets`] in CSR form: one flat target list, segment-sliced.
pub(super) struct OrderedTargets {
    /// Segment `s`'s targets are `flat[start[s]..start[s + 1]]`.
    start: Vec<usize>,
    flat: Vec<usize>,
}

impl OrderedTargets {
    pub(super) fn of(&self, sidx: usize) -> &[usize] {
        &self.flat[self.start[sidx]..self.start[sidx + 1]]
    }
}

/// Everything the build reads: the commit graph plus the decided data (facts fields, the
/// plan, the layout, and the chain-structure decisions).
pub(super) struct GraphInputs<'a> {
    pub cg: &'a CommitGraph,
    pub tips: &'a [gix::ObjectId],
    pub in_set: &'a IdSet,
    pub boundaries: &'a IdSet,
    pub owner_of: &'a IdMap<gix::ObjectId>,
    pub plan: &'a ChainPlan,
    pub layout: &'a LayoutPlan,
    pub workspace_commit: gix::ObjectId,
    pub ws_empty_ref: Option<&'a gix::refs::FullName>,
    pub advanced_outside: &'a [AdvancedOutside],
    pub remote_tracking: &'a HashMap<gix::refs::FullName, gix::refs::FullName>,
    pub symbolic_remotes: &'a [String],
    pub stack_branches: Option<&'a [Vec<gix::refs::FullName>]>,
    /// Index into `layout.chains` where the AD-HOC chains begin: they splice only after
    /// the entry region exists (managed builds mint that region late).
    pub ad_hoc_chain_start: usize,
    pub region_pinned: &'a IdSet,
    pub claimed_remote_names: &'a HashSet<gix::refs::FullName>,
    pub entrypoint: gix::ObjectId,
    pub entrypoint_ref: Option<&'a gix::refs::FullName>,
    pub target_ref: Option<&'a gix::refs::FullName>,
    pub extra_target: Option<gix::ObjectId>,
}

/// THE BUILD: every materializer pass (mint + connect + chain structure + the remote passes +
/// the coverage regions + the sweeps) run on the store from the decisions alone. The store
/// authors the stored ref positions; it never becomes graph storage itself.
#[tracing::instrument(name = "segment_data::build", level = "trace", skip_all)]
pub(super) fn build(inputs: GraphInputs<'_>) -> SegmentData {
    let (mut store, sidx_of_tip) = mint_segments(
        inputs.cg,
        inputs.tips,
        inputs.in_set,
        inputs.boundaries,
        inputs.plan,
        inputs.workspace_commit,
        inputs.remote_tracking,
    );
    let before_chains = store.segments.len();
    if inputs.stack_branches.is_some() || !inputs.layout.chains.is_empty() {
        build_chain_structure(
            inputs.cg,
            &mut store,
            &sidx_of_tip,
            inputs.workspace_commit,
            inputs.ws_empty_ref,
            inputs.advanced_outside,
            inputs.layout,
            inputs.ad_hoc_chain_start,
            inputs.remote_tracking,
        );
    }
    // The coverage gates evaluate the PRE-CHAIN view: segments minted by the chain-structure
    // pass don't count as coverage.
    let chain_created: HashSet<usize> = (before_chains..store.segments.len()).collect();
    let mut pending_edges: Vec<(usize, gix::ObjectId)> = Vec::new();
    add_remote_segments(
        inputs.cg,
        &mut store,
        &sidx_of_tip,
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
    add_untracked_remote_segments(
        inputs.cg,
        &mut store,
        inputs.remote_tracking,
        &sidx_of_tip,
        inputs.in_set,
        inputs.owner_of,
    );
    surface_target_remote(
        inputs.cg,
        &mut store,
        inputs.target_ref,
        inputs.in_set,
        &sidx_of_tip,
        inputs.owner_of,
        inputs.plan,
        inputs.remote_tracking,
        inputs.region_pinned,
        inputs.claimed_remote_names,
        &mut pending_edges,
    );
    // The extra-target twin: a stored target position uncovered by any pre-chain segment grows
    // its own (nameless) region.
    if let Some(extra) = inputs.extra_target
        && inputs.cg.node(extra).is_some()
        && store
            .sidx_by_commit_excluding(inputs.cg, extra, &chain_created)
            .is_none()
    {
        segment_ahead_region(
            inputs.cg,
            &mut store,
            None,
            extra,
            inputs.in_set,
            &sidx_of_tip,
            inputs.owner_of,
            inputs.remote_tracking,
            None,
            inputs.region_pinned,
            inputs.claimed_remote_names,
            &mut pending_edges,
        );
    }
    // The outside-entrypoint twin: an adhoc checkout outside the workspace grows its region.
    if !inputs.in_set.contains(&inputs.entrypoint)
        && inputs.cg.node(inputs.entrypoint).is_some()
        && store
            .sidx_by_commit_excluding(inputs.cg, inputs.entrypoint, &chain_created)
            .is_none()
    {
        segment_ahead_region(
            inputs.cg,
            &mut store,
            inputs.entrypoint_ref,
            inputs.entrypoint,
            inputs.in_set,
            &sidx_of_tip,
            inputs.owner_of,
            inputs.remote_tracking,
            None,
            inputs.region_pinned,
            inputs.claimed_remote_names,
            &mut pending_edges,
        );
    }
    cover_explicit_seeds(
        inputs.cg,
        &mut store,
        &chain_created,
        inputs.in_set,
        &sidx_of_tip,
        inputs.owner_of,
        inputs.remote_tracking,
        inputs.region_pinned,
        inputs.claimed_remote_names,
        &mut pending_edges,
    );
    // The target's remote segment may exist before its LOCAL got a segment (the local can materialize
    // from the extra-target region above) — link them like every other creator does.
    if let Some(tr) = inputs.target_ref
        && let Some(tr_sidx) = store.sidx_by_ref(tr)
    {
        store.link_remote_to_local(tr_sidx, tr, inputs.remote_tracking);
    }
    add_co_located_remote_empties(inputs.cg, &mut store, inputs.remote_tracking);
    wire_pending_edges(inputs.cg, &mut store, pending_edges);
    // The AD-HOC chains splice now: their anchors live in the entry region, which only
    // exists from this point on (managed builds mint it after the chain structure).
    if inputs.ad_hoc_chain_start < inputs.layout.chains.len() {
        insert_empty_branches(
            inputs.cg,
            &mut store,
            None,
            &inputs.layout.chains[inputs.ad_hoc_chain_start..],
            inputs.layout,
            inputs.remote_tracking,
        );
    }
    if let Some(ep_sidx) = decide_remote_name_float(
        inputs.cg,
        &store,
        inputs.entrypoint,
        inputs.entrypoint_ref,
        inputs.workspace_commit,
    ) {
        apply_remote_name_float(&mut store, ep_sidx);
    }
    if inputs.stack_branches.is_some() {
        drop_suppressed_tip_links(&mut store, inputs.plan, &sidx_of_tip);
    }
    store.entrypoint = Some(inputs.entrypoint);
    store.entrypoint_sidx = decide_entrypoint_row(
        inputs.cg,
        &store,
        inputs.ws_empty_ref,
        inputs.entrypoint,
        inputs.entrypoint_ref,
        inputs.workspace_commit,
    )
    .map(|placement| {
        apply_entrypoint_placement(
            &mut store,
            placement,
            inputs.entrypoint,
            inputs.entrypoint_ref,
            inputs.remote_tracking,
        )
    });
    store
}

/// Tip segments in facts order, float placeholders, then the parent edges — every decision read
/// from the plan data.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "trace", skip_all)]
fn mint_segments(
    cg: &CommitGraph,
    tips: &[gix::ObjectId],
    in_set: &IdSet,
    boundaries: &IdSet,
    plan: &ChainPlan,
    workspace_commit: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> (SegmentData, IdMap<usize>) {
    let mut store = SegmentData::default();
    let mut sidx_of_tip: IdMap<usize> = IdMap::default();
    // Handle space: which segment owns each arena commit, and each run's bottom — filled by the
    // run walks, so the edge pass below needs no per-parent hash lookups.
    let mut sidx_at: Vec<u32> = vec![u32::MAX; cg.node_count()];
    let mut bottom_at: Vec<usize> = Vec::with_capacity(tips.len());
    for (sidx, &tip) in tips.iter().enumerate() {
        let mut bottom = usize::MAX;
        let run = tip_run_and_name(cg, tip, in_set, boundaries, plan, |c| {
            sidx_at[c] = sidx as u32;
            bottom = c;
        });
        bottom_at.push(bottom);
        if let (Some(displaced), Some(&c0)) = (run.displaced, run.commits.first()) {
            store.extra_refs.entry(c0).or_default().push(displaced);
        }
        let sidx = store.add_segment(run.named.as_ref().map(|nt| nt.name.clone()), run.commits);
        if let Some(super::plan::NamedTip {
            name,
            tip: commit_id,
        }) = run.named
        {
            store.set_tip(sidx, commit_id);
            store.segments[sidx].remote_tracking_ref_name = remote_tracking.get(&name).cloned();
        }
        sidx_of_tip.insert(tip, sidx);
    }
    let mut placeholder_of: IdMap<usize> = IdMap::default();
    for float in &plan.floats {
        // Placeholders are synthetic: their name has no resolved ref tip here.
        let sidx = store.add_segment(Some(float.name.clone()), Vec::new());
        store.segments[sidx].remote_tracking_ref_name = remote_tracking.get(&float.name).cloned();
        placeholder_of.insert(float.tip, sidx);
    }
    let float_tips: IdSet = plan.floats.iter().map(|fl| fl.tip).collect();
    for (src, &tip) in tips.iter().enumerate() {
        // A tip is an in-set boundary, so its run has at least itself.
        debug_assert_ne!(bottom_at[src], usize::MAX);
        // One edge per in-graph parent of the run's bottom, in first-parent order; the
        // WORKSPACE commit's edge to a floated parent goes to the float's placeholder.
        for p in cg.connected_parents_at(bottom_at[src]) {
            let r = sidx_at[p];
            if r == u32::MAX {
                continue;
            }
            let dst = if tip == workspace_commit && float_tips.contains(&cg.id_at(p)) {
                placeholder_of[&cg.id_at(p)]
            } else {
                r as usize
            };
            store.connect(src, dst);
        }
    }
    for float in &plan.floats {
        let (Some(&ph), Some(&tip_sidx)) =
            (placeholder_of.get(&float.tip), sidx_of_tip.get(&float.tip))
        else {
            continue;
        };
        store.connect(ph, tip_sidx);
    }
    (store, sidx_of_tip)
}

/// The empty-workspace splice, the decided advanced-outside branches, then the store's
/// chains — all consumed as data.
#[tracing::instrument(level = "trace", skip_all)]
#[allow(clippy::too_many_arguments)]
fn build_chain_structure(
    cg: &CommitGraph,
    store: &mut SegmentData,
    sidx_of_tip: &IdMap<usize>,
    workspace_commit: gix::ObjectId,
    ws_empty_ref: Option<&gix::refs::FullName>,
    advanced_outside: &[AdvancedOutside],
    layout: &LayoutPlan,
    ad_hoc_chain_start: usize,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let mut ws_empty_sidx = None;
    if let Some(ws_ref) = ws_empty_ref
        && let Some(&stack) = sidx_of_tip.get(&workspace_commit)
    {
        let sidx = store.add_segment(Some(ws_ref.clone()), Vec::new());
        store.set_tip(sidx, workspace_commit);
        store.connect(sidx, stack);
        ws_empty_sidx = Some(sidx);
    }
    for decision in advanced_outside {
        let Some(owner) = store.sidx_by_commit(cg, decision.rejoin) else {
            continue;
        };
        let sidx = store.add_segment(decision.name.clone(), decision.commits.clone());
        if let Some(name) = decision.name.as_ref() {
            store.set_tip(sidx, decision.tip);
            store.segments[sidx].remote_tracking_ref_name = remote_tracking.get(name).cloned();
            // Only a NAMED advanced branch is the in-workspace segment's sibling; the workspace
            // position itself never links to outside content.
            if decision.rejoin != workspace_commit
                && store.segments[owner].sibling_segment_id.is_none()
            {
                store.segments[owner].sibling_segment_id = Some(sidx);
            }
        }
        store.connect(sidx, owner);
    }
    let ws_sidx = ws_empty_sidx.or_else(|| sidx_of_tip.get(&workspace_commit).copied());
    insert_empty_branches(
        cg,
        store,
        ws_sidx,
        &layout.chains[..ad_hoc_chain_start.min(layout.chains.len())],
        layout,
        remote_tracking,
    );
}

/// Anonymous bases lose their names, naming refs take their anchors, empties splice above in metadata
/// order.
#[tracing::instrument(level = "trace", skip_all)]
fn insert_empty_branches(
    cg: &CommitGraph,
    store: &mut SegmentData,
    ws_sidx: Option<usize>,
    chains: &[RefChain],
    layout: &LayoutPlan,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    for &tip in &layout.anonymous_bases {
        let Some(anchor) = store.sidx_by_commit(cg, tip) else {
            continue;
        };
        store.clear_name(anchor);
        store.segments[anchor].remote_tracking_ref_name = None;
        store.segments[anchor].remote_tracking_branch_segment_id = None;
    }
    for (li, chain) in chains.iter().enumerate() {
        // Without a workspace anchor (the late ad-hoc pass), the chain hangs from its
        // own first anchor's segment — the pure ad-hoc shape.
        let mut from_sidx = ws_sidx.or_else(|| {
            chain
                .anchors
                .first()
                .and_then(|&(commit, _)| store.sidx_by_commit(cg, commit))
        });
        for &(commit, gi) in &chain.anchors {
            let group = &layout.at_commit[&commit][gi];
            if group.placement == GroupPlacement::Skipped {
                continue;
            }
            let Some(anchor) = store.sidx_by_commit(cg, commit) else {
                continue;
            };
            if let Some(naming_ref) = &group.naming_ref {
                store.set_name(anchor, naming_ref.name.clone(), Some(commit));
                store.segments[anchor].remote_tracking_ref_name =
                    remote_tracking.get(&naming_ref.name).cloned();
                if naming_ref.clear_remote {
                    store.segments[anchor].remote_tracking_branch_segment_id = None;
                }
            }
            if let GroupPlacement::Splice {
                members,
                into_owning_chain,
            } = &group.placement
                && !members.is_empty()
            {
                insert_empty_chain_above(
                    store,
                    from_sidx,
                    anchor,
                    members,
                    remote_tracking,
                    *into_owning_chain,
                    (from_sidx == ws_sidx).then_some(li),
                );
            }
            from_sidx = Some(anchor);
        }
    }
}

/// Splice a run of empty named segments in above `anchor`: incoming edges redirect to the top
/// empty (or a fresh edge from `from_sidx` joins it), and the bottom empty connects down.
#[allow(clippy::too_many_arguments)]
fn insert_empty_chain_above(
    store: &mut SegmentData,
    from_sidx: Option<usize>,
    anchor: usize,
    empties: &[gix::refs::FullName],
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    into_owning_chain: bool,
    fresh_connection_position: Option<usize>,
) {
    let ids: Vec<usize> = empties
        .iter()
        .map(|b| {
            let sidx = store.add_segment(Some(b.clone()), Vec::new());
            store.segments[sidx].remote_tracking_ref_name = remote_tracking.get(b).cloned();
            sidx
        })
        .collect();
    let Some(&top) = ids.first() else {
        return;
    };
    if let Some(from_sidx) = from_sidx {
        let mut redirected = false;
        let redirect_sources: Vec<usize> = if into_owning_chain {
            (0..store.segments.len())
                .filter(|&sidx| !ids.contains(&sidx) && !store.is_remote_segment(sidx))
                .collect()
        } else {
            vec![from_sidx]
        };
        for source in redirect_sources {
            redirected |= store.retarget_edges(source, anchor, top) > 0;
        }
        if !redirected {
            let find_parent = |require_commits: bool| {
                (0..store.segments.len()).find(|&sidx| {
                    sidx != from_sidx
                        && !store.is_remote_segment(sidx)
                        && (!require_commits || !store.segments[sidx].commits.is_empty())
                        && store.segments[sidx].connections.contains(&anchor)
                })
            };
            let chain_parent = into_owning_chain
                .then(|| find_parent(true).or_else(|| find_parent(false)))
                .flatten();
            match chain_parent {
                Some(parent) => {
                    store.retarget_edges(parent, anchor, top);
                }
                None => match fresh_connection_position {
                    Some(parent_number) => store.insert_connect_at(from_sidx, parent_number, top),
                    None => store.connect(from_sidx, top),
                },
            }
        }
    }
    for i in 0..ids.len() {
        let next = ids.get(i + 1).copied().unwrap_or(anchor);
        store.connect(ids[i], next);
    }
}

/// An uncovered explicit seed grows its own region; a covered one whose ref names no segment gets
/// an empty tip-named segment above its owner.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "trace", skip_all)]
fn cover_explicit_seeds(
    cg: &CommitGraph,
    store: &mut SegmentData,
    chain_created: &HashSet<usize>,
    in_set: &IdSet,
    sidx_of_tip: &IdMap<usize>,
    owner_of: &IdMap<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    region_pinned: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(usize, gix::ObjectId)>,
) {
    for t in cg.seeds.iter().filter(|_| cg.explicit_seeds) {
        if cg.node(t.id).is_none() {
            continue;
        }
        match store.sidx_by_commit_excluding(cg, t.id, chain_created) {
            None => segment_ahead_region(
                cg,
                store,
                t.ref_name.as_ref(),
                t.id,
                in_set,
                sidx_of_tip,
                owner_of,
                remote_tracking,
                None,
                region_pinned,
                claimed_remote_names,
                pending_edges,
            ),
            Some(owner_sidx) => {
                let Some(ref_name) = t.ref_name.clone() else {
                    continue;
                };
                if but_core::is_workspace_ref_name(ref_name.as_ref()) {
                    continue;
                }
                if store.sidx_by_ref(&ref_name).is_some()
                    || store.segments[owner_sidx].ref_name() == Some(ref_name.as_ref())
                {
                    continue;
                }
                // The plan deliberately left this tip-started segment anonymous — keep it that way.
                if store.segments[owner_sidx]
                    .commits
                    .first()
                    .is_some_and(|&h| cg.id_at(h) == t.id)
                    && store.segments[owner_sidx].name.is_none()
                {
                    continue;
                }
                let empty = store.add_segment(Some(ref_name.clone()), Vec::new());
                store.set_tip(empty, t.id);
                store.segments[empty].remote_tracking_ref_name =
                    remote_tracking.get(&ref_name).cloned();
                store.connect(empty, owner_sidx);
            }
        }
    }
}

/// Connect each stopped segment to the segment owning its parent commit — every creator has run by
/// now, so the owner exists.
#[tracing::instrument(level = "trace", skip_all)]
fn wire_pending_edges(
    cg: &CommitGraph,
    store: &mut SegmentData,
    pending_edges: Vec<(usize, gix::ObjectId)>,
) {
    for (src, parent) in pending_edges {
        let Some(dst) = store.sidx_by_commit(cg, parent) else {
            continue;
        };
        store.connect(src, dst);
    }
}

/// Whether a ref-less checkout sits at a remote-named segment's tip — decided READ-ONLY;
/// [`apply_remote_name_float`] performs the move. Returns the segment whose name floats.
fn decide_remote_name_float(
    cg: &CommitGraph,
    store: &SegmentData,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    workspace_commit: gix::ObjectId,
) -> Option<usize> {
    if entrypoint_ref.is_some() || entrypoint == workspace_commit {
        return None;
    }
    let ep_sidx = store.sidx_by_commit(cg, entrypoint)?;
    (store.is_remote_segment(ep_sidx)
        && store.segments[ep_sidx]
            .commits
            .first()
            .map(|&h| cg.id_at(h))
            == Some(entrypoint))
    .then_some(ep_sidx)
}

/// Apply the float: the remote name (and its links) moves to a fresh empty segment above
/// the checkout's segment; edges and links aimed at the named segment follow.
#[tracing::instrument(level = "trace", skip_all)]
fn apply_remote_name_float(store: &mut SegmentData, ep_sidx: usize) {
    let name = store.take_name(ep_sidx);
    let tip = store.segments[ep_sidx].tip.take();
    let rt_name = store.segments[ep_sidx].remote_tracking_ref_name.take();
    let sibling = store.segments[ep_sidx].sibling_segment_id.take();
    let rt_row = store.segments[ep_sidx]
        .remote_tracking_branch_segment_id
        .take();
    let floated = store.add_segment(None, Vec::new());
    store.put_name(floated, name);
    store.segments[floated].tip = tip;
    store.segments[floated].remote_tracking_ref_name = rt_name;
    store.segments[floated].sibling_segment_id = sibling;
    store.segments[floated].remote_tracking_branch_segment_id = rt_row;
    for sidx in 0..store.segments.len() {
        if sidx == floated {
            continue;
        }
        if store.segments[sidx].sibling_segment_id == Some(ep_sidx) {
            store.segments[sidx].sibling_segment_id = Some(floated);
        }
        if store.segments[sidx].remote_tracking_branch_segment_id == Some(ep_sidx) {
            store.segments[sidx].remote_tracking_branch_segment_id = Some(floated);
        }
        store.retarget_edges(sidx, ep_sidx, floated);
    }
    store.connect(floated, ep_sidx);
}

/// A floated or anonymized tip's build-time name lost its remote links to whichever segment finally
/// carries the name.
fn drop_suppressed_tip_links(
    store: &mut SegmentData,
    plan: &ChainPlan,
    sidx_of_tip: &IdMap<usize>,
) {
    for tip in plan
        .floats
        .iter()
        .map(|fl| fl.tip)
        .chain(plan.anonymous_bases.iter().copied())
    {
        if let Some(&sidx) = sidx_of_tip.get(&tip) {
            store.segments[sidx].remote_tracking_ref_name = None;
            store.segments[sidx].remote_tracking_branch_segment_id = None;
        }
    }
}

/// How the entrypoint lands in the store, decided READ-ONLY by
/// [`decide_entrypoint_row`] and applied by [`apply_entrypoint_placement`] — the build's
/// last two structure changes, kept late on purpose: whether the checked-out ref ended
/// up naming a segment is knowledge no earlier pass has.
enum EntrypointPlacement {
    /// An existing segment is the entry row as-is.
    Existing(usize),
    /// The anonymous segment starting at the entrypoint takes the checked-out ref's name.
    Name(usize),
    /// The entry segment is named by ANOTHER ref: an empty entrypoint-named segment
    /// splices in above and becomes the row.
    SpliceAbove(usize),
}

/// Pick the entrypoint's segment — the empty workspace segment for a ref-less checkout AT
/// the workspace position, the segment the checked-out ref already names, or the segment
/// STARTING at the entrypoint commit (named or split when the placement is applied).
fn decide_entrypoint_row(
    cg: &CommitGraph,
    store: &SegmentData,
    ws_empty_ref: Option<&gix::refs::FullName>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    workspace_commit: gix::ObjectId,
) -> Option<EntrypointPlacement> {
    if let (Some(ws_sidx), None, true) = (
        ws_empty_ref.and_then(|r| store.sidx_by_ref(r)),
        entrypoint_ref,
        entrypoint == workspace_commit,
    ) {
        return Some(EntrypointPlacement::Existing(ws_sidx));
    }
    if let Some(named) = entrypoint_ref.and_then(|r| store.sidx_by_ref(r)) {
        return Some(EntrypointPlacement::Existing(named));
    }
    let (sidx, pos) = (0..store.segments.len()).find_map(|sidx| {
        store.segments[sidx]
            .commits
            .iter()
            .position(|&h| cg.id_at(h) == entrypoint)
            .map(|p| (sidx, p))
    })?;
    if pos != 0 {
        return None;
    }
    let Some(ep_ref) = entrypoint_ref else {
        return Some(EntrypointPlacement::Existing(sidx));
    };
    Some(match store.segments[sidx].ref_name() {
        None => EntrypointPlacement::Name(sidx),
        Some(existing) if existing != ep_ref.as_ref() => EntrypointPlacement::SpliceAbove(sidx),
        Some(_) => EntrypointPlacement::Existing(sidx),
    })
}

/// Apply the placement: name the anonymous entry segment, or splice the empty
/// entrypoint-named segment in above (which becomes the row).
fn apply_entrypoint_placement(
    store: &mut SegmentData,
    placement: EntrypointPlacement,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> usize {
    match placement {
        EntrypointPlacement::Existing(sidx) => sidx,
        EntrypointPlacement::Name(sidx) => {
            let ep_ref = entrypoint_ref.expect("Name placements carry a checked-out ref");
            store.set_name(sidx, ep_ref.clone(), Some(entrypoint));
            store.segments[sidx].remote_tracking_ref_name = remote_tracking.get(ep_ref).cloned();
            sidx
        }
        EntrypointPlacement::SpliceAbove(sidx) => {
            let ep_ref = entrypoint_ref.expect("SpliceAbove placements carry a checked-out ref");
            let empty = store.add_segment(Some(ep_ref.clone()), Vec::new());
            store.set_tip(empty, entrypoint);
            store.segments[empty].remote_tracking_ref_name = remote_tracking.get(ep_ref).cloned();
            for other in 0..store.segments.len() {
                if other == empty {
                    continue;
                }
                store.retarget_edges(other, sidx, empty);
            }
            store.connect(empty, sidx);
            empty
        }
    }
}

/// The final enrichments, both pure functions of a name: worktree annotation (segment names and
/// the refs riding commits) and metadata classification. Runs once the names are final.
pub(super) fn enrich<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    store: &SegmentData,
    meta: &T,
    worktree_by_branch: &BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
) -> (
    std::collections::HashMap<gix::refs::FullName, crate::workspace::BranchDetails>,
    Option<crate::workspace::WorkspaceMeta>,
) {
    let mut details = std::collections::HashMap::new();
    let mut workspace_meta = None;
    for row in &store.segments {
        let Some(name) = row.name.as_ref() else {
            continue;
        };
        let metadata = super::segment_metadata(name.as_ref(), meta);
        if workspace_meta.is_none()
            && let Some(crate::SegmentMetadata::Workspace(ws)) = &metadata
        {
            workspace_meta = Some(crate::workspace::WorkspaceMeta {
                ref_name: name.clone(),
                metadata: ws.clone(),
            });
        }
        let remote_walk_tip = row.remote_tracking_branch_segment_id.and_then(|rsidx| {
            let remote = &store.segments[rsidx];
            remote
                .commits
                .first()
                .map(|&h| cg.id_at(h))
                // A caught-up remote's row is empty; its ref target is the
                // walk start then.
                .or(remote.tip)
        });
        details.insert(
            name.clone(),
            crate::workspace::BranchDetails {
                metadata: metadata.and_then(|md| match md {
                    crate::SegmentMetadata::Branch(md) => Some(md),
                    crate::SegmentMetadata::Workspace(_) => None,
                }),
                worktree: worktree_by_branch
                    .get(name)
                    .and_then(|w| w.first())
                    .cloned(),
                remote_walk_tip,
            },
        );
    }
    (details, workspace_meta)
}
