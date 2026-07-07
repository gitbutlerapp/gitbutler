//! Phase 3 of gather-then-build: mint the segment graph in its final shape from
//! the commit-graph and the chain plan.

use std::collections::{BTreeMap, HashMap, HashSet};

use gix::reference::Category;

use super::chains::{
    add_advanced_outside_branches, effective_lower_bound, insert_empty_branches,
    insert_empty_workspace_segment,
};
use super::facts::{Facts, facts};
use super::plan::{
    ChainPlan, Float, author_arrangement, chain_plan, debug_assert_arrangement_matches_plan,
};
use super::remotes::{
    add_co_located_remote_empties, add_remote_segments, add_untracked_remote_segments,
    link_remote_to_local, remote_name_in_play, segment_ahead_region,
};
use super::{
    IdMap, IdSet, connect, segment_by_commit, segment_by_commit_excluding, segment_by_ref,
    segment_metadata,
};
use crate::{Commit, CommitGraph, RefInfo, Segment, SegmentIndex, segment_graph::SegmentGraph};

/// Build a segment [`Graph`](crate::Graph) from `cg`.
///
/// Inputs mirror the projection's enrichment: the workspace commit, the target that bounds/integrates,
/// and the local→remote tracking map. `project_meta`/`options` are carried onto the `Graph`.
///
/// This is "gather-then-build": everything is decided as data BEFORE any segment exists, then
/// materialized in one pass. Roughly, in order:
///
/// 1. **Facts** (`facts`) — the boundaries where segments start, which boundary owns each
///    commit, and the tips in materialization order. Pure facts over `cg`; reads no segment.
/// 2. **Lower bound** — the base all chains and the target converge on.
/// 3. **Chain plan** (`chain_plan`) — the NAME each tip's segment gets (some go anonymous so an
///    empty named segment can float above them), decided before any segment is built.
/// 4. **Materialize** (`mint_tip_segments`, `mint_float_placeholders`) — one local segment per
///    tip holding its first-parent commit run, then the planned float placeholders (empty named
///    segments) spliced above the anonymized tips.
/// 5. **Connect** (`connect_parents`) — each segment's bottom commit points at the segments
///    owning its parents.
/// 6. **Chain structure** (`build_chain_structure`) — empty-workspace segment, advanced-outside
///    branches, empty-branch splices. Runs before the remote passes so those link the chain
///    segments at creation.
/// 7. **Remote / target / entrypoint passes** — a remote root segment per local branch whose
///    remote tip is present (`add_remote_segments`); the target's own remote segment when no
///    local tracks it (`surface_target_remote`); regions for an extra (older) target position,
///    an outside checkout, and any explicit tip left uncovered; then the whole-graph sweeps
///    (entrypoint resolution, metadata, ref dedup, worktree annotation).
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "graph_from_commit_graph",
    level = "trace",
    skip_all,
    fields(commits = cg.row_count())
)]
pub(crate) fn graph_from_commit_graph<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    target: Option<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    // Remote names implied by the workspace configuration (push remote, target's remote). Only these
    // remotes' AHEAD regions are traversed; a config-only tracking link keeps its name but its remote's
    // own commits stay out of the graph, matching the walk's traversal reach.
    symbolic_remotes: &[String],
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    // A managed workspace (`workspace_commit` is the gitbutler/workspace octopus merge). When false,
    // `workspace_commit` is just the checked-out tip: no stack/ws-ref/anonymize passes.
    managed: bool,
    // Which worktree (if any) checks out each ref, keyed by ref name — the main worktree `[🌳]` and any
    // linked worktrees `[📁]`. Mirrors the walk's `RefInfo::from_ref` lookup.
    worktree_by_branch: &BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
    meta: &T,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> (crate::Graph, crate::ref_arrangement::RefArrangement) {
    let f = facts(
        cg,
        workspace_commit,
        entrypoint,
        target,
        remote_tracking,
        stack_branches,
        managed,
        meta,
        &project_meta,
        &options,
    );
    // The workspace's lower bound — the base all chains and the (stored/extra) target converge
    // on, extended down to an older target position.
    let ws_lower_bound =
        effective_lower_bound(cg, workspace_commit, target, &project_meta, &options);
    let plan = chain_plan(
        cg,
        &f,
        workspace_commit,
        entrypoint,
        entrypoint_ref.as_ref(),
        target,
        remote_tracking,
        stack_branches,
        ws_lower_bound,
        managed,
        meta,
        project_meta.target_ref.as_ref(),
        symbolic_remotes,
        options.extra_target_commit_id,
    );
    // The ref placement table: authored here, stored on the commit graph, and consumed by the
    // chain-structure pass below. The oracle keeps proving it round-trips the plan it came from.
    let arrangement = author_arrangement(&plan);
    debug_assert_arrangement_matches_plan(&arrangement, &plan);
    let Facts {
        in_set,
        ws_is_managed_merge: _,
        empty_ws_case,
        pinned_commits,
        boundaries,
        entrypoint_forced_boundary: _,
        owner_of,
        tips,
    } = f;

    let mut sg = SegmentGraph::new();
    let seg_of_tip = mint_tip_segments(
        cg,
        &mut sg,
        &tips,
        &in_set,
        &boundaries,
        &plan,
        remote_tracking,
    );
    let placeholder_of = mint_float_placeholders(&mut sg, &plan, remote_tracking);
    connect_parents(
        cg,
        &mut sg,
        &tips,
        workspace_commit,
        &owner_of,
        &seg_of_tip,
        &placeholder_of,
        &plan,
    );

    let (ws_empty_sidx, chain_created) = build_chain_structure(
        cg,
        &mut sg,
        &seg_of_tip,
        &in_set,
        &pinned_commits,
        stack_branches,
        workspace_commit,
        remote_tracking,
        meta,
        project_meta.target_ref.as_ref(),
        empty_ws_case,
        managed,
        &arrangement,
    );

    // Remote segments: for each local segment with a remote-tracking ref whose remote tip is
    // present, create a remote root segment (holding the remote-ahead commits) that connects into
    // the local segment, doubly-linked via siblings. The remote passes historically ran BEFORE the
    // chain structure and keyed on the pre-chain names — the overlay carries exactly that view
    // (materialization names plus the passes' own renames), so the chain reorder cannot change
    // their decisions.
    let pre_chain_names: IdMap<gix::refs::FullName> = plan.base_name_of.clone();
    let claimed_remote_names = claim_remote_names(
        cg,
        &plan,
        &in_set,
        remote_tracking,
        stack_branches,
        symbolic_remotes,
    );
    // Connections from a region into another creator's territory (a run stopped at a claimed
    // remote): recorded during region creation, wired once every creator ran.
    let mut pending_edges: Vec<(SegmentIndex, gix::ObjectId)> = Vec::new();
    // The entrypoint is a planned boundary in every region too: a checkout inside a remote's
    // ahead run starts its own segment at creation, never split out after the fact.
    let region_pinned = {
        let mut p = pinned_commits.clone();
        p.insert(entrypoint);
        p
    };
    add_remote_segments(
        cg,
        &mut sg,
        &seg_of_tip,
        &in_set,
        &owner_of,
        symbolic_remotes,
        stack_branches,
        &region_pinned,
        remote_tracking,
        &pre_chain_names,
        &plan.renames,
        &claimed_remote_names,
        &mut pending_edges,
    );
    add_untracked_remote_segments(
        cg,
        &mut sg,
        remote_tracking,
        &seg_of_tip,
        &in_set,
        &owner_of,
    );
    surface_target_remote(
        cg,
        &mut sg,
        project_meta.target_ref.as_ref(),
        &in_set,
        &owner_of,
        &seg_of_tip,
        &plan,
        remote_tracking,
        &region_pinned,
        &claimed_remote_names,
        &mut pending_edges,
    );
    add_extra_target_region(
        cg,
        &mut sg,
        options.extra_target_commit_id,
        &chain_created,
        &in_set,
        &seg_of_tip,
        &owner_of,
        remote_tracking,
        &region_pinned,
        &claimed_remote_names,
        &mut pending_edges,
    );
    add_outside_entrypoint_region(
        cg,
        &mut sg,
        entrypoint,
        entrypoint_ref.as_ref(),
        &chain_created,
        &in_set,
        &seg_of_tip,
        &owner_of,
        remote_tracking,
        &region_pinned,
        &claimed_remote_names,
        &mut pending_edges,
    );
    cover_explicit_tips(
        cg,
        &mut sg,
        &chain_created,
        &in_set,
        &seg_of_tip,
        &owner_of,
        remote_tracking,
        &region_pinned,
        &claimed_remote_names,
        &mut pending_edges,
    );

    // The target's remote segment may have been created before its LOCAL got a segment (the
    // local can materialize from the extra-target region above) — link them like every other
    // creator does.
    if let Some(tr) = project_meta.target_ref.as_ref()
        && let Some(tr_sidx) = segment_by_ref(&sg, tr)
    {
        let tr = tr.clone();
        link_remote_to_local(&mut sg, tr_sidx, &tr, remote_tracking);
    }

    add_co_located_remote_empties(&mut sg, remote_tracking);
    wire_pending_edges(&mut sg, pending_edges);

    float_remote_named_checkout(
        &mut sg,
        entrypoint,
        entrypoint_ref.as_ref(),
        workspace_commit,
    );
    if managed {
        drop_suppressed_tip_links(&mut sg, &plan, &seg_of_tip);
    }

    let entrypoint_sidx = resolve_entrypoint_segment(
        &mut sg,
        ws_empty_sidx,
        entrypoint,
        entrypoint_ref.as_ref(),
        workspace_commit,
        remote_tracking,
    );
    classify_segment_metadata(&mut sg, meta);
    strip_segment_named_refs(&mut sg);
    annotate_worktrees(&mut sg, worktree_by_branch);

    let entrypoint =
        entrypoint_sidx.map(|sidx| (sidx, crate::EntryPointCommit::AtCommit(entrypoint)));

    // Surface the extra target (an older target position) as an integrated traversal tip. The projection
    // derives `target_commit` from the deepest integrated tip and uses it to extend the workspace base
    // down to it — showing the commits integrated since then, exactly as the walk does. Only when the
    // commit actually made it into a segment — validation requires every tip to be owned by one, and
    // the traversal legitimately never reaches an extra target outside its cut.
    let mut traversal_tips = Vec::new();
    if let Some(extra) = options.extra_target_commit_id
        && segment_by_commit(&sg, extra).is_some()
    {
        traversal_tips
            .push(crate::init::Tip::new(extra).with_role(crate::init::TipRole::TargetRemote));
    }

    let mut graph = crate::Graph {
        inner: sg,
        entrypoint,
        entrypoint_ref,
        project_meta,
        options,
        traversal_tips,
        ..crate::Graph::default()
    };
    // The traversal's hard-limit signal survives the derivation — consumers surface it to the user.
    if cg.hard_limit_hit {
        graph.set_hard_limit_hit();
    }
    (graph, arrangement)
}

