//! Every read of graph topology lives here, and only here.
//!
//! The listing itself only assembles stacks from what these functions return, so
//! when `but-graph` changes its model, this file is the port target: rewrite these
//! functions against the new API and the listing snapshot tests in
//! `tests/branches` decide whether the port preserved behavior. The stack
//! builders in `lib.rs` still read projection *data* (workspace stacks and their
//! segments), but ask no topology questions of their own.

use std::collections::{BTreeMap, BTreeSet};

use bstr::BString;
use but_core::RefMetadata;
use but_graph::{CommitFlags, Graph, SegmentIndex, Workspace, init::Tip};

use gix::refs::{Category, FullName};

use crate::display_identity;

/// Build the workspace graph from `head`, additionally traversing the given
/// `(tip, ref name)` pairs, and report whether a traversal limit cut it short.
///
/// `integrated_tip` marks a commit whose history counts as integrated even without
/// workspace metadata; `hard_limit` bounds the traversal for very large repositories.
pub(crate) fn build_workspace(
    head: gix::Id<'_>,
    head_ref: Option<FullName>,
    extra_tips: impl IntoIterator<Item = (gix::ObjectId, FullName)>,
    meta: &impl RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
    integrated_tip: Option<gix::ObjectId>,
    hard_limit: Option<usize>,
) -> anyhow::Result<(Workspace, bool)> {
    let traversal = but_graph::init::Options {
        extra_target_commit_id: integrated_tip,
        hard_limit,
        ..Default::default()
    };
    let graph = Graph::from_commit_traversal_with_extra_tips(
        head,
        head_ref,
        extra_tips
            .into_iter()
            .map(|(tip, ref_name)| Tip::reachable(tip, Some(ref_name))),
        meta,
        project_meta,
        traversal,
    )?;
    let incomplete = graph.hard_limit_hit();
    Ok((graph.into_workspace()?, incomplete))
}

/// The target branch tip and the segment owning it, if there is a target.
pub(crate) fn target_of(ws: &Workspace) -> Option<(gix::ObjectId, SegmentIndex)> {
    ws.target_ref
        .as_ref()
        .and_then(|target| {
            ws.graph
                .resolve_to_unambiguously_pointed_to_commit(target.segment_index)
                .map(|(commit, segment)| (commit.id, segment))
        })
        .or(ws
            .target_commit
            .as_ref()
            .map(|target| (target.commit_id, target.segment_index)))
}

/// How a ref relates to the segment the [`ref_index()`] maps it to.
pub(crate) enum Anchor {
    /// The ref heads its own segment and owns exclusive history.
    OwnsTip(SegmentIndex),
    /// The ref points into history owned by another branch; it owns no commits.
    MidHistory(SegmentIndex),
    /// The traversal was cut short before this ref's commit was walked; nothing
    /// exact is known about it.
    Unreached(SegmentIndex),
}

impl Anchor {
    /// Classify a [`ref_index()`] entry against the commit the ref was enumerated at.
    pub(crate) fn classify(
        segment: SegmentIndex,
        commit: Option<gix::ObjectId>,
        tip: gix::ObjectId,
    ) -> Self {
        match commit {
            Some(commit) if commit == tip => Anchor::OwnsTip(segment),
            Some(_) => Anchor::MidHistory(segment),
            None => Anchor::Unreached(segment),
        }
    }

    /// The segment the ref maps to, whatever its relation.
    pub(crate) fn segment(&self) -> SegmentIndex {
        match *self {
            Anchor::OwnsTip(segment) | Anchor::MidHistory(segment) | Anchor::Unreached(segment) => {
                segment
            }
        }
    }
}

/// Map every ref name found on a segment or one of its commits to that segment,
/// along with the commit the ref points to: the segment's first commit, or for
/// empty segments the peeled id recorded on the ref itself.
pub(crate) fn ref_index(
    graph: &Graph,
) -> BTreeMap<FullName, (SegmentIndex, Option<gix::ObjectId>)> {
    let mut index = BTreeMap::new();
    for sidx in graph.segments() {
        let segment = &graph[sidx];
        if let Some(info) = &segment.ref_info {
            index.insert(
                info.ref_name.clone(),
                (
                    sidx,
                    segment.commits.first().map(|c| c.id).or(info.commit_id),
                ),
            );
        }
        for commit in &segment.commits {
            for info in &commit.refs {
                index
                    .entry(info.ref_name.clone())
                    .or_insert((sidx, Some(commit.id)));
            }
        }
    }
    index
}

/// All segments reachable from `start`, including itself.
///
/// Unlike [`Graph::find_segments_reachable_from_a_not_b()`], the result is computed
/// once and shared as the excluded set across every branch's [`count_outside()`]
/// call, which is what keeps the listing linear in the number of branches.
pub(crate) fn reachable_from(graph: &Graph, start: SegmentIndex) -> BTreeSet<SegmentIndex> {
    use but_graph::petgraph::{Direction, visit::EdgeRef};
    let mut reachable = BTreeSet::new();
    let mut queue = vec![start];
    while let Some(sidx) = queue.pop() {
        if !reachable.insert(sidx) {
            continue;
        }
        queue.extend(
            graph
                .edges_directed(sidx, Direction::Outgoing)
                .map(|edge| edge.target()),
        );
    }
    reachable
}

