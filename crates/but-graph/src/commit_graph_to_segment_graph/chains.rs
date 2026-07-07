//! Workspace chain structure: empty-segment splices that realize metadata-only
//! branches as chains in the graph.

use std::collections::{HashMap, HashSet};

use super::remotes::is_remote_segment;
use super::{
    IdMap, IdSet, connect, disambiguated_ref, is_plain_local_branch, segment_by_commit,
    segment_by_ref,
};
use crate::ref_arrangement::{GroupPlacement, RefArrangement};
use crate::{
    Commit, CommitGraph, RefInfo, Segment, SegmentIndex,
    segment_graph::{Connection, SegmentGraph},
};

/// Splice an empty `gitbutler/workspace` segment above the stack tip the workspace ref is co-located
/// with (no dedicated merge commit). It holds no commits, carries the main worktree, and connects into
/// the stack segment that owns `workspace_commit`.
pub(super) fn insert_empty_workspace_segment(
    sg: &mut SegmentGraph,
    seg_of_tip: &IdMap<SegmentIndex>,
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
) -> Option<SegmentIndex> {
    let stack_sidx = *seg_of_tip.get(&workspace_commit)?;
    // The traversal may have dropped the special workspace ref from the commit's refs when a stack
    // branch on the same commit named its raw segment — the caller established the ref points here,
    // so fall back to the well-known name rather than silently skipping the workspace segment.
    let ws_ref = cg
        .refs_at(workspace_commit)
        .into_iter()
        .find(|r| but_core::is_workspace_ref_name(r.as_ref()))
        .or_else(|| but_core::WORKSPACE_REF_NAME.try_into().ok())?;
    // The worktree annotation comes from the shared `worktree_by_branch` pass — HEAD may well be on
    // a stack branch, not the workspace ref.
    let ws_seg = sg.add_segment(Segment {
        id: 0,
        ref_info: Some(RefInfo {
            ref_name: ws_ref,
            commit_id: Some(workspace_commit),
            worktree: None,
        }),
        remote_tracking_ref_name: None,
        sibling_segment_id: None,
        remote_tracking_branch_segment_id: None,
        commits: Vec::new(),
        metadata: None,
        connections: Vec::new(),
    });
    sg.segment_mut(ws_seg).expect("just added").id = ws_seg;
    connect(sg, ws_seg, stack_sidx);
    Some(ws_seg)
}

