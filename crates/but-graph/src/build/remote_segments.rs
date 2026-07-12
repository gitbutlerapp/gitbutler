//! The remote passes of the build: locals' remote-tracking counterparts become segments —
//! behind/at remotes as empty roots into the owning segment, ahead remotes segmented region
//! by region ([`segment_ahead_region`], also the region-grower for extra targets, outside
//! entrypoints, and explicit seeds) — plus the untracked same-tip remotes, the target remote's
//! surfacing, and the co-located remote empties. Every pass mutates the row arena
//! ([`SegmentData`]) and wires sibling/tracking links as it creates rows.

use std::collections::{HashMap, HashSet};

use gix::reference::Category;

use super::materialize::commit_run;
use super::plan::ChainPlan;
use super::remotes::{AheadRegion, region_tips, remote_name_in_play, unique_plain_local};
use super::segment_data::SegmentData;
use super::{IdMap, IdSet, is_plain_local_branch};
use crate::CommitGraph;

/// The remote pass: locals keyed on the plan's pre-chain names in link order, behind/at
/// remotes as empty roots (skipped when the plan's rename already named the owner — that
/// owner still gets the sibling/tracking links), ahead remotes segmented region by region.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn add_remote_segments(
    cg: &CommitGraph,
    store: &mut SegmentData,
    sidx_of_tip: &IdMap<usize>,
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
    let mut locals: Vec<(usize, gix::refs::FullName)> = sidx_of_tip
        .iter()
        .filter_map(|(&tip, &sidx)| {
            let name = plan.base_name_of.get(&tip)?;
            let rt = remote_tracking.get(name).cloned()?;
            Some((store.sidx_by_ref(name).unwrap_or(sidx), rt))
        })
        .collect();
    locals.sort_by_key(|&(sidx, ..)| sidx);
    for (link_sidx, remote_ref) in locals {
        let Some(remote_tip) = cg.commit_by_ref(remote_ref.as_ref()) else {
            continue;
        };
        if in_set.contains(&remote_tip) {
            let owner = owner_of.get(&remote_tip).copied().unwrap_or(remote_tip);
            let owner_sidx = sidx_of_tip[&owner];
            let named_by_this = plan
                .renames
                .get(&owner)
                .is_some_and(|(name, _)| name == &remote_ref);
            if named_by_this {
                store.segments[owner_sidx].sibling_segment_id = Some(link_sidx);
                store.segments[link_sidx].remote_tracking_branch_segment_id = Some(owner_sidx);
            } else {
                let remote_sidx = store.add_segment(Some(remote_ref.clone()), Vec::new());
                store.set_tip(remote_sidx, remote_tip);
                store.segments[remote_sidx].sibling_segment_id = Some(link_sidx);
                store.segments[link_sidx].remote_tracking_branch_segment_id = Some(remote_sidx);
                store.connect(remote_sidx, owner_sidx);
            }
            continue;
        }
        let in_play = remote_name_in_play(&remote_ref, symbolic_remotes);
        if !in_play || super::is_stack_branch(stack_branches, &remote_ref) {
            continue;
        }
        segment_ahead_region(
            cg,
            store,
            Some(&remote_ref),
            remote_tip,
            in_set,
            sidx_of_tip,
            owner_of,
            remote_tracking,
            Some(link_sidx),
            region_pinned,
            claimed_remote_names,
            pending_edges,
        );
    }
}

