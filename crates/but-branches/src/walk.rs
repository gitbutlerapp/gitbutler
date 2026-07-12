//! Every read of graph topology lives here, and only here.
//!
//! The listing itself only assembles stacks from what these functions return, so
//! when `but-graph` changes its model, this file is the port target: rewrite these
//! functions against the new API and the listing snapshot tests in
//! `tests/branches` decide whether the port preserved behavior. The stack
//! builders in `lib.rs` still read projection *data* (workspace stacks and their
//! segments), but ask no topology questions of their own.
//!
//! The handle these functions pass around is a COMMIT ID: the commit a region of
//! history starts at. It replaces the segment index the segment-graph model used,
//! and serves the same purpose — an opaque, orderable key the listing groups by —
//! while being meaningful in the commit graph rather than in one projection of it.

use std::collections::{BTreeMap, BTreeSet};

use bstr::BString;
use but_core::RefMetadata;
use but_graph::{CommitFlags, CommitGraph, Workspace};

use gix::refs::{Category, FullName};

use crate::display_identity;

/// A region of history, identified by the commit it starts at.
pub(crate) type Region = gix::ObjectId;

/// Build the workspace from `head`, additionally traversing the given
/// `(tip, ref name)` pairs, and report whether a traversal limit cut it short.
///
/// `options.extra_target_commit_id` marks a commit whose history counts as integrated
/// even without workspace metadata; `options.hard_limit` bounds the traversal.
pub(crate) fn build_workspace(
    head: gix::Id<'_>,
    head_ref: Option<FullName>,
    extra_tips: impl IntoIterator<Item = (gix::ObjectId, FullName)>,
    meta: &impl RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
    db: &mut but_db::DbHandle,
    options: but_graph::walk::Options,
) -> anyhow::Result<(Workspace, bool)> {
    let repo = head.repo;
    let head = head.detach();
    let integrated_tip = options.extra_target_commit_id;
    // The workspace's own seeds first, so its declared branches still name segments,
    // then the branches outside it that only this listing cares about.
    let mut seeds = but_graph::walk::seeds_from_workspace_metadata(
        repo,
        meta,
        head,
        head_ref.as_ref(),
        &project_meta,
        integrated_tip,
    )?;
    let mut seeded_commits: BTreeSet<_> = seeds.iter().map(|seed| seed.id).collect();
    let seeded_refs: BTreeSet<_> = seeds
        .iter()
        .filter_map(|seed| seed.ref_name.clone())
        .collect();
    // One seed per commit and per ref name, which the traversal requires: two branches
    // on one commit would walk the same history twice. Refs left out here are still
    // discovered when the walk reaches the commit they point at.
    seeds.extend(
        extra_tips
            .into_iter()
            .filter(|(tip, ref_name)| {
                !seeded_refs.contains(ref_name) && seeded_commits.insert(*tip)
            })
            .map(|(tip, ref_name)| but_graph::walk::Seed::reachable(tip, Some(ref_name))),
    );
    let ws = Workspace::from_seeds(repo, seeds, meta, project_meta, db, options)?;
    // A hit traversal budget means history exists below what we can see.
    let incomplete = ws.is_truncated();
    Ok((ws, incomplete))
}

/// The target branch tip, if there is a target.
///
/// The tip doubles as its own region: the target's history starts at the commit it
/// points to.
pub(crate) fn target_of(ws: &Workspace) -> Option<(gix::ObjectId, Region)> {
    let tip = ws.resolved_target_commit_id()?;
    Some((tip, tip))
}

/// How a ref relates to the region the [`ref_index()`] maps it to.
pub(crate) enum Anchor {
    /// The ref heads its own region and owns exclusive history.
    OwnsTip(Region),
    /// The ref points into history owned by another branch; it owns no commits.
    MidHistory(Region),
    /// The traversal was cut short before this ref's commit was walked; nothing
    /// exact is known about it.
    Unreached(Region),
}