/// Count the commits reachable from `start` but not through `excluded` segments,
/// or `None` if the count would be cut short by a traversal limit — a clip-awareness
/// that [`Graph::find_commits_reachable_from_a_not_b()`] does not offer.
pub(crate) fn count_outside(
    graph: &Graph,
    excluded: &BTreeSet<SegmentIndex>,
    start: SegmentIndex,
) -> Option<usize> {
    use but_graph::petgraph::{Direction, visit::EdgeRef};
    let mut seen = BTreeSet::new();
    let mut queue = vec![start];
    let mut count = 0;
    while let Some(sidx) = queue.pop() {
        if excluded.contains(&sidx) || !seen.insert(sidx) {
            continue;
        }
        count += graph[sidx].commits.len();
        if traversal_was_clipped(graph, sidx) {
            // The connection to the excluded set lies beyond the traversal limit;
            // an exact-looking count would be wrong.
            return None;
        }
        queue.extend(
            graph
                .edges_directed(sidx, Direction::Outgoing)
                .map(|edge| edge.target()),
        );
    }
    Some(count)
}

/// What a downward walk over a branch's own first-parent history found.
pub(crate) struct OwnedHistory {
    /// Commits in segments owned by the branch, up to the boundary.
    pub(crate) commit_count: usize,
    /// The segment named after another branch that ended the walk, if any.
    pub(crate) boundary: Option<SegmentIndex>,
    /// The walk ran into the traversal limit before finding a boundary.
    pub(crate) clipped: bool,
}

/// Walk down the first-parent history of `start`, owned by the branch named
/// `identity`, until its exclusive history ends: at workspace or integrated
/// commits, at the target tip, at a segment named after another branch, or at
/// history shared with other branches. This is the single definition of where
/// one branch ends, used for both commit counts and stack inference.
pub(crate) fn owned_history(
    graph: &Graph,
    start: SegmentIndex,
    identity: &BString,
    remote_names: &gix::remote::Names,
    target_tip: Option<gix::ObjectId>,
) -> OwnedHistory {
    let mut out = OwnedHistory {
        commit_count: 0,
        boundary: None,
        clipped: false,
    };
    graph.visit_segments_downward_along_first_parent_include_start(start, |seg| {
        if boundary_flags(seg, target_tip) {
            return true;
        }
        if seg.id != start {
            if segment_has_foreign_name(seg, identity, remote_names) {
                if traversal_was_clipped(graph, seg.id) {
                    // The boundary segment was never actually walked; the full
                    // graph may have continued through it differently, so neither
                    // the count nor the boundary can be trusted.
                    out.clipped = true;
                } else {
                    out.boundary = Some(seg.id);
                }
                return true;
            }
            if is_shared_history(graph, seg.id) {
                return true;
            }
        }
        out.commit_count += seg.commits.len();
        if traversal_was_clipped(graph, seg.id) {
            // The fork point lies beyond the traversal limit.
            out.clipped = true;
            return true;
        }
        false
    });
    out
}

/// The commit a stack segment sits on: its first commit, the recorded ref position
/// for empty segments, or the base it rests on.
pub(crate) fn segment_tip(segment: &but_graph::workspace::StackSegment) -> Option<gix::ObjectId> {
    segment
        .tip()
        .or_else(|| segment.ref_info.as_ref().and_then(|info| info.commit_id))
        .or(segment.base)
}

/// Return `true` if traversal stopped at `segment` due to a limit or a shallow
/// boundary, meaning history below it exists but is not part of the graph.
pub(crate) fn traversal_was_clipped(graph: &Graph, segment: SegmentIndex) -> bool {
    use but_graph::{StopCondition, petgraph::Direction};
    if graph.stop_condition(segment).is_some_and(|condition| {
        condition.intersects(StopCondition::Limit | StopCondition::ShallowBoundary)
    }) {
        return true;
    }
    // An empty segment without connections is a tip whose commit was never walked,
    // which only happens when the traversal was cut short before reaching it.
    graph.hard_limit_hit()
        && graph[segment].commits.is_empty()
        && graph
            .edges_directed(segment, Direction::Outgoing)
            .next()
            .is_none()
}

/// Return `true` if `segment` is named after a branch other than `identity`.
///
/// A segment named by the branch's own remote-tracking ref is transparent: a branch
/// whose remote lags behind still owns the commits below the remote's position.
fn segment_has_foreign_name(
    segment: &but_graph::Segment,
    identity: &BString,
    remote_names: &gix::remote::Names,
) -> bool {
    segment
        .ref_info
        .as_ref()
        .is_some_and(|info| display_identity(&info.ref_name, remote_names) != *identity)
}

/// Return `true` if `segment` starts history that belongs to the workspace or target.
///
/// The comparison with `target_tip` matters when the target tip is also reachable
/// as local history, where its commit carries no integrated flag.
fn boundary_flags(segment: &but_graph::Segment, target_tip: Option<gix::ObjectId>) -> bool {
    let Some(first_commit) = segment.commits.first() else {
        return false;
    };
    first_commit
        .flags
        .intersects(CommitFlags::InWorkspace | CommitFlags::Integrated)
        || Some(first_commit.id) == target_tip
}

/// Return `true` if `segment` is history shared with other branches, i.e. more than
/// one segment connects to it. This bounds commit counts at fork points in
/// repositories without a target, where no commit carries an integrated flag.
///
/// Connections from remote-tracking segments don't count as sharing: a branch whose
/// own remote lags behind still owns the commits below the remote's position.
fn is_shared_history(graph: &Graph, segment: SegmentIndex) -> bool {
    use but_graph::petgraph::{Direction, visit::EdgeRef};
    graph
        .edges_directed(segment, Direction::Incoming)
        .filter(|edge| {
            graph[edge.source()]
                .ref_info
                .as_ref()
                .is_none_or(|info| info.ref_name.category() != Some(Category::RemoteBranch))
        })
        .count()
        > 1
}