/// Segment one AHEAD region into segments: the [`AheadRegion`] shape, the interior cut/stop scan,
/// then segments and edges in creation order.
#[allow(clippy::too_many_arguments)]
pub(super) fn segment_ahead_region(
    cg: &CommitGraph,
    store: &mut SegmentData,
    remote_ref: Option<&gix::refs::FullName>,
    remote_tip: gix::ObjectId,
    in_set: &IdSet,
    sidx_of_tip: &IdMap<usize>,
    owner_of: &IdMap<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    local_sidx: Option<usize>,
    pinned_commits: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(usize, gix::ObjectId)>,
) {
    let region = AheadRegion::compute(cg, remote_tip, in_set);
    let ahead_set = &region.set;
    let is_boundary =
        |c: gix::ObjectId| region.is_shape_boundary(cg, remote_tip, pinned_commits, c);
    // Merge-heavy regions make nearly every commit a tip, and a per-tip store scan is
    // quadratic (20s on an 80k-commit repo) — index first commits up front instead.
    let mut first_commit_sidx: IdMap<usize> = IdMap::default();
    let mut remote_first_commits: IdSet = IdSet::default();
    for (sidx, seg) in store.segments.iter().enumerate() {
        if let Some(&first) = seg.commits.first() {
            first_commit_sidx.entry(cg.id_at(first)).or_insert(sidx);
            if store.is_remote_segment(sidx) {
                remote_first_commits.insert(cg.id_at(first));
            }
        }
    }

    let root_is_remote =
        remote_ref.is_some_and(|r| r.as_ref().category() == Some(Category::RemoteBranch));
    let mut interior_cuts: IdMap<gix::refs::FullName> = IdMap::default();
    let mut stop: Option<gix::ObjectId> = None;
    if root_is_remote {
        let existing_remote_tip = |c: gix::ObjectId| remote_first_commits.contains(&c);
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
                    && store.sidx_by_ref(r).is_none()
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
    let mut ahead_sidx: IdMap<usize> = IdMap::default();
    let mut reused: IdSet = IdSet::default();
    for &tip in &tips {
        if stop == Some(tip) {
            continue;
        }
        let commits = commit_run(cg, tip, ahead_set, &is_boundary, |_| {});
        for &c in &commits {
            ahead_owner.insert(cg.id_at(c), tip);
        }
        let is_root = tip == remote_tip;
        if !is_root && let Some(&existing) = first_commit_sidx.get(&tip) {
            ahead_sidx.insert(tip, existing);
            reused.insert(tip);
            continue;
        }
        // Like the interior cuts: a name that already names a segment (e.g. an
        // advanced-outside branch over these same commits) is taken — the run stays
        // anonymous rather than authoring the name twice.
        let name = (if is_root {
            remote_ref
                .cloned()
                .or_else(|| unique_plain_local(cg, remote_tip))
        } else {
            interior_cuts
                .get(&tip)
                .cloned()
                .or_else(|| unique_plain_local(cg, tip))
        })
        .filter(|name| store.sidx_by_ref(name).is_none());
        let sidx = store.add_segment(name, commits);
        if let Some(name) = store.segments[sidx].ref_name().map(|n| n.to_owned()) {
            store.set_tip(sidx, if is_root { remote_tip } else { tip });
            if is_plain_local_branch(&name) {
                store.segments[sidx].remote_tracking_ref_name = remote_tracking.get(&name).cloned();
            }
        }
        if is_root {
            store.segments[sidx].sibling_segment_id = local_sidx;
            if let Some(local_sidx) = local_sidx {
                store.segments[local_sidx].remote_tracking_branch_segment_id = Some(sidx);
            }
        }
        if let Some(cut_ref) = interior_cuts.get(&tip) {
            let cut_ref = cut_ref.clone();
            store.link_remote_to_local(sidx, &cut_ref, remote_tracking);
        }
        if let Some(&first) = store.segments[sidx].commits.first() {
            first_commit_sidx.entry(cg.id_at(first)).or_insert(sidx);
        }
        ahead_sidx.insert(tip, sidx);
    }

    for &tip in &tips {
        if reused.contains(&tip) || stop == Some(tip) {
            continue;
        }
        let src = ahead_sidx[&tip];
        let bottom = store.segments[src]
            .commits
            .last()
            .map(|&h| cg.id_at(h))
            .unwrap_or(tip);
        for parent in cg.all_parent_ids(bottom) {
            let dst = if ahead_set.contains(&parent) {
                ahead_owner
                    .get(&parent)
                    .and_then(|o| ahead_sidx.get(o))
                    .copied()
            } else {
                owner_of
                    .get(&parent)
                    .and_then(|o| sidx_of_tip.get(o))
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

/// Unclaimed remote refs whose local counterpart shares the tip become empty roots into the
/// owning segment (in-set case only).
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn add_untracked_remote_segments(
    cg: &CommitGraph,
    store: &mut SegmentData,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    sidx_of_tip: &IdMap<usize>,
    in_set: &IdSet,
    owner_of: &IdMap<gix::ObjectId>,
) {
    for r in super::remote_refs(cg) {
        if store.sidx_by_ref(&r).is_some() {
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
            && let Some(&owner_sidx) = sidx_of_tip.get(&owner)
        {
            let remote_sidx = store.add_segment(Some(r.clone()), Vec::new());
            store.set_tip(remote_sidx, tip);
            store.connect(remote_sidx, owner_sidx);
            store.link_remote_to_local(remote_sidx, &r, remote_tracking);
        }
    }
}

/// Surface the target remote: the in-set case adds the sibling link when the plan's rename
/// already named the owner; an outside target grows its region, and a local tracking branch
/// on the region's tip takes the name — the remote becomes an empty segment above it,
/// sibling-linked.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn surface_target_remote(
    cg: &CommitGraph,
    store: &mut SegmentData,
    target_ref: Option<&gix::refs::FullName>,
    in_set: &IdSet,
    sidx_of_tip: &IdMap<usize>,
    owner_of: &IdMap<gix::ObjectId>,
    plan: &ChainPlan,
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
    if in_set.contains(&tip) {
        let owner_tip = owner_of.get(&tip).copied().unwrap_or(tip);
        if plan.renames.get(&owner_tip).is_some_and(|(n, _)| n == tr)
            && let Some(owner_sidx) = store.sidx_by_commit(cg, tip)
        {
            // Sibling: the segment whose FIRST commit is the local tracking ref's position.
            let local_sidx = remote_tracking
                .iter()
                .find(|(_, r)| *r == tr)
                .and_then(|(local, _)| cg.commit_by_ref(local.as_ref()))
                .and_then(|lc| {
                    store.sidx_by_commit(cg, lc).filter(|&sidx| {
                        sidx != owner_sidx
                            && store.segments[sidx]
                                .commits
                                .first()
                                .is_some_and(|&h| cg.id_at(h) == lc)
                    })
                });
            if let Some(local_sidx) = local_sidx {
                store.segments[owner_sidx].sibling_segment_id = Some(local_sidx);
            }
        }
        return;
    }
    if store.sidx_by_ref(tr).is_some() {
        return;
    }
    segment_ahead_region(
        cg,
        store,
        Some(tr),
        tip,
        in_set,
        sidx_of_tip,
        owner_of,
        remote_tracking,
        None,
        region_pinned,
        claimed_remote_names,
        pending_edges,
    );
    let local_on_tip = remote_tracking
        .iter()
        .find(|(local, r)| *r == tr && cg.commit_by_ref(local.as_ref()) == Some(tip))
        .map(|(local, _)| local.clone());
    if let Some(local) = local_on_tip
        && let Some(owner) = store.sidx_by_commit(cg, tip)
        && store.segments[owner].ref_name() == Some(tr.as_ref())
        && store.segments[owner]
            .commits
            .first()
            .is_some_and(|&h| cg.id_at(h) == tip)
    {
        store.set_name(owner, local, Some(tip));
        store.segments[owner].remote_tracking_ref_name = Some(tr.clone());
        let remote_sidx = store.add_segment(Some(tr.clone()), Vec::new());
        store.set_tip(remote_sidx, tip);
        store.segments[remote_sidx].sibling_segment_id = Some(owner);
        store.segments[owner].remote_tracking_branch_segment_id = Some(remote_sidx);
        store.connect(remote_sidx, owner);
    }
}

/// Every further remote ref on a remote segment's first commit becomes an empty segment pointing at
/// it.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn add_co_located_remote_empties(
    cg: &CommitGraph,
    store: &mut SegmentData,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let existing = store.segments.len();
    for sidx in 0..existing {
        if !store.is_remote_segment(sidx) {
            continue;
        }
        let Some(&first) = store.segments[sidx].commits.first() else {
            continue;
        };
        for ri in cg.node_at(first).refs.clone().iter() {
            if ri.ref_name.as_ref().category() != Some(Category::RemoteBranch)
                || store.sidx_by_ref(&ri.ref_name).is_some()
            {
                continue;
            }
            let empty = store.add_segment(Some(ri.ref_name.clone()), Vec::new());
            store.set_tip(empty, cg.id_at(first));
            store.connect(empty, sidx);
            store.link_remote_to_local(empty, &ri.ref_name, remote_tracking);
        }
    }
}