impl Anchor {
    /// Classify a [`ref_index()`] entry against the commit the ref was enumerated at.
    pub(crate) fn classify(
        region: Region,
        commit: Option<gix::ObjectId>,
        tip: gix::ObjectId,
    ) -> Self {
        match commit {
            Some(commit) if commit == tip => Anchor::OwnsTip(region),
            Some(_) => Anchor::MidHistory(region),
            None => Anchor::Unreached(region),
        }
    }

    /// The region the ref maps to, whatever its relation.
    pub(crate) fn region(&self) -> Region {
        match *self {
            Anchor::OwnsTip(r) | Anchor::MidHistory(r) | Anchor::Unreached(r) => r,
        }
    }
}

/// Map every ref name the graph knows to the commit it rests on, along with the
/// commit it actually points at.
///
/// The stored ref layout already records placement as a fact, so this reads it
/// rather than re-deriving it by walking segments.
pub(crate) fn ref_index(
    graph: &CommitGraph,
) -> BTreeMap<FullName, (Region, Option<gix::ObjectId>)> {
    let mut index = BTreeMap::new();
    let Some(layout) = graph.layout() else {
        return index;
    };
    for (name, on) in layout.placements() {
        index.insert(name.clone(), (on, graph.commit_by_ref(name.as_ref())));
    }
    // A ref riding on a commit without its own placement still belongs to the commit
    // it decorates.
    for id in graph.commit_ids().collect::<Vec<_>>() {
        let Some(node) = graph.node(id) else { continue };
        for info in &node.refs {
            index.entry(info.ref_name.clone()).or_insert((id, Some(id)));
        }
    }
    index
}

/// All commits reachable from `start`, including itself.
///
/// The result is computed once and shared as the excluded set across every branch's
/// [`count_outside()`] call, which is what keeps the listing linear in the number
/// of branches.
pub(crate) fn reachable_from(graph: &CommitGraph, start: Region) -> BTreeSet<Region> {
    let mut reachable: BTreeSet<Region> = graph.ancestor_set(start).into_iter().collect();
    reachable.insert(start);
    reachable
}

/// Count the commits reachable from `start` but not in `excluded`, or `None` if the
/// count would be cut short by a traversal limit — an exact-looking count would be
/// wrong when the connection to the excluded set lies beyond what was walked.
pub(crate) fn count_outside(
    graph: &CommitGraph,
    excluded: &BTreeSet<Region>,
    start: Region,
) -> Option<usize> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![start];
    let mut count = 0;
    while let Some(id) = queue.pop() {
        if excluded.contains(&id) || !seen.insert(id) {
            continue;
        }
        count += 1;
        if traversal_was_clipped(graph, id) {
            return None;
        }
        queue.extend(graph.parents(id));
    }
    Some(count)
}

/// What a downward walk over a branch's own first-parent history found.
pub(crate) struct OwnedHistory {
    /// Commits owned by the branch, up to the boundary.
    pub(crate) commit_count: usize,
    /// The region named after another branch that ended the walk, if any.
    pub(crate) boundary: Option<Region>,
    /// The walk ran into the traversal limit before finding a boundary.
    pub(crate) clipped: bool,
}

/// Walk down the first-parent history of `start`, owned by the branch named
/// `identity`, until its exclusive history ends: at workspace or integrated
/// commits, at the target tip, at a commit named after another branch, or at
/// history shared with other branches. This is the single definition of where
/// one branch ends, used for both commit counts and stack inference.
pub(crate) fn owned_history(
    graph: &CommitGraph,
    start: Region,
    identity: &BString,
    remote_names: &gix::remote::Names,
    target_tip: Option<gix::ObjectId>,
) -> OwnedHistory {
    let mut out = OwnedHistory {
        commit_count: 0,
        boundary: None,
        clipped: false,
    };
    let mut at = Some(start);
    let mut seen = BTreeSet::new();
    while let Some(id) = at.take() {
        if !seen.insert(id) {
            break;
        }
        if boundary_flags(graph, id, target_tip) {
            break;
        }
        if id != start {
            if commit_has_foreign_name(graph, id, identity, remote_names) {
                if traversal_was_clipped(graph, id) {
                    // The boundary was never actually walked; the full graph may have
                    // continued through it differently, so neither the count nor the
                    // boundary can be trusted.
                    out.clipped = true;
                } else {
                    out.boundary = Some(id);
                }
                break;
            }
            if is_shared_history(graph, id) {
                break;
            }
        }
        out.commit_count += 1;
        if traversal_was_clipped(graph, id) {
            // The fork point lies beyond the traversal limit.
            out.clipped = true;
            break;
        }
        at = graph.first_parent(id);
    }
    out
}

