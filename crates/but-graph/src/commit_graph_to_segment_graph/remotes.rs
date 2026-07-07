//! The remote passes: link remote-tracking segments to their local counterparts
//! at creation time.

use std::collections::{HashMap, HashSet};

use gix::reference::Category;

use super::materialize::commit_run;
use super::{IdMap, IdSet, connect, is_plain_local_branch, segment_by_ref};
use crate::{CommitGraph, RefInfo, Segment, SegmentIndex, segment_graph::SegmentGraph};

/// Link a just-created remote-named segment to the local segment named by its tracking
/// counterpart: the remote's sibling points at the local, and the local carries the remote's
/// name and segment id. A no-op when no such local exists (e.g. the local ref only rides a
/// commit).
pub(super) fn link_remote_to_local(
    sg: &mut SegmentGraph,
    remote_sidx: SegmentIndex,
    remote_ref: &gix::refs::FullName,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let Some(local_name) = remote_tracking
        .iter()
        .find_map(|(l, r)| (r == remote_ref).then_some(l))
    else {
        return;
    };
    let Some(local_sidx) = segment_by_ref(sg, local_name) else {
        return;
    };
    if let Some(s) = sg.segment_mut(remote_sidx) {
        s.sibling_segment_id = Some(local_sidx);
    }
    if let Some(s) = sg.segment_mut(local_sidx) {
        s.remote_tracking_ref_name = Some(remote_ref.clone());
        s.remote_tracking_branch_segment_id = Some(remote_sidx);
    }
}

#[expect(clippy::too_many_arguments)]
pub(super) fn add_remote_segments(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    seg_of_tip: &IdMap<SegmentIndex>,
    in_set: &IdSet,
    owner_of: &IdMap<gix::ObjectId>,
    symbolic_remotes: &[String],
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    pinned_commits: &IdSet,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    pre_chain_names: &IdMap<gix::refs::FullName>,
    renames: &IdMap<(gix::refs::FullName, gix::ObjectId)>,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
) {
    // Locals are keyed on the PRE-CHAIN names: this pass historically ran before the chain shape
    // was applied and saw every build-time name. The LINKS however belong to whichever segment
    // finally carries the name — a chain placeholder, a spliced empty, a group-named anchor —
    // which exists already (chains precede this pass), so no reconciliation is needed after.
    let mut locals: Vec<(SegmentIndex, gix::refs::FullName, gix::ObjectId)> = seg_of_tip
        .iter()
        .filter_map(|(&tip, &sidx)| {
            let name = pre_chain_names.get(&tip)?;
            let rt = remote_tracking.get(name).cloned()?;
            let link_sidx = segment_by_ref(sg, name).unwrap_or(sidx);
            Some((link_sidx, rt, tip))
        })
        .collect();
    // Deterministic remote-segment ids: `seg_of_tip` is a HashMap, its order varies per process.
    locals.sort_by_key(|&(sidx, ..)| sidx);
    for (local_sidx, remote_ref, _local_tip) in locals {
        let Some(remote_tip) = cg.commit_by_ref(remote_ref.as_ref()) else {
            continue;
        };
        // The remote points BEHIND/at an in-set commit: it names that commit's segment rather than
        // forming its own root. If the segment is anonymous, the remote ref names it directly; if it is
        // already named (e.g. the target `main`), a separate empty remote root points into it.
        if in_set.contains(&remote_tip) {
            let owner = owner_of.get(&remote_tip).copied().unwrap_or(remote_tip);
            let owner_sidx = seg_of_tip[&owner];
            // Materialization applied the plan's rename when this remote NAMES the (previously
            // anonymous) owner; this pass only adds the links.
            let named_by_this = renames
                .get(&owner)
                .is_some_and(|(name, _)| name == &remote_ref);
            if named_by_this {
                if let Some(s) = sg.segment_mut(owner_sidx) {
                    s.sibling_segment_id = Some(local_sidx);
                }
                sg.segment_mut(local_sidx)
                    .expect("present")
                    .remote_tracking_branch_segment_id = Some(owner_sidx);
            } else {
                let remote_sidx = add_empty_remote_root(sg, &remote_ref, remote_tip, local_sidx);
                connect(sg, remote_sidx, owner_sidx);
            }
            continue;
        }

        // The remote is AHEAD: segment its ahead region like the local graph (split at merges and
        // second-parent branches), not as one flat first-parent run. Only for remotes the workspace
        // configuration implies (target/push remote, or a git-configured tracking branch) — and never
        // when the remote ref is ITSELF a workspace-metadata stack branch: then it lives in the
        // workspace as a stack, its commits are the user's own, not an upstream.
        let in_play = remote_name_in_play(&remote_ref, symbolic_remotes);
        let is_metadata_stack_branch = stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| *b == remote_ref);
        if !in_play || is_metadata_stack_branch {
            continue;
        }
        segment_ahead_region(
            cg,
            sg,
            Some(&remote_ref),
            remote_tip,
            in_set,
            seg_of_tip,
            owner_of,
            remote_tracking,
            Some(local_sidx),
            pinned_commits,
            claimed_remote_names,
            pending_edges,
        );
    }
}