/// Create a local segment per tip, holding its first-parent commit run. Names come from the
/// plan: floated and demoted tips start ANONYMOUS (their name never touches the segment); a
/// float's displaced build-time name rides on the tip commit as a passive ref.
fn mint_tip_segments(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    tips: &[gix::ObjectId],
    in_set: &IdSet,
    boundaries: &IdSet,
    plan: &ChainPlan,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> IdMap<SegmentIndex> {
    let is_boundary = |c: gix::ObjectId| boundaries.contains(&c);
    let floated: IdMap<&Float> = plan.floats.iter().map(|fl| (fl.tip, fl)).collect();
    let mut seg_of_tip: IdMap<SegmentIndex> = IdMap::default();
    for &tip in tips {
        let mut commits = commit_run(cg, tip, in_set, &is_boundary);
        let suppressed = floated.contains_key(&tip) || plan.demoted.contains(&tip);
        let named = if suppressed {
            None
        } else {
            plan.base_name_of
                .get(&tip)
                .map(|n| (n.clone(), tip))
                .or_else(|| plan.renames.get(&tip).cloned())
        };
        if let Some(displaced) = floated
            .get(&tip)
            .and_then(|fl| fl.displaced_ref_name.as_ref())
            && let Some(c0) = commits.first_mut()
            && !c0.refs.iter().any(|r| r.ref_name == *displaced)
        {
            c0.refs.push(RefInfo {
                ref_name: displaced.clone(),
                commit_id: Some(tip),
                worktree: None,
            });
            c0.refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
        }
        let ref_info = named.map(|(ref_name, commit_id)| RefInfo {
            ref_name,
            commit_id: Some(commit_id),
            worktree: None,
        });
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let sidx = sg.add_segment(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.segment_mut(sidx).expect("just added").id = sidx;
        seg_of_tip.insert(tip, sidx);
    }
    seg_of_tip
}

/// The planned float placeholders: empty segments carrying the floated names, spliced between
/// the workspace and the now-anonymous shared tips (edges below). Returns each placeholder
/// keyed by the tip it floats above.
fn mint_float_placeholders(
    sg: &mut SegmentGraph,
    plan: &ChainPlan,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> IdMap<SegmentIndex> {
    let mut placeholder_of: IdMap<SegmentIndex> = IdMap::default();
    for float in &plan.floats {
        let sidx = sg.add_segment(Segment {
            id: 0,
            ref_info: Some(RefInfo {
                ref_name: float.name.clone(),
                // Metadata-derived empties are synthetic: no resolved ref tip, like the walk's
                // `branch_segment_from_name_and_meta` without a refs-by-id lookup. Consumers treat
                // a `Some` here as an amendable tip, which must not happen for empty chains.
                commit_id: None,
                worktree: None,
            }),
            remote_tracking_ref_name: remote_tracking.get(&float.name).cloned(),
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits: Vec::new(),
            metadata: None,
            connections: Vec::new(),
        });
        sg.segment_mut(sidx).expect("just added").id = sidx;
        placeholder_of.insert(float.tip, sidx);
    }
    placeholder_of
}

/// Connections: for each segment, its bottom commit's parents point at the segment owning each
/// parent, in first-parent order. The workspace's edge to a FLOATED parent routes through the
/// placeholder instead, and each placeholder connects down into its anonymized shared segment.
#[allow(clippy::too_many_arguments)]
fn connect_parents(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    tips: &[gix::ObjectId],
    workspace_commit: gix::ObjectId,
    owner_of: &IdMap<gix::ObjectId>,
    seg_of_tip: &IdMap<SegmentIndex>,
    placeholder_of: &IdMap<SegmentIndex>,
    plan: &ChainPlan,
) {
    for &tip in tips {
        let src = seg_of_tip[&tip];
        let bottom = sg
            .segment(src)
            .expect("present")
            .commits
            .last()
            .map(|c| c.id)
            .unwrap_or(tip);
        for parent in cg.all_parent_ids(bottom) {
            if let Some(&owner) = owner_of.get(&parent) {
                let dst = if tip == workspace_commit
                    && let Some(&ph) = placeholder_of.get(&parent)
                {
                    ph
                } else {
                    seg_of_tip[&owner]
                };
                connect(sg, src, dst);
            }
        }
    }
    // Placeholder → the anonymized shared segment.
    for float in &plan.floats {
        let (Some(&ph), Some(&tip_sidx)) =
            (placeholder_of.get(&float.tip), seg_of_tip.get(&float.tip))
        else {
            continue;
        };
        connect(sg, ph, tip_sidx);
    }
}

/// The chain STRUCTURE (empty-ws segment, advanced-outside branches, empty-branch splices)
/// precedes the remote passes, which link the chain segments at creation.
///
/// Returns the empty workspace segment (when one was spliced in) and the segments this pass
/// created: the coverage gates (extra target, outside entrypoint, explicit tips) historically
/// evaluated BEFORE any chain existed — they must not be shadowed by chain segments (e.g. an
/// advanced-outside run swallowing the stored target position that the extra-target region
/// must surface).
#[allow(clippy::too_many_arguments)]
fn build_chain_structure<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    seg_of_tip: &IdMap<SegmentIndex>,
    in_set: &IdSet,
    pinned_commits: &IdSet,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    workspace_commit: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
    empty_ws_case: bool,
    managed: bool,
    arrangement: &crate::ref_arrangement::RefArrangement,
) -> (Option<SegmentIndex>, HashSet<SegmentIndex>) {
    let mut ws_empty_sidx = None;
    let before_chains: HashSet<SegmentIndex> = sg.segment_ids().collect();
    if managed {
        if empty_ws_case {
            ws_empty_sidx = insert_empty_workspace_segment(sg, seg_of_tip, cg, workspace_commit);
        }
        add_advanced_outside_branches(
            sg,
            cg,
            in_set,
            stack_branches,
            workspace_commit,
            remote_tracking,
            meta,
            target_ref,
            pinned_commits,
        );
        let ws_sidx = ws_empty_sidx.or_else(|| seg_of_tip.get(&workspace_commit).copied());
        insert_empty_branches(sg, ws_sidx, arrangement, remote_tracking);
    }
    let chain_created: HashSet<SegmentIndex> = sg
        .segment_ids()
        .filter(|sidx| !before_chains.contains(sidx))
        .collect();
    (ws_empty_sidx, chain_created)
}

/// Remote refs some creator will consume as a segment name: the region builder cuts its run
/// at interior remote refs only when unclaimed. Plan-modeled names (`remote_used` covers the
/// walk seeds) plus the ahead-case remotes of EVERY boundary-tip local (`add_remote_segments`
/// regions all of them, mirroring its gates) plus explicit-tip remote names.
fn claim_remote_names(
    cg: &CommitGraph,
    plan: &ChainPlan,
    in_set: &IdSet,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    symbolic_remotes: &[String],
) -> HashSet<gix::refs::FullName> {
    let mut claimed_remote_names: HashSet<gix::refs::FullName> = plan.remote_used.clone();
    claimed_remote_names.extend(plan.base_name_of.values().filter_map(|name| {
        let rt = remote_tracking.get(name)?;
        let rt_tip = cg.commit_by_ref(rt.as_ref())?;
        let is_meta_stack_branch = stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| b == rt);
        (!in_set.contains(&rt_tip)
            && remote_name_in_play(rt, symbolic_remotes)
            && !is_meta_stack_branch)
            .then(|| rt.clone())
    }));
    if cg.explicit_tips {
        claimed_remote_names.extend(cg.traversal_tips.iter().filter_map(|t| {
            t.ref_name
                .clone()
                .filter(|r| r.as_ref().category() == Some(Category::RemoteBranch))
        }));
    }
    claimed_remote_names
}

/// The TARGET remote must surface as a segment even when no local segment tracks it — its local
/// ref may be a mere commit-ref on a stack commit (e.g. `main` on a stack tip the metadata branch
/// names), or absent entirely. In the workspace, the walk names the target's rejoin segment after
/// the target and links it as sibling of the segment owning the local tracking ref's position.
/// Outside it (ahead or fully disjoint history), the target's own commits become a standalone
/// remote segment.
#[allow(clippy::too_many_arguments)]
fn surface_target_remote(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    target_ref: Option<&gix::refs::FullName>,
    in_set: &IdSet,
    owner_of: &IdMap<gix::ObjectId>,
    seg_of_tip: &IdMap<SegmentIndex>,
    plan: &ChainPlan,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    region_pinned: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
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
        // Materialization applied the plan's rename when the target NAMES the (previously
        // anonymous) owner; this pass only adds the sibling link.
        if plan.renames.get(&owner_tip).is_some_and(|(n, _)| n == tr)
            && let Some(owner_sidx) = segment_by_commit(sg, tip)
        {
            // Sibling: the segment whose FIRST commit is the local tracking ref's position.
            let local_sidx = remote_tracking
                .iter()
                .find(|(_, r)| *r == tr)
                .and_then(|(local, _)| cg.commit_by_ref(local.as_ref()))
                .and_then(|lc| {
                    segment_by_commit(sg, lc).filter(|&sidx| {
                        sidx != owner_sidx
                            && sg
                                .segment(sidx)
                                .is_some_and(|s| s.commits.first().is_some_and(|c| c.id == lc))
                    })
                });
            if let Some(local_sidx) = local_sidx
                && let Some(s) = sg.segment_mut(owner_sidx)
            {
                s.sibling_segment_id = Some(local_sidx);
            }
        }
    } else if segment_by_ref(sg, tr).is_none() {
        // The target's own (remote) commits: segment its region like any remote's — split at
        // merges, connect every rejoin (including a merge's second parent) back into the
        // workspace — so the projection can find the common base. No tracking local, no links.
        segment_ahead_region(
            cg,
            sg,
            Some(tr),
            tip,
            in_set,
            seg_of_tip,
            owner_of,
            remote_tracking,
            None,
            region_pinned,
            claimed_remote_names,
            pending_edges,
        );
        // The target's LOCAL tracking branch can sit on the region's tip (a fully disjoint
        // target only reached via the target tip itself). The local owns the commit — remotes
        // never take owned commits — so the local names the segment and the remote becomes an
        // empty segment above it, sibling-linked, exactly like the walk. Target queries then
        // count 0 commits ahead (the remote segment is empty).
        let local_on_tip = remote_tracking
            .iter()
            .find(|(local, r)| *r == tr && cg.commit_by_ref(local.as_ref()) == Some(tip))
            .map(|(local, _)| local.clone());
        if let Some(local) = local_on_tip
            && let Some(owner_sidx) = segment_by_commit(sg, tip)
            && sg.segment(owner_sidx).is_some_and(|s| {
                s.ref_info.as_ref().is_some_and(|ri| &ri.ref_name == tr)
                    && s.commits.first().is_some_and(|c| c.id == tip)
            })
        {
            if let Some(s) = sg.segment_mut(owner_sidx) {
                s.ref_info = Some(RefInfo {
                    ref_name: local,
                    commit_id: Some(tip),
                    worktree: None,
                });
                s.remote_tracking_ref_name = Some(tr.clone());
            }
            let remote_sidx = sg.add_segment(Segment {
                id: 0,
                ref_info: Some(RefInfo {
                    ref_name: tr.clone(),
                    commit_id: Some(tip),
                    worktree: None,
                }),
                remote_tracking_ref_name: None,
                sibling_segment_id: Some(owner_sidx),
                remote_tracking_branch_segment_id: None,
                commits: Vec::new(),
                metadata: None,
                connections: Vec::new(),
            });
            sg.segment_mut(remote_sidx).expect("just added").id = remote_sidx;
            if let Some(s) = sg.segment_mut(owner_sidx) {
                s.remote_tracking_branch_segment_id = Some(remote_sidx);
            }
            connect(sg, remote_sidx, owner_sidx);
        }
    }
}