/// A metadata stack branch pointing at a commit OUTSIDE the workspace has advanced past it. Surface
/// its outside commits as a segment named after the branch: the first-parent run from its tip down to
/// the first in-workspace commit, connected into the segment owning that commit. That owning segment
/// gets a sibling link so the projection can display it under the advanced branch's name.
#[expect(clippy::too_many_arguments)]
pub(super) fn add_advanced_outside_branches<T: but_core::RefMetadata>(
    sg: &mut SegmentGraph,
    cg: &CommitGraph,
    in_set: &IdSet,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    workspace_commit: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
    pinned_commits: &IdSet,
) {
    for b in stack_branches.into_iter().flatten().flatten() {
        // Only LOCAL branches advance past a workspace; metadata can also list remote refs as stack
        // branches, and those are handled by the remote passes.
        if !is_plain_local_branch(b) || segment_by_ref(sg, b).is_some() {
            continue;
        }
        let Some(tip) = cg.commit_by_ref(b.as_ref()) else {
            continue;
        };
        if in_set.contains(&tip) {
            continue;
        }
        // A PINNED commit (a stored/extra target position) must start its own segment via the
        // extra-target region — the projection derives the remembered base from it. When chains
        // run before the remote passes this pass would otherwise swallow it into the branch's
        // outside run first.
        if pinned_commits.contains(&tip) {
            continue;
        }
        // The branch's outside commits, down to where it rejoins the workspace.
        let mut commits: Vec<Commit> = Vec::new();
        let mut cursor = Some(tip);
        let mut rejoin = None;
        while let Some(id) = cursor {
            if in_set.contains(&id) {
                rejoin = Some(id);
                break;
            }
            if let Some(row) = cg.row(id) {
                commits.push(row.commit.clone());
            }
            cursor = cg.first_parent(id);
        }
        let (Some(rejoin), false) = (rejoin, commits.is_empty()) else {
            continue;
        };
        // Several stack branches can share the outside tip (e.g. an applied-branch preview where
        // `E` and `D` rest on the same not-yet-merged commit) — the run is materialized ONCE.
        if segment_by_commit(sg, tip).is_some() {
            continue;
        }
        let Some(owner_sidx) = segment_by_commit(sg, rejoin) else {
            continue;
        };
        // Named like any tip: ambiguous refs keep the segment anonymous (the walk's floating
        // `►D, ►E` run), a unique branch names it (the advanced `B` above its own chain).
        let ref_info =
            disambiguated_ref(cg, tip, remote_tracking, meta, None, target_ref).map(|ref_name| {
                RefInfo {
                    ref_name,
                    commit_id: Some(tip),
                    worktree: None,
                }
            });
        let named = ref_info.is_some();
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let seg = sg.add_segment(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.segment_mut(seg).expect("just added").id = seg;
        connect(sg, seg, owner_sidx);
        // Only a NAMED advanced branch is the in-workspace segment's sibling (the projection shows
        // that segment under the advanced branch's name); a floating anonymous run stays unlinked,
        // and the workspace position itself never links to outside content.
        if named
            && rejoin != workspace_commit
            && let Some(owner) = sg.segment_mut(owner_sidx)
            && owner.sibling_segment_id.is_none()
        {
            owner.sibling_segment_id = Some(seg);
        }
    }
}

/// Materialize the [table](RefArrangement)'s chains: per metadata stack list, thread the
/// same-commit groups top→bottom — the table-decided namer takes the anchor, the table-decided
/// empties splice above it in metadata order — producing
/// `ws → [empties] → seg(c1) → [empties] → seg(c2) → … → [empties] → base`.
/// Which refs become empties and how a group lands (dependent splice, own chain, passive) is
/// table DATA; this pass only looks up anchors and splices.
pub(super) fn insert_empty_branches(
    sg: &mut SegmentGraph,
    ws_sidx: Option<SegmentIndex>,
    arrangement: &RefArrangement,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    // DEMOTIONS, decided by `chain_plan`: a shared base at/below the bound stays anonymous while
    // every chain's branches float above as their own chain; the lower-bound anchor of an
    // otherwise-unrepresented chain floats likewise. Remote links of a demoted name are
    // established on the floated segment by the remote creators.
    for &tip in &arrangement.demoted {
        let Some(anchor) = segment_by_commit(sg, tip) else {
            continue;
        };
        if let Some(s) = sg.segment_mut(anchor) {
            s.ref_info = None;
            s.remote_tracking_ref_name = None;
            s.remote_tracking_branch_segment_id = None;
        }
    }
    for (li, chain) in arrangement.chains.iter().enumerate() {
        // `from_sidx` feeds the top of the chain: the workspace segment for the first group, then each
        // group's anchor for the next (so its empties splice into the edge coming from above).
        let mut from_sidx = ws_sidx;
        for &(commit, gi) in &chain.anchors {
            let group = &arrangement.at_commit[&commit][gi];
            // Outside the workspace or co-located with a managed merge commit: nothing to place.
            if group.placement == GroupPlacement::Skipped {
                continue;
            }
            let Some(anchor) = segment_by_commit(sg, commit) else {
                continue;
            };
            // GROUP NAMING, decided by `chain_plan`: the bottom-most branch names an anonymous
            // anchor; metadata order overrides a build-time name that belongs to the group (its
            // remote links are cleared, the remote creators link its floated empty instead).
            if let Some(namer) = &group.namer
                && let Some(s) = sg.segment_mut(anchor)
            {
                s.ref_info = Some(RefInfo {
                    ref_name: namer.name.clone(),
                    commit_id: Some(commit),
                    worktree: None,
                });
                s.remote_tracking_ref_name = remote_tracking.get(&namer.name).cloned();
                if namer.clear_remote {
                    s.remote_tracking_branch_segment_id = None;
                }
            }
            if !group.empties.is_empty() {
                // ANOTHER stack owns the (non-integrated) commit: these branches stay PASSIVE
                // refs — consumers (apply) discover them on the commit and record them as
                // dependent branches in THIS stack's metadata, after which the same-list path
                // splices them.
                if group.placement == GroupPlacement::Passive {
                    from_sidx = Some(anchor);
                    continue;
                }
                let dependent = group.placement == GroupPlacement::Dependent;
                insert_empty_chain_above(
                    sg,
                    from_sidx,
                    anchor,
                    &group.empties,
                    remote_tracking,
                    dependent,
                    dependent,
                    // A fresh chain straight off the workspace lands at its metadata position:
                    // chains are threaded in metadata-stack order, so `li` is the slot among the
                    // workspace's connections (existing chains sit in parent-array order, which
                    // metadata mirrors in steady state).
                    (from_sidx == ws_sidx).then_some(li),
                );
            }
            from_sidx = Some(anchor);
        }
    }
}

/// The workspace's LOWER BOUND: the nearest commit common to the target and EVERY workspace parent
/// (the walk's `compute_lowest_base` — the base all stacks and the target converge on). BFS from the
/// workspace over all parents, so the nearest such commit wins.
pub(super) fn workspace_lower_bound(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    target: gix::ObjectId,
) -> Option<gix::ObjectId> {
    let mut common = cg.ancestor_set(target);
    for parent in cg.all_parent_ids(workspace_commit) {
        let parent_ancestors = cg.ancestor_set(parent);
        common.retain(|c| parent_ancestors.contains(c));
    }
    let mut seen = HashSet::new();
    let mut queue = std::collections::VecDeque::from([workspace_commit]);
    while let Some(c) = queue.pop_front() {
        if common.contains(&c) {
            return Some(c);
        }
        if seen.insert(c) {
            queue.extend(cg.all_parent_ids(c));
        }
    }
    None
}

/// The lower bound the PROJECTION will use: the merge base with the target, extended DOWN to a
/// stored/extra target position lying below it — an older target location keeps the commits
/// integrated since then visible, so stacks resting between the bound and the merge base are real
/// (kept) stacks, not empty floats.
pub(super) fn effective_lower_bound(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    target: Option<gix::ObjectId>,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    options: &crate::init::Options,
) -> Option<gix::ObjectId> {
    let mut lb = target
        .or(project_meta.target_commit_id)
        .or(options.extra_target_commit_id)
        .and_then(|t| workspace_lower_bound(cg, workspace_commit, t))?;
    for candidate in [
        project_meta.target_commit_id,
        options.extra_target_commit_id,
    ]
    .into_iter()
    .flatten()
    {
        if candidate != lb && cg.ancestor_set(lb).contains(&candidate) {
            lb = candidate;
        }
    }
    Some(lb)
}

/// Splice `empties` as a chain of empty segments ABOVE `anchor`, routing `from_sidx` to `anchor`
/// through them. If `from_sidx` already has edge(s) into `anchor` (including a merge's duplicate
/// parents), they are moved onto the chain top; if it has none — because a sibling empty stack already
/// consumed the shared edge to `anchor` (two empty stacks on the same base) — a fresh edge is added.
/// Other stacks' and remotes' edges into `anchor` are untouched. Produces `top_empty → … → anchor`.
#[allow(clippy::too_many_arguments)]
pub(super) fn insert_empty_chain_above(
    sg: &mut SegmentGraph,
    from_sidx: Option<SegmentIndex>,
    anchor: SegmentIndex,
    empties: &[gix::refs::FullName],
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    // The anchor commit sits strictly inside another stack's chain (not at/below the base): splice into
    // that chain's existing edge rather than adding a fresh workspace chain.
    dependent: bool,
    // Route EVERY incoming edge to the anchor through the chain (a splice INTO the chain, above the
    // bound): both the workspace's parent edge and the chain edge from the commit-holding segment
    // above enter at the chain top — the walk's inline-splice shape. `false` keeps other stacks'
    // direct edges (a true shared base where each stack has its own chain).
    redirect_all: bool,
    // Where a FRESH chain edge goes in `from_sidx`'s connections: the stack's metadata index, so a
    // new empty stack surfaces at its metadata position (e.g. on top for `Some(0)`) instead of
    // last. Connection order on the workspace segment is stack order in the projection. `None` appends.
    fresh_connection_slot: Option<usize>,
) {
    let seg_ids: Vec<SegmentIndex> = empties
        .iter()
        .map(|b| {
            let s = sg.add_segment(Segment {
                id: 0,
                ref_info: Some(RefInfo {
                    ref_name: b.clone(),
                    // Metadata-derived empties are synthetic: no resolved ref tip. A `Some` would
                    // make consumers treat the anchor commit as this branch's amendable tip.
                    commit_id: None,
                    worktree: None,
                }),
                remote_tracking_ref_name: remote_tracking.get(b).cloned(),
                sibling_segment_id: None,
                remote_tracking_branch_segment_id: None,
                commits: Vec::new(),
                metadata: None,
                connections: Vec::new(),
            });
            sg.segment_mut(s).expect("just added").id = s;
            s
        })
        .collect();
    let Some(&top) = seg_ids.first() else {
        return;
    };
    // Move `from_sidx`'s edge(s) into the anchor onto the chain top; other stacks and remotes that also
    // reach the anchor keep their direct edges. If it has none, the anchor may sit MID-CHAIN of another
    // stack (dependent branches, e.g. `D`/`E` pointing into `S1`'s spine): splice into the existing
    // incoming edge from the commit-holding local segment above, matching the walk — a fresh workspace
    // edge would mint a duplicate chain showing the anchor's commits twice. Only when no such chain
    // parent exists (a sibling empty stack already took the shared edge to this base) does a fresh
    // edge connect this stack from above.
    if let Some(from_sidx) = from_sidx {
        let mut redirected = false;
        let redirect_sources: Vec<SegmentIndex> = if redirect_all {
            sg.segment_ids()
                .filter(|&s| !seg_ids.contains(&s) && !is_remote_segment(sg, s))
                .collect()
        } else {
            vec![from_sidx]
        };
        for source in redirect_sources {
            redirected |= sg.retarget_edges(source, anchor, top) > 0;
        }
        if !redirected {
            // Prefer a commit-holding chain parent (the dependent-branch pattern); an EMPTY one —
            // another stack's branch already spliced above the same anchor — also carries the
            // chain, so a further dependent branch slots in underneath it rather than minting a
            // fresh chain.
            let find_parent = |require_commits: bool| {
                sg.segment_ids().find(|&sidx| {
                    sidx != from_sidx
                        && !is_remote_segment(sg, sidx)
                        && sg.segment(sidx).is_some_and(|s| {
                            (!require_commits || !s.commits.is_empty())
                                && s.connections.iter().any(|c| c.target == anchor)
                        })
                })
            };
            let chain_parent = dependent
                .then(|| find_parent(true).or_else(|| find_parent(false)))
                .flatten();
            match chain_parent {
                Some(parent) => {
                    sg.retarget_edges(parent, anchor, top);
                }
                None => match fresh_connection_slot {
                    Some(slot) => {
                        let conn = Connection::new(top, None, None, None, None)
                            .adjusted_for(from_sidx, top, sg);
                        sg.insert_edge_at(from_sidx, slot, conn);
                    }
                    None => connect(sg, from_sidx, top),
                },
            }
        }
    }
    for i in 0..seg_ids.len() {
        let next = seg_ids.get(i + 1).copied().unwrap_or(anchor);
        connect(sg, seg_ids[i], next);
    }
}