/// A region's AHEAD set (commits reachable from its tip that are not in-set) with the shape
/// data both writers segment it by: merges' first parents and the child fan-out.
pub(super) struct AheadRegion {
    pub(super) set: IdSet,
    merge_first_parents: IdSet,
    children: IdMap<Vec<gix::ObjectId>>,
}

impl AheadRegion {
    pub(super) fn compute(cg: &CommitGraph, tip: gix::ObjectId, in_set: &IdSet) -> Self {
        let mut set = IdSet::default();
        let mut stack = vec![tip];
        while let Some(id) = stack.pop() {
            if in_set.contains(&id) || !set.insert(id) {
                continue;
            }
            stack.extend(cg.all_parent_ids(id));
        }
        let mut children: IdMap<Vec<gix::ObjectId>> = IdMap::default();
        for &c in &set {
            for p in cg.all_parent_ids(c) {
                if set.contains(&p) {
                    children.entry(p).or_default().push(c);
                }
            }
        }
        let merge_first_parents: IdSet = set
            .iter()
            .filter(|&&c| cg.all_parent_ids(c).len() > 1)
            .filter_map(|&c| cg.first_parent(c))
            .filter(|p| set.contains(p))
            .collect();
        AheadRegion {
            set,
            merge_first_parents,
            children,
        }
    }

    /// The region's SHAPE boundaries, mirroring local segmentation: the tip, pinned commits,
    /// merges and their first parents, plain-local-branch carriers, and fan-out/second-parent
    /// joints.
    pub(super) fn is_shape_boundary(
        &self,
        cg: &CommitGraph,
        tip: gix::ObjectId,
        pinned_commits: &IdSet,
        c: gix::ObjectId,
    ) -> bool {
        c == tip
            || pinned_commits.contains(&c)
            || cg.all_parent_ids(c).len() > 1
            || self.merge_first_parents.contains(&c)
            || cg.refs_at(c).iter().any(is_plain_local_branch)
            || {
                let kids = self.children.get(&c).map(Vec::as_slice).unwrap_or_default();
                kids.len() > 1
                    || kids
                        .iter()
                        .any(|&k| cg.first_parent(k) != Some(c) && self.set.contains(&k))
            }
    }
}

/// The region's segment tips in minting order: the region tip first, then descending
/// generation, then id — deterministic even though the ahead set is a hash set.
pub(super) fn region_tips(
    cg: &CommitGraph,
    region: &AheadRegion,
    region_tip: gix::ObjectId,
    is_boundary: &impl Fn(gix::ObjectId) -> bool,
) -> Vec<gix::ObjectId> {
    let mut tips: Vec<gix::ObjectId> = region
        .set
        .iter()
        .copied()
        .filter(|&c| is_boundary(c))
        .collect();
    tips.sort_by_cached_key(|&t| {
        (
            t != region_tip,
            std::cmp::Reverse(cg.row(t).map(|n| n.generation).unwrap_or(0)),
            t,
        )
    });
    tips
}

/// The unique plain local branch at `c`, if any — the name fallback for region roots and
/// interior boundaries; ambiguity yields `None`.
pub(super) fn unique_plain_local(
    cg: &CommitGraph,
    c: gix::ObjectId,
) -> Option<gix::refs::FullName> {
    let mut it = cg.refs_at(c).into_iter().filter(is_plain_local_branch);
    it.next().filter(|_| it.next().is_none())
}