/// An EXTRA TARGET (an older target position) whose commit isn't part of any region so far — e.g.
/// one below the workspace's own history under a traversal cut — is surfaced like a target's
/// region, so the projection can derive `target_commit` from it.
#[allow(clippy::too_many_arguments)]
fn add_extra_target_region(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    extra_target: Option<gix::ObjectId>,
    chain_created: &HashSet<SegmentIndex>,
    in_set: &IdSet,
    seg_of_tip: &IdMap<SegmentIndex>,
    owner_of: &IdMap<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    region_pinned: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
) {
    if let Some(extra) = extra_target
        && cg.row(extra).is_some()
        && segment_by_commit_excluding(sg, extra, chain_created).is_none()
    {
        segment_ahead_region(
            cg,
            sg,
            None,
            extra,
            in_set,
            seg_of_tip,
            owner_of,
            remote_tracking,
            None,
            region_pinned,
            claimed_remote_names,
            pending_edges,
        );
    }
}

/// The entrypoint itself sits OUTSIDE the workspace (an adhoc checkout in a repository that has
/// a managed one): its history becomes a region segmented like a remote's — split at inner
/// merges, connected where it rejoins the workspace (a boundary via `entrypoint_outside`) — so
/// the graph carries both components like the walk, and operations from an outside checkout
/// still see the workspace. The projection downgrades it to the single-branch view.
#[allow(clippy::too_many_arguments)]
fn add_outside_entrypoint_region(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    chain_created: &HashSet<SegmentIndex>,
    in_set: &IdSet,
    seg_of_tip: &IdMap<SegmentIndex>,
    owner_of: &IdMap<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    region_pinned: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
) {
    if !in_set.contains(&entrypoint)
        && cg.row(entrypoint).is_some()
        && segment_by_commit_excluding(sg, entrypoint, chain_created).is_none()
    {
        segment_ahead_region(
            cg,
            sg,
            entrypoint_ref,
            entrypoint,
            in_set,
            seg_of_tip,
            owner_of,
            remote_tracking,
            None,
            region_pinned,
            claimed_remote_names,
            pending_edges,
        );
    }
}

