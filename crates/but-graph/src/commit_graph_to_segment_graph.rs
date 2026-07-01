//! SPIKE (commit-graph-experiment): build the segment [`Graph`] from a [`CommitGraph`] — Route B
//! toward deleting the segment graph. Rather than reproduce the projection's simplified stacks, this
//! reconstructs the FULL segment graph (workspace / branch / anonymous / target / remote segments,
//! their first-parent connections, generations, and remote↔local sibling links) so that everything
//! downstream (projection, renderer, consumers) is unchanged.
//!
//! Verified structurally via `graph_structure` (a commit-id-keyed fingerprint) rather than by segment
//! index, since the id numbering necessarily differs from the walk's. First milestone: the clean linear
//! `single-stack` case (each commit its own segment + a co-located remote root).

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use bstr::ByteSlice;
use gix::reference::Category;

use crate::{
    Commit, CommitGraph, RefInfo, Segment, SegmentIndex,
    segment_graph::{Connection, SegmentGraph},
};

/// Build a segment [`Graph`](crate::Graph) from `cg`.
///
/// Inputs mirror the projection's enrichment: the workspace commit, the target that bounds/integrates,
/// and the local→remote tracking map. `project_meta`/`options` are carried onto the `Graph`.
pub fn graph_from_commit_graph(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    target: Option<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> crate::Graph {
    // The commit set the LOCAL segments span: everything reachable from the workspace commit. The
    // integrated trunk below the base is reachable through it; remote-ahead commits are NOT (they hang
    // off remote tips only) and become their own remote segments.
    let _ = target;
    let in_set: HashSet<gix::ObjectId> = ancestors(cg, workspace_commit);

    // In-set children per commit, to detect branch points (a commit reached by >1 child).
    let mut children: HashMap<gix::ObjectId, Vec<gix::ObjectId>> = HashMap::new();
    for &c in &in_set {
        for p in cg.all_parent_ids(c) {
            if in_set.contains(&p) {
                children.entry(p).or_default().push(c);
            }
        }
    }

    // Where each remote-tracking branch rejoins the local graph: the first in-set commit along the
    // remote tip's first-parent spine. These are segment boundaries (the remote connects INTO them).
    let remote_rejoins: HashSet<gix::ObjectId> = remote_tracking
        .values()
        .filter_map(|r| cg.commit_by_ref(r.as_ref()))
        .filter_map(|tip| {
            let mut c = Some(tip);
            while let Some(id) = c {
                if in_set.contains(&id) {
                    return Some(id);
                }
                c = cg.first_parent(id);
            }
            None
        })
        .collect();

    // A commit starts a new segment when it carries a disambiguated ref, is the workspace tip, is a
    // merge, or is a convergence/branch point (reached by other than a single first-parent child).
    let is_boundary = |c: gix::ObjectId| -> bool {
        c == workspace_commit
            || remote_rejoins.contains(&c)
            || disambiguated_ref(cg, c, remote_tracking).is_some()
            || cg.all_parent_ids(c).len() > 1
            || {
                let kids = children.get(&c).map(Vec::as_slice).unwrap_or_default();
                // Reached by a non-first-parent edge, or by more than one child.
                kids.len() > 1
                    || kids
                        .iter()
                        .any(|&k| cg.first_parent(k) != Some(c) && in_set.contains(&k))
            }
    };

    // Assign each in-set commit to the segment tip that owns it: walk UP first-parents to the nearest
    // boundary. The owner's commit run is [boundary .. next boundary) along first parents.
    let mut owner_of: HashMap<gix::ObjectId, gix::ObjectId> = HashMap::new();
    for &c in &in_set {
        let mut tip = c;
        while !is_boundary(tip) {
            match cg.first_parent(tip) {
                Some(p) if in_set.contains(&p) => tip = p,
                _ => break,
            }
        }
        owner_of.insert(c, tip);
    }

    // Segment tips in a stable order (workspace first, then by descending generation, then id) so the
    // numbering is deterministic even though it need not match the walk's.
    let mut tips: Vec<gix::ObjectId> = owner_of
        .values()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    tips.sort_by_key(|&t| {
        (
            t != workspace_commit,
            std::cmp::Reverse(cg.node(t).map(|n| n.generation).unwrap_or(0)),
            t,
        )
    });

    let mut sg = SegmentGraph::new();
    let mut seg_of_tip: HashMap<gix::ObjectId, SegmentIndex> = HashMap::new();

    // Create a local segment per tip, holding its first-parent commit run.
    for &tip in &tips {
        let commits = commit_run(cg, tip, &in_set, &is_boundary);
        // The workspace tip is named by the workspace ref itself (a `gitbutler/*` ref, which normal
        // disambiguation skips); every other tip by its disambiguated branch.
        let ref_name = if tip == workspace_commit {
            cg.refs_at(tip)
                .into_iter()
                .find(|r| r.as_bstr().starts_with_str("refs/heads/gitbutler/"))
        } else {
            disambiguated_ref(cg, tip, remote_tracking)
        };
        let ref_info = ref_name.map(|ref_name| RefInfo {
            ref_name,
            commit_id: Some(tip),
            worktree: None,
        });
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let sidx = sg.add_node(Segment {
            id: 0,
            generation: 0,
            ref_info,
            remote_tracking_ref_name,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(sidx).expect("just added").id = sidx;
        seg_of_tip.insert(tip, sidx);
    }

    // Connections: for each segment, its bottom commit's parents point at the segment owning each
    // parent, in first-parent order.
    for &tip in &tips {
        let src = seg_of_tip[&tip];
        let bottom = sg
            .node(src)
            .expect("present")
            .commits
            .last()
            .map(|c| c.id)
            .unwrap_or(tip);
        for parent in cg.all_parent_ids(bottom) {
            if let Some(&owner) = owner_of.get(&parent) {
                let dst = seg_of_tip[&owner];
                let conn = Connection::new(dst, None, Some(bottom), None, Some(parent));
                sg.add_edge(src, conn);
            }
        }
    }

    // Remote segments: for each local segment with a remote-tracking ref whose remote tip is present,
    // create a remote root segment (holding the remote-ahead commits) that connects into the local
    // segment, doubly-linked via siblings.
    add_remote_segments(cg, &mut sg, &seg_of_tip, &in_set, &owner_of);

    // Empty metadata branches (no commits) are spliced in as empty segments at their place in the
    // stack's branch order.
    insert_empty_branches(&mut sg, stack_branches);

    // Generations: longest path from a root (a segment with no incoming connections).
    assign_generations(&mut sg);

    crate::Graph {
        inner: sg,
        project_meta,
        options,
        ..crate::Graph::default()
    }
}

/// The first-parent commit run owned by `tip`: `tip` and each first-parent descendant-in-history until
/// the next boundary (exclusive) or the set edge.
fn commit_run(
    cg: &CommitGraph,
    tip: gix::ObjectId,
    in_set: &HashSet<gix::ObjectId>,
    is_boundary: &impl Fn(gix::ObjectId) -> bool,
) -> Vec<Commit> {
    let mut out = Vec::new();
    let mut id = Some(tip);
    while let Some(c) = id {
        if !in_set.contains(&c) {
            break;
        }
        if c != tip && is_boundary(c) {
            break;
        }
        if let Some(node) = cg.node(c) {
            out.push(node.commit.clone());
        }
        id = cg.first_parent(c).filter(|p| in_set.contains(p));
    }
    out
}

fn add_remote_segments(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    in_set: &HashSet<gix::ObjectId>,
    owner_of: &HashMap<gix::ObjectId, gix::ObjectId>,
) {
    let locals: Vec<(SegmentIndex, gix::refs::FullName, gix::ObjectId)> = seg_of_tip
        .iter()
        .filter_map(|(&tip, &sidx)| {
            sg.node(sidx)
                .and_then(|s| s.remote_tracking_ref_name.clone())
                .map(|rt| (sidx, rt, tip))
        })
        .collect();
    for (local_sidx, remote_ref, _local_tip) in locals {
        let Some(remote_tip) = cg.commit_by_ref(remote_ref.as_ref()) else {
            continue;
        };
        // Walk the remote tip's first-parent spine, collecting the commits it is ahead by (not in the
        // local set) until it rejoins the local graph. `rejoin` is where the remote reconnects.
        let mut ahead: Vec<Commit> = Vec::new();
        let mut rejoin = None;
        let mut c = Some(remote_tip);
        while let Some(id) = c {
            if in_set.contains(&id) {
                rejoin = Some(id);
                break;
            }
            if let Some(node) = cg.node(id) {
                ahead.push(node.commit.clone());
            }
            c = cg.first_parent(id);
        }
        let remote_sidx = sg.add_node(Segment {
            id: 0,
            generation: 0,
            ref_info: Some(RefInfo {
                ref_name: remote_ref.clone(),
                commit_id: Some(remote_tip),
                worktree: None,
            }),
            remote_tracking_ref_name: None,
            sibling_segment_id: Some(local_sidx),
            remote_tracking_branch_segment_id: None,
            commits: ahead,
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(remote_sidx).expect("just added").id = remote_sidx;
        sg.node_mut(local_sidx)
            .expect("present")
            .remote_tracking_branch_segment_id = Some(remote_sidx);
        // Connect the remote segment to the segment owning the rejoin commit.
        if let Some(rejoin) = rejoin
            && let Some(&owner) = owner_of.get(&rejoin)
            && let Some(&dst) = seg_of_tip.get(&owner)
        {
            let src_last = sg
                .node(remote_sidx)
                .and_then(|s| s.commits.last().map(|c| c.id));
            sg.add_edge(
                remote_sidx,
                Connection::new(dst, None, src_last, None, Some(rejoin)),
            );
        }
    }
}

/// Find the segment named exactly `ref_name`, if any.
fn segment_by_ref(sg: &SegmentGraph, ref_name: &gix::refs::FullName) -> Option<SegmentIndex> {
    sg.node_indices().find(|&sidx| {
        sg.node(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| &ri.ref_name == ref_name)
    })
}

/// Splice each stack's empty metadata branches (those with no commits, hence no ref on a commit) into
/// the segment chain at their position in the branch list: an empty branch sits between the segment
/// above it and that segment's base.
fn insert_empty_branches(
    sg: &mut SegmentGraph,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
) {
    let Some(lists) = stack_branches else {
        return;
    };
    for list in lists {
        let Some(first) = list.first() else {
            continue;
        };
        let mut current = segment_by_ref(sg, first);
        for b in &list[1..] {
            if let Some(existing) = segment_by_ref(sg, b) {
                current = Some(existing);
            } else if let Some(cur) = current {
                // Insert an empty segment `b` between `cur` and its base: `cur`'s outgoing connections
                // move to the new empty segment, and `cur` connects into it.
                let moved = std::mem::take(&mut sg.node_mut(cur).expect("present").connections);
                let src_last = sg.node(cur).and_then(|s| s.commits.last().map(|c| c.id));
                let new = sg.add_node(Segment {
                    id: 0,
                    generation: 0,
                    ref_info: Some(RefInfo {
                        ref_name: b.clone(),
                        commit_id: None,
                        worktree: None,
                    }),
                    remote_tracking_ref_name: None,
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
                    commits: Vec::new(),
                    metadata: None,
                    connections: moved,
                });
                sg.node_mut(new).expect("just added").id = new;
                sg.add_edge(cur, Connection::new(new, None, src_last, None, None));
                current = Some(new);
            }
        }
    }
}

/// Longest path from a root (segment with no incoming connection); roots are generation 0.
fn assign_generations(sg: &mut SegmentGraph) {
    let order = sg.toposort();
    // toposort yields sources-before-targets; connections point tip→base, so a base's generation is
    // 1 + max over its incoming sources.
    let mut depth: HashMap<SegmentIndex, usize> = HashMap::new();
    for sidx in &order {
        depth.entry(*sidx).or_insert(0);
    }
    for sidx in order {
        let g = depth[&sidx];
        let targets: Vec<SegmentIndex> = sg
            .node(sidx)
            .map(|s| s.connections.iter().map(|c| c.target).collect())
            .unwrap_or_default();
        for t in targets {
            let e = depth.entry(t).or_insert(0);
            *e = (*e).max(g + 1);
        }
    }
    for (sidx, g) in depth {
        if let Some(s) = sg.node_mut(sidx) {
            s.generation = g;
        }
    }
}

/// All ancestors of `start` (inclusive) present in the graph, walking every parent.
fn ancestors(cg: &CommitGraph, start: gix::ObjectId) -> HashSet<gix::ObjectId> {
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(c) = stack.pop() {
        if cg.node(c).is_none() {
            continue;
        }
        if seen.insert(c) {
            stack.extend(cg.all_parent_ids(c));
        }
    }
    seen
}

/// The unambiguous local-branch at `c`: prefer the single branch with a remote-tracking branch, else
/// the single branch overall (mirrors the projection's remote-tiered disambiguation).
fn disambiguated_ref(
    cg: &CommitGraph,
    c: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Option<gix::refs::FullName> {
    let branches: Vec<gix::refs::FullName> = cg
        .refs_at(c)
        .into_iter()
        .filter(is_plain_local_branch)
        .collect();
    let unique = |pred: &dyn Fn(&gix::refs::FullName) -> bool| {
        let mut it = branches.iter().filter(|r| pred(r));
        it.next().filter(|_| it.next().is_none()).cloned()
    };
    unique(&|r| remote_tracking.contains_key(r)).or_else(|| unique(&|_| true))
}

fn is_plain_local_branch(rn: &gix::refs::FullName) -> bool {
    let rn = rn.as_ref();
    rn.category() == Some(Category::LocalBranch)
        && !rn.as_bstr().starts_with_str("refs/heads/gitbutler/")
}