/// Segment a remote's AHEAD region (commits reachable from `remote_tip` that are not in-set) the same
/// way the local graph is segmented — splitting at merges and their second-parent branches — instead
/// of collapsing it into one flat first-parent run. The tip segment is named `remote_ref` (sibling
/// `local_sidx`); interior merges and second-parent branches become their own anonymous remote
/// segments. Bottom-of-segment parents connect to the owning ahead segment, or to the local segment
/// where the region rejoins the graph.
#[allow(clippy::too_many_arguments)]
pub(super) fn segment_ahead_region(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    // `None` for a bare EXTRA TARGET commit id — the tip segment is then named by the unique
    // plain local branch on it, if any.
    remote_ref: Option<&gix::refs::FullName>,
    remote_tip: gix::ObjectId,
    in_set: &IdSet,
    seg_of_tip: &IdMap<SegmentIndex>,
    owner_of: &IdMap<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    // The local segment tracking this remote, if any — a TARGET without a tracking local segment
    // builds the same region without the sibling/remote-tracking links.
    local_sidx: Option<SegmentIndex>,
    // Stored/extra target positions must start their own segment even inside a remote's ahead
    // region — the projection's `TargetCommit::from_commit` ignores one that sits mid-segment,
    // silently disabling integration checks against it.
    pinned_commits: &IdSet,
    // Remote refs some creator will consume as a segment name (tracked roots, untracked
    // surfacing, the target, explicit tips): an interior remote ref cuts the root run only
    // when unclaimed — a claimed one STOPS it (that creator's region owns the territory).
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    // Connections into another creator's region, recorded as (source segment, parent commit)
    // and wired by the caller once every creator ran — the target segment may not exist yet.
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
) {
    let region = AheadRegion::compute(cg, remote_tip, in_set);
    let ahead_set = &region.set;
    let is_boundary =
        |c: gix::ObjectId| region.is_shape_boundary(cg, remote_tip, pinned_commits, c);

    // Remote refs riding non-boundary commits of a remote-named root's run resolve at creation
    // (formerly the post-hoc surgery of `split_remote_interior_refs`/`split_stacked_remotes`):
    // an UNCLAIMED ref cuts the run — the commit starts its own segment named by that ref; a
    // CLAIMED ref (another creator's root, built or not) STOPS the run — its territory belongs
    // to that creator's region, and the connection into it is wired once every creator ran.
    let root_is_remote =
        remote_ref.is_some_and(|r| r.as_ref().category() == Some(Category::RemoteBranch));
    let mut interior_cuts: IdMap<gix::refs::FullName> = IdMap::default();
    let mut stop: Option<gix::ObjectId> = None;
    if root_is_remote {
        let existing_remote_tip = |c: gix::ObjectId| {
            sg.segment_ids().any(|sidx| {
                sg.segment(sidx).is_some_and(|s| {
                    s.commits.first().is_some_and(|f| f.id == c)
                        && s.ref_info.as_ref().is_some_and(|ri| {
                            ri.ref_name.as_ref().category() == Some(Category::RemoteBranch)
                        })
                })
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
                    && segment_by_ref(sg, r).is_none()
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
    let mut ahead_seg: IdMap<SegmentIndex> = IdMap::default();
    let mut reused: IdSet = IdSet::default();
    for &tip in &tips {
        // The stop commit is another creator's root: this region neither mints nor owns it —
        // the connection into that segment is a pending edge resolved after every creator ran.
        if stop == Some(tip) {
            continue;
        }
        let commits = commit_run(cg, tip, ahead_set, &is_boundary);
        for c in &commits {
            ahead_owner.insert(c.id, tip);
        }
        let is_root = tip == remote_tip;
        // Overlapping regions can split at the same boundary (two stacked remotes above `main`):
        // a segment starting at this commit may already exist. Reuse it rather than minting a
        // dangling twin. Roots keep their own identity (their name and sibling links belong to
        // THIS region's ref).
        if !is_root
            && let Some(existing) = sg.segment_ids().find(|&sidx| {
                sg.segment(sidx)
                    .is_some_and(|s| s.commits.first().is_some_and(|c| c.id == tip))
            })
        {
            ahead_seg.insert(tip, existing);
            reused.insert(tip);
            continue;
        }
        let root_name = || {
            remote_ref
                .cloned()
                .or_else(|| unique_plain_local(cg, remote_tip))
        };
        // Interior segments are named by the cutting remote ref, else by the unique plain local
        // branch at their boundary, like the local graph's ref-driven segmentation; ambiguity
        // keeps them anonymous.
        let interior_name = || {
            interior_cuts
                .get(&tip)
                .cloned()
                .or_else(|| unique_plain_local(cg, tip))
        };
        let ref_info = if is_root {
            root_name().map(|ref_name| RefInfo {
                ref_name,
                commit_id: Some(remote_tip),
                worktree: None,
            })
        } else {
            interior_name().map(|ref_name| RefInfo {
                ref_name,
                commit_id: Some(tip),
                worktree: None,
            })
        };
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .filter(|ri| is_plain_local_branch(&ri.ref_name))
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let sidx = sg.add_segment(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name,
            sibling_segment_id: if is_root { local_sidx } else { None },
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.segment_mut(sidx).expect("just added").id = sidx;
        ahead_seg.insert(tip, sidx);
        if is_root && let Some(local_sidx) = local_sidx {
            sg.segment_mut(local_sidx)
                .expect("present")
                .remote_tracking_branch_segment_id = Some(sidx);
        }
        // A cut segment is remote-named: link it to its tracking local like every remote creator.
        if let Some(cut_ref) = interior_cuts.get(&tip) {
            let cut_ref = cut_ref.clone();
            link_remote_to_local(sg, sidx, &cut_ref, remote_tracking);
        }
    }

    for &tip in &tips {
        // A reused segment already carries its own outgoing connections; the stop commit was
        // never minted here.
        if reused.contains(&tip) || stop == Some(tip) {
            continue;
        }
        let src = ahead_seg[&tip];
        let bottom = sg
            .segment(src)
            .and_then(|s| s.commits.last().map(|c| c.id))
            .unwrap_or(tip);
        for parent in cg.all_parent_ids(bottom) {
            let dst = if ahead_set.contains(&parent) {
                ahead_owner
                    .get(&parent)
                    .and_then(|o| ahead_seg.get(o))
                    .copied()
            } else {
                owner_of
                    .get(&parent)
                    .and_then(|o| seg_of_tip.get(o))
                    .copied()
            };
            if let Some(dst) = dst {
                connect(sg, src, dst);
            } else if ahead_set.contains(&parent) {
                // The parent is beyond the stop: another creator's region owns it, and its
                // segment may not exist yet — wire once every creator ran.
                pending_edges.push((src, parent));
            }
        }
    }
}

/// Create segments for remote-tracking refs that no local segment claimed (untracked/orphan remotes,
/// e.g. `origin/C` pointing at an anonymous commit). Each becomes an empty root connecting to the
/// segment owning its tip, with no sibling.
pub(super) fn add_untracked_remote_segments(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    seg_of_tip: &IdMap<SegmentIndex>,
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
        if segment_by_ref(sg, &r).is_some() {
            continue;
        }
        let Some(tip) = cg.commit_by_ref(r.as_ref()) else {
            continue;
        };
        // Only surface a remote whose LOCAL counterpart actually sits on the same commit (e.g.
        // `C`/`origin/C` on an ambiguous tip). An ORPHAN remote (`origin/A` with no local `A`)
        // has no chain to pair with and is deliberately invisible: the traversal never walks it
        // (remotes are queued off encountered LOCAL refs only), and a metadata branch whose local
        // ref is missing is skipped at seeding. When the local ref (re)appears — e.g. an anonymous
        // segment renamed back to `A` — the next build pairs the remote again automatically.
        // Probed 2026-07-03: surfacing orphans leaks into apply/ad-hoc behavior, not just display.
        // `remote_tracking` maps every remote to a local name, so the discriminator is whether
        // that local ref really exists here.
        let has_local_counterpart = cg
            .refs_at(tip)
            .iter()
            .any(|l| remote_tracking.get(l) == Some(&r));
        if !has_local_counterpart {
            continue;
        }
        // Only the behind/in-set case for now: an empty root into the segment owning the tip.
        if in_set.contains(&tip)
            && let Some(&owner) = owner_of.get(&tip)
            && let Some(&owner_sidx) = seg_of_tip.get(&owner)
        {
            let remote_sidx = sg.add_segment(Segment {
                id: 0,
                ref_info: Some(RefInfo {
                    ref_name: r.clone(),
                    commit_id: Some(tip),
                    worktree: None,
                }),
                remote_tracking_ref_name: None,
                sibling_segment_id: None,
                remote_tracking_branch_segment_id: None,
                commits: Vec::new(),
                metadata: None,
                connections: Vec::new(),
            });
            sg.segment_mut(remote_sidx).expect("just added").id = remote_sidx;
            connect(sg, remote_sidx, owner_sidx);
            link_remote_to_local(sg, remote_sidx, &r, remote_tracking);
        }
    }
}

/// Several remote refs can share ONE commit (e.g. `origin/A`+`origin/B`+`origin/C` after squash
/// merges rewrote their locals) — the first named the commit-holding remote segment; every other
/// becomes an EMPTY segment pointing at it, so each remote ref resolves to a segment. A pure
/// creator over final names: adds empties + links, mutates nothing existing.
pub(super) fn add_co_located_remote_empties(
    sg: &mut SegmentGraph,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let is_remote = |sg: &SegmentGraph, sidx: SegmentIndex| {
        sg.segment(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
    };
    for sidx in sg.segment_ids().collect::<Vec<_>>() {
        if !is_remote(sg, sidx) {
            continue;
        }
        let Some(first) = sg.segment(sidx).and_then(|s| s.commits.first().cloned()) else {
            continue;
        };
        for ri in &first.refs {
            if ri.ref_name.as_ref().category() != Some(Category::RemoteBranch)
                || segment_by_ref(sg, &ri.ref_name).is_some()
            {
                continue;
            }
            let empty = sg.add_segment(Segment {
                id: 0,
                ref_info: Some(RefInfo {
                    ref_name: ri.ref_name.clone(),
                    commit_id: Some(first.id),
                    worktree: None,
                }),
                remote_tracking_ref_name: None,
                sibling_segment_id: None,
                remote_tracking_branch_segment_id: None,
                commits: Vec::new(),
                metadata: None,
                connections: Vec::new(),
            });
            sg.segment_mut(empty).expect("just added").id = empty;
            connect(sg, empty, sidx);
            let name = ri.ref_name.clone();
            link_remote_to_local(sg, empty, &name, remote_tracking);
        }
    }
}

/// Create an empty remote root segment named `remote_ref`, sibling-linked to `local_sidx` (and set the
/// local's `remote_tracking_branch_segment_id`).
pub(super) fn add_empty_remote_root(
    sg: &mut SegmentGraph,
    remote_ref: &gix::refs::FullName,
    remote_tip: gix::ObjectId,
    local_sidx: SegmentIndex,
) -> SegmentIndex {
    let remote_sidx = sg.add_segment(Segment {
        id: 0,
        ref_info: Some(RefInfo {
            ref_name: remote_ref.clone(),
            commit_id: Some(remote_tip),
            worktree: None,
        }),
        remote_tracking_ref_name: None,
        sibling_segment_id: Some(local_sidx),
        remote_tracking_branch_segment_id: None,
        commits: Vec::new(),
        metadata: None,
        connections: Vec::new(),
    });
    sg.segment_mut(remote_sidx).expect("just added").id = remote_sidx;
    sg.segment_mut(local_sidx)
        .expect("present")
        .remote_tracking_branch_segment_id = Some(remote_sidx);
    remote_sidx
}

/// Does the segment name a remote-tracking branch?
pub(super) fn is_remote_segment(sg: &SegmentGraph, sidx: SegmentIndex) -> bool {
    sg.segment(sidx)
        .and_then(|s| s.ref_info.as_ref())
        .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
}

/// Is `remote_ref` on a remote the workspace configuration implies (target/push remote, or a
/// git-configured tracking branch)? Only such remotes' ahead regions are traversed.
pub(super) fn remote_name_in_play(
    remote_ref: &gix::refs::FullName,
    symbolic_remotes: &[String],
) -> bool {
    remote_ref
        .as_bstr()
        .strip_prefix(b"refs/remotes/".as_ref())
        .is_some_and(|rest| {
            symbolic_remotes.iter().any(|r| {
                rest.strip_prefix(r.as_bytes())
                    .is_some_and(|s| s.first() == Some(&b'/'))
            })
        })
}

/// Local branch -> its remote-tracking branch, mirroring the walk's
/// `lookup_remote_tracking_branch_or_deduce_it`, plus the SYMBOLIC remote names in play:
/// 1. A branch CONFIGURED in git (`branch.<name>.remote`/`merge`) tracks that remote branch.
/// 2. Otherwise the relationship is deduced by name (`refs/remotes/<remote>/<X>` for `refs/heads/<X>`),
///    but ONLY against remotes the workspace configuration implies — the `push_remote` (highest
///    priority: "the push-remote overrides the remote we use for listing, even if a fetch remote is
///    available"), then the remote of the configured `target_ref`. A workspace with neither deduces
///    NO name-based relationships at all.
///
/// The returned symbolic names also gate which remotes' AHEAD regions the graph traverses — a
/// config-only tracking link keeps its name, but its remote's own commits stay out of the graph,
/// matching what the walk's traversal reaches.
pub(crate) fn remote_tracking_from_repository(
    repo: &gix::Repository,
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<(
    HashMap<gix::refs::FullName, gix::refs::FullName>,
    Vec<String>,
)> {
    let mut remotes: Vec<String> = Vec::new();
    if let Some(push_remote) = project_meta.push_remote.as_deref() {
        remotes.push(push_remote.to_string());
    }
    if let Some(target_ref) = project_meta.target_ref.as_ref()
        && let Some((remote, _short)) =
            but_core::extract_remote_name_and_short_name(target_ref.as_ref(), &repo.remote_names())
        && !remotes.contains(&remote)
    {
        remotes.push(remote);
    }

    let remote_refs: Vec<gix::refs::FullName> = repo
        .references()?
        .all()?
        .filter_map(Result::ok)
        .filter(|r| r.name().as_bstr().starts_with(b"refs/remotes/"))
        .map(|r| r.name().to_owned())
        .collect();
    let mut map = HashMap::new();
    // Name-deduction against the symbolic remotes.
    for remote in &remotes {
        let prefix = format!("refs/remotes/{remote}/");
        for name in &remote_refs {
            if let Some(short) = name.as_bstr().strip_prefix(prefix.as_bytes()) {
                let local = format!("refs/heads/{}", String::from_utf8_lossy(short));
                if let Ok(local_ref) = gix::refs::FullName::try_from(local) {
                    // The first (highest-priority) remote to claim a local branch wins.
                    map.entry(local_ref).or_insert_with(|| name.clone());
                }
            }
        }
    }
    // Git-configured tracking branches win over name-deduction.
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let local = reference.name().to_owned();
        // The configured NAME counts even when the remote ref does not exist (yet) — the link is
        // name-only then, and passes that need the remote's commits skip unresolvable refs anyway.
        if let Some(Ok(rt)) =
            repo.branch_remote_tracking_ref_name(local.as_ref(), gix::remote::Direction::Fetch)
        {
            let rt = rt.into_owned();
            // The walk also traverses the remotes of git-configured tracking branches — their remote
            // names join the eligibility set.
            let rest = &rt.as_bstr()[b"refs/remotes/".len()..];
            if let Some(slash) = rest.iter().position(|&b| b == b'/') {
                let remote = String::from_utf8_lossy(&rest[..slash]).into_owned();
                if !remotes.contains(&remote) {
                    remotes.push(remote);
                }
            }
            map.insert(local, rt);
        }
    }
    // A remote tracks ONE local: a git-CONFIGURED binding evicts a name-deduced pair for the same
    // remote (e.g. `base-of-A` configured to track `origin/A` after `A` was rebased away from it —
    // `A` no longer tracks anything).
    let mut config_bound: HashMap<gix::refs::FullName, gix::refs::FullName> = HashMap::new();
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let local = reference.name().to_owned();
        if let Some(Ok(rt)) =
            repo.branch_remote_tracking_ref_name(local.as_ref(), gix::remote::Direction::Fetch)
        {
            config_bound.insert(rt.into_owned(), local);
        }
    }
    map.retain(|local, rt| {
        config_bound
            .get(rt)
            .is_none_or(|config_local| config_local == local)
    });
    Ok((map, remotes))
}