/// The walk seeds a segment per tip, and validation requires every tip to be owned by one.
/// An EXPLICIT traversal tip still uncovered gets its own region, named by the tip's ref; a
/// covered one whose ref names no segment (e.g. an integrated remote target riding on an
/// in-set commit) gets an EMPTY tip-named segment spliced above its commit's owner — the
/// walk's tip-seeded shape, which reachability-based consumers (upstream integration,
/// divergence classification) depend on.
#[allow(clippy::too_many_arguments)]
fn cover_explicit_tips(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    chain_created: &HashSet<SegmentIndex>,
    in_set: &IdSet,
    seg_of_tip: &IdMap<SegmentIndex>,
    owner_of: &IdMap<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    region_pinned: &IdSet,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
) {
    for t in cg.traversal_tips.iter().filter(|_| cg.explicit_tips) {
        if cg.row(t.id).is_none() {
            continue;
        }
        match segment_by_commit_excluding(sg, t.id, chain_created) {
            None => segment_ahead_region(
                cg,
                sg,
                t.ref_name.as_ref(),
                t.id,
                in_set,
                seg_of_tip,
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
                // Workspace refs are the managed machinery's territory — it names or splices the
                // workspace segment itself (a second one here would violate name uniqueness).
                if but_core::is_workspace_ref_name(ref_name.as_ref()) {
                    continue;
                }
                if segment_by_ref(sg, &ref_name).is_some()
                    || sg.segment(owner_sidx).is_some_and(|s| {
                        s.ref_info
                            .as_ref()
                            .is_some_and(|ri| ri.ref_name == ref_name)
                    })
                {
                    continue;
                }
                // An ANONYMOUS segment starting at the tip takes the tip's name — applied by
                // MATERIALIZATION from the plan's renames; a still-anonymous tip start here
                // means the plan and the build disagree.
                if sg.segment(owner_sidx).is_some_and(|s| {
                    s.commits.first().is_some_and(|c| c.id == t.id) && s.ref_info.is_none()
                }) {
                    debug_assert!(
                        false,
                        "the plan names every anonymous tip-started segment ({ref_name} at {})",
                        t.id
                    );
                    continue;
                }
                let empty_sidx = sg.add_segment(Segment {
                    id: 0,
                    ref_info: Some(RefInfo {
                        ref_name: ref_name.clone(),
                        commit_id: Some(t.id),
                        worktree: None,
                    }),
                    remote_tracking_ref_name: remote_tracking.get(&ref_name).cloned(),
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
                    commits: Vec::new(),
                    metadata: None,
                    connections: Vec::new(),
                });
                sg.segment_mut(empty_sidx).expect("just added").id = empty_sidx;
                connect(sg, empty_sidx, owner_sidx);
            }
        }
    }
}