/// The commit that locates a segment: its first commit, the recorded ref position for
/// empty segments, or failing both, the base it rests on.
pub(crate) fn segment_tip(segment: &but_graph::workspace::StackSegment) -> Option<gix::ObjectId> {
    segment
        .tip()
        .or_else(|| segment.ref_info.as_ref().and_then(|info| info.commit_id))
        .or(segment.base)
}

/// Whether `segment`'s commits are all of them, rather than as many as the walk got to.
///
/// A count that is really a floor must not be reported as a count, so an empty segment
/// is only exact when it rests on a commit the walk actually reached: a segment truncated
/// away entirely looks empty too, and has no base to rest on.
pub(crate) fn segment_count_is_exact(
    graph: &CommitGraph,
    segment: &but_graph::workspace::StackSegment,
) -> bool {
    match segment.commits.last() {
        Some(bottom) => !traversal_was_clipped(graph, bottom.id),
        None => segment
            .base
            .is_some_and(|base| !traversal_was_clipped(graph, base)),
    }
}

/// Return `true` if traversal stopped at `at` due to a budget or a shallow
/// boundary, meaning history below it exists but is not part of the graph.
pub(crate) fn traversal_was_clipped(graph: &CommitGraph, at: Region) -> bool {
    // A commit whose raw parents were never walked ends its extent early — the
    // traversal's cut, worn by the commit that stopped.
    graph.has_cut_parents(at)
}

/// Return `true` if a ref on `at` names a branch other than `identity`.
///
/// A commit named by the branch's own remote-tracking ref is transparent: a branch
/// whose remote lags behind still owns the commits below the remote's position.
fn commit_has_foreign_name(
    graph: &CommitGraph,
    at: Region,
    identity: &BString,
    remote_names: &gix::remote::Names,
) -> bool {
    graph.node(at).is_some_and(|node| {
        node.refs
            .iter()
            .any(|info| display_identity(&info.ref_name, remote_names) != *identity)
    })
}

/// Return `true` if `at` starts history that belongs to the workspace or target.
///
/// The comparison with `target_tip` matters when the target tip is also reachable
/// as local history, where its commit carries no integrated flag.
fn boundary_flags(graph: &CommitGraph, at: Region, target_tip: Option<gix::ObjectId>) -> bool {
    let Some(node) = graph.node(at) else {
        return false;
    };
    node.flags
        .intersects(CommitFlags::InWorkspace | CommitFlags::Integrated)
        || Some(at) == target_tip
}

/// Return `true` if `at` is history shared with other branches, i.e. more than one
/// commit descends into it. This bounds commit counts at fork points in repositories
/// without a target, where no commit carries an integrated flag.
///
/// A descendant carrying a remote-tracking ref does not count as sharing: a branch
/// whose own remote lags behind still owns the commits below the remote's position.
fn is_shared_history(graph: &CommitGraph, at: Region) -> bool {
    graph
        .children(at)
        .into_iter()
        .filter(|child| {
            graph.node(*child).is_some_and(|node| {
                node.refs
                    .iter()
                    .all(|info| info.ref_name.category() != Some(Category::RemoteBranch))
            })
        })
        .count()
        > 1
}