/// Wire the stopped runs into the segments that own their territory — every creator has run,
/// so the target of each pending connection exists (the owning creator's root, a cut segment,
/// or a mid-run commit of one).
fn wire_pending_edges(sg: &mut SegmentGraph, pending_edges: Vec<(SegmentIndex, gix::ObjectId)>) {
    for (src, parent) in pending_edges {
        let Some(dst) = sg.segment_ids().find(|&sidx| {
            sg.segment(sidx)
                .is_some_and(|s| s.commits.iter().any(|c| c.id == parent))
        }) else {
            continue;
        };
        connect(sg, src, dst);
    }
}

/// A no-ref checkout at a REMOTE-named segment's tip: the walk's anonymous entrypoint tip owns
/// the commits as a local segment — a remote ref never names it — and the remote's machinery
/// re-establishes the name as an EMPTY segment above. Float the name up so the projection sees
/// a detached view, not the remote segment. A LOCAL name stays: the walk names the entrypoint
/// segment after it.
fn float_remote_named_checkout(
    sg: &mut SegmentGraph,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    workspace_commit: gix::ObjectId,
) {
    if entrypoint_ref.is_none()
        && entrypoint != workspace_commit
        && let Some(ep_sidx) = segment_by_commit(sg, entrypoint)
        && sg.segment(ep_sidx).is_some_and(|s| {
            s.ref_info
                .as_ref()
                .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
                && s.commits.first().is_some_and(|c| c.id == entrypoint)
        })
    {
        let (ref_info, rt_name, sibling, rt_seg) = {
            let s = sg.segment_mut(ep_sidx).expect("present");
            (
                s.ref_info.take(),
                s.remote_tracking_ref_name.take(),
                s.sibling_segment_id.take(),
                s.remote_tracking_branch_segment_id.take(),
            )
        };
        let floated = sg.add_segment(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name: rt_name,
            sibling_segment_id: sibling,
            remote_tracking_branch_segment_id: rt_seg,
            commits: Vec::new(),
            metadata: None,
            connections: Vec::new(),
        });
        sg.segment_mut(floated).expect("just added").id = floated;
        // Links and edges aimed at the named segment now belong to its floated name.
        for sidx in sg.segment_ids().collect::<Vec<_>>() {
            if sidx == floated {
                continue;
            }
            if let Some(s) = sg.segment_mut(sidx) {
                if s.sibling_segment_id == Some(ep_sidx) {
                    s.sibling_segment_id = Some(floated);
                }
                if s.remote_tracking_branch_segment_id == Some(ep_sidx) {
                    s.remote_tracking_branch_segment_id = Some(floated);
                }
            }
            sg.retarget_edges(sidx, ep_sidx, floated);
        }
        connect(sg, floated, ep_sidx);
    }
}

/// The remote/target passes link remotes against the plan's effective names — the
/// floated/demoted name's segment carries the links, so the suppressed tip drops its own.
fn drop_suppressed_tip_links(
    sg: &mut SegmentGraph,
    plan: &ChainPlan,
    seg_of_tip: &IdMap<SegmentIndex>,
) {
    for tip in plan
        .floats
        .iter()
        .map(|fl| fl.tip)
        .chain(plan.demoted.iter().copied())
    {
        if let Some(s) = seg_of_tip.get(&tip).and_then(|&sidx| sg.segment_mut(sidx)) {
            s.remote_tracking_ref_name = None;
            s.remote_tracking_branch_segment_id = None;
        }
    }
}

/// A checkout inside a stack (from_commit_traversal) splits the enclosing segment so the entrypoint
/// begins its own segment — there is always a segment starting at the entrypoint.
fn resolve_entrypoint_segment(
    sg: &mut SegmentGraph,
    ws_empty_sidx: Option<SegmentIndex>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    workspace_commit: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Option<SegmentIndex> {
    if let (Some(ws_seg), None, true) = (
        ws_empty_sidx,
        entrypoint_ref,
        entrypoint == workspace_commit,
    ) {
        // from_head into a co-located workspace: the entrypoint is the empty workspace segment.
        // Only when the checkout IS the workspace position — a ref-less checkout elsewhere (e.g.
        // an unapply preview with the branch dropped) must not claim the workspace segment while
        // remembering a commit the graph doesn't hold.
        Some(ws_seg)
    } else if let Some(named) = entrypoint_ref.and_then(|r| segment_by_ref(sg, r)) {
        // The checked-out ref already names a segment — including an EMPTY one spliced in for a
        // virtual stack branch resting on the workspace base. That segment is the entrypoint, not
        // the segment owning the commit it points to.
        Some(named)
    } else {
        name_entrypoint_segment(sg, entrypoint, entrypoint_ref, remote_tracking)
    }
}

/// Classify each named segment by its ref's metadata: the workspace ref → Workspace, a tracked
/// branch → Branch, others → None. Matches the walk's `extract_local_branch_metadata`.
fn classify_segment_metadata<T: but_core::RefMetadata>(sg: &mut SegmentGraph, meta: &T) {
    for sidx in sg.segment_ids().collect::<Vec<_>>() {
        let ref_name = sg
            .segment(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .map(|ri| ri.ref_name.clone());
        if let Some(ref_name) = ref_name {
            let md = segment_metadata(ref_name.as_ref(), meta);
            if let Some(s) = sg.segment_mut(sidx) {
                s.metadata = md;
            }
        }
    }
}

/// A ref that NAMES a segment (or is a segment's remote-tracking ref) lives on that segment, so it is
/// removed from every commit's own ref list — including an empty branch's ref that sits on another
/// segment's commit (the walk does the same, avoiding showing it twice).
fn strip_segment_named_refs(sg: &mut SegmentGraph) {
    let segment_names: HashSet<gix::refs::FullName> = sg
        .segment_ids()
        .flat_map(|sidx| {
            sg.segment(sidx)
                .map(|s| {
                    s.ref_info
                        .as_ref()
                        .map(|ri| ri.ref_name.clone())
                        .into_iter()
                        .chain(s.remote_tracking_ref_name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();
    for sidx in sg.segment_ids().collect::<Vec<_>>() {
        if let Some(s) = sg.segment_mut(sidx) {
            for commit in &mut s.commits {
                // Also drop remote-tracking refs: a remote is only ever shown as its own segment, never
                // annotated on a commit.
                commit.refs.retain(|ri| {
                    !segment_names.contains(&ri.ref_name)
                        && ri.ref_name.as_ref().category() != Some(Category::RemoteBranch)
                });
            }
        }
    }
}

/// Annotate every ref with the worktree that checks it out — the main worktree `[🌳]` (whatever
/// ref HEAD actually points at, including the workspace ref) and linked worktrees `[📁]`. Keyed
/// by ref name, mirroring the walk's `RefInfo::from_ref`. No hardcoded HEAD assumption: marking
/// the workspace-commit segment unconditionally put `[🌳]` on a stack branch when HEAD was on
/// the workspace ref, and vice versa.
fn annotate_worktrees(
    sg: &mut SegmentGraph,
    worktree_by_branch: &BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
) {
    let annotate = |ri: &mut RefInfo| {
        if ri.worktree.is_none()
            && let Some(wt) = worktree_by_branch.get(&ri.ref_name).and_then(|w| w.first())
        {
            ri.worktree = Some(wt.clone());
        }
    };
    for sidx in sg.segment_ids().collect::<Vec<_>>() {
        let Some(s) = sg.segment_mut(sidx) else {
            continue;
        };
        if let Some(ri) = s.ref_info.as_mut() {
            annotate(ri);
        }
        for commit in &mut s.commits {
            for ri in &mut commit.refs {
                annotate(ri);
            }
        }
    }
}

/// The segment starting at the `entrypoint` commit — which exists by construction: the
/// entrypoint is a planned boundary in materialization and in every region, so no commit run
/// ever contains it mid-run. A checked-out `entrypoint_ref` names it (validation requires it):
/// an anonymous segment takes the name directly; one already named by ANOTHER ref keeps its
/// commits and the entrypoint ref becomes an empty segment spliced in above, like the walk's.
fn name_entrypoint_segment(
    sg: &mut SegmentGraph,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Option<SegmentIndex> {
    let (sidx, pos) = sg.segment_ids().find_map(|sidx| {
        sg.segment(sidx)
            .and_then(|s| s.commits.iter().position(|c| c.id == entrypoint))
            .map(|pos| (sidx, pos))
    })?;
    debug_assert_eq!(
        pos, 0,
        "the entrypoint {entrypoint} is a planned boundary everywhere, yet sits mid-run in {sidx}"
    );
    if pos != 0 {
        return None;
    }
    if let Some(ep_ref) = entrypoint_ref {
        let current = sg
            .segment(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .map(|ri| ri.ref_name.clone());
        match current {
            None => {
                if let Some(s) = sg.segment_mut(sidx) {
                    s.ref_info = Some(RefInfo {
                        ref_name: ep_ref.clone(),
                        commit_id: Some(entrypoint),
                        worktree: None,
                    });
                    s.remote_tracking_ref_name = remote_tracking.get(ep_ref).cloned();
                }
            }
            Some(existing) if existing != *ep_ref => {
                let empty = sg.add_segment(Segment {
                    id: 0,
                    ref_info: Some(RefInfo {
                        ref_name: ep_ref.clone(),
                        commit_id: Some(entrypoint),
                        worktree: None,
                    }),
                    remote_tracking_ref_name: remote_tracking.get(ep_ref).cloned(),
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
                    commits: Vec::new(),
                    connections: Vec::new(),
                    metadata: None,
                });
                sg.segment_mut(empty).expect("just added").id = empty;
                // Incoming edges now route through the entrypoint's empty segment.
                for other in sg.segment_ids().collect::<Vec<_>>() {
                    if other == empty {
                        continue;
                    }
                    sg.retarget_edges(other, sidx, empty);
                }
                connect(sg, empty, sidx);
                return Some(empty);
            }
            Some(_) => {}
        }
    }
    Some(sidx)
}

/// The first-parent commit run owned by `tip`: `tip` and each first-parent descendant-in-history until
/// the next boundary (exclusive) or the set edge.
pub(super) fn commit_run(
    cg: &CommitGraph,
    tip: gix::ObjectId,
    in_set: &IdSet,
    is_boundary: &impl Fn(gix::ObjectId) -> bool,
) -> Vec<Commit> {
    commit_run_ids(cg, tip, in_set, is_boundary)
        .into_iter()
        .filter_map(|c| cg.row(c).map(|row| row.commit.clone()))
        .collect()
}

/// [`commit_run`] as ids only — no commit payload cloned.
pub(super) fn commit_run_ids(
    cg: &CommitGraph,
    tip: gix::ObjectId,
    in_set: &IdSet,
    is_boundary: &impl Fn(gix::ObjectId) -> bool,
) -> Vec<gix::ObjectId> {
    let mut out = Vec::new();
    let mut id = Some(tip);
    while let Some(c) = id {
        if !in_set.contains(&c) {
            break;
        }
        if c != tip && is_boundary(c) {
            break;
        }
        out.push(c);
        id = cg.first_parent(c).filter(|p| in_set.contains(p));
    }
    out
}
