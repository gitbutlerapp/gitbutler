//! Every read of graph topology lives here, and only here.
//!
//! The listing itself only assembles stacks from what these functions return, so
//! when `but-graph` changes its model, this file is the port target: rewrite these
//! functions against the new API and the listing snapshot tests in
//! `tests/branches` decide whether the port preserved behavior. The stack
//! builders in `lib.rs` still read projection *data* (workspace stacks and their
//! segments), but ask no topology questions of their own.
//!
//! Topology is read from the workspace's [`BranchGraph`](but_graph::BranchGraph): a flat
//! adjacency list of branches (named or anonymous runs of commits, plus empty named
//! routing nodes), where a "segment" is a branch index into that list.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use bstr::BString;
use but_core::RefMetadata;
use but_graph::{CommitFlags, Workspace, branch_graph::Branch, init::Tip};

use gix::refs::{Category, FullName};

use crate::display_identity;

/// An index into the branch list of the [`Topology`], the unit of ownership in the listing.
pub(crate) type SegmentIndex = usize;

/// Build the workspace graph from `head`, additionally traversing the given
/// `(tip, ref name)` pairs, and report whether a traversal limit cut it short.
pub(crate) fn build_workspace(
    head: gix::Id<'_>,
    head_ref: Option<FullName>,
    extra_tips: impl IntoIterator<Item = (gix::ObjectId, FullName)>,
    meta: &impl RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
    db: &mut but_db::DbHandle,
    traversal: but_graph::init::Options,
) -> anyhow::Result<(Workspace, bool)> {
    let ws = Workspace::from_commit_traversal_with_extra_tips(
        head,
        head_ref,
        extra_tips
            .into_iter()
            .map(|(tip, ref_name)| Tip::reachable(tip, Some(ref_name))),
        meta,
        project_meta,
        db,
        traversal,
    )?;
    let incomplete = ws.hard_limit_hit;
    Ok((ws, incomplete))
}

/// The branch topology of a workspace, with the reverse edges and lookups the listing needs
/// precomputed once so per-branch queries stay cheap.
pub(crate) struct Topology<'a> {
    ws: &'a Workspace,
    branches: Vec<Branch>,
    /// Per branch, the branches connecting *into* it.
    incoming: Vec<Vec<SegmentIndex>>,
    /// The branch owning each walked commit.
    owner_of_commit: HashMap<gix::ObjectId, SegmentIndex>,
    /// The branch named by each ref name.
    by_ref: BTreeMap<FullName, SegmentIndex>,
}

impl<'a> Topology<'a> {
    /// Read the topology of `ws`; `repo` only resolves the workspace commit by message.
    pub(crate) fn new(ws: &'a Workspace, repo: &gix::Repository) -> Self {
        let branches = ws.branch_graph(repo).branches;
        let mut incoming = vec![Vec::new(); branches.len()];
        let mut owner_of_commit = HashMap::new();
        let mut by_ref = BTreeMap::new();
        for (idx, branch) in branches.iter().enumerate() {
            for &(target, _) in &branch.outgoing {
                if let Some(sources) = incoming.get_mut(target) {
                    sources.push(idx);
                }
            }
            for commit in &branch.commits {
                owner_of_commit.insert(commit.id, idx);
            }
            if let Some(name) = &branch.ref_name {
                by_ref.entry(name.clone()).or_insert(idx);
            }
        }
        Topology {
            ws,
            branches,
            incoming,
            owner_of_commit,
            by_ref,
        }
    }

    fn branch(&self, idx: SegmentIndex) -> &Branch {
        &self.branches[idx]
    }

    /// The branch owning `commit`, if it was walked.
    pub(crate) fn owner_of(&self, commit: gix::ObjectId) -> Option<SegmentIndex> {
        self.owner_of_commit.get(&commit).copied()
    }

    /// The branch a projected stack segment corresponds to: the one carrying its name, else the
    /// one owning its tip commit.
    pub(crate) fn branch_of_stack_segment(
        &self,
        segment: &but_graph::workspace::StackSegment,
    ) -> Option<SegmentIndex> {
        segment
            .ref_name()
            .and_then(|name| self.by_ref.get(name).copied())
            .or_else(|| segment.tip().and_then(|tip| self.owner_of(tip)))
    }

    /// The first-parent successor of `idx`: the lowest parent-order edge.
    fn first_parent(&self, idx: SegmentIndex) -> Option<SegmentIndex> {
        self.branch(idx)
            .outgoing
            .iter()
            .min_by_key(|(_, order)| *order)
            .map(|(target, _)| *target)
    }

    /// The commit `idx` resolves to: its first commit, or, for an empty branch, the first commit
    /// of the branch it routes to (unambiguously, i.e. through a single outgoing edge).
    fn tip_skip_empty(&self, mut idx: SegmentIndex) -> Option<gix::ObjectId> {
        for _ in 0..self.branches.len().max(1) {
            let branch = self.branch(idx);
            if let Some(commit) = branch.commits.first() {
                return Some(commit.id);
            }
            match branch.outgoing.as_slice() {
                [(next, _)] => idx = *next,
                _ => return None,
            }
        }
        None
    }

    /// Whether `commit` was walked but its history below was not: the traversal limit or a
    /// shallow boundary cut it off, so history exists that is not part of the graph.
    pub(crate) fn commit_clipped(&self, commit: gix::ObjectId) -> bool {
        let Some(commit_graph) = self.ws.commit_graph_ref() else {
            return false;
        };
        let Some(node) = commit_graph.commit(commit) else {
            return false;
        };
        if node.flags.contains(CommitFlags::ShallowBoundary) {
            return true;
        }
        !node.parent_ids.is_empty() && commit_graph.walked_parent_count(commit) == 0
    }
}

/// The target branch tip and the branch owning it, if there is a target.
pub(crate) fn target_of(topo: &Topology<'_>) -> Option<(gix::ObjectId, SegmentIndex)> {
    let ws = topo.ws;
    ws.target_ref
        .as_ref()
        .and_then(|target| {
            let tip = target.tip_commit_id?;
            let idx = topo
                .by_ref
                .get(&target.ref_name)
                .copied()
                .or_else(|| topo.owner_of(tip))?;
            Some((tip, idx))
        })
        .or_else(|| {
            let target = ws.target_commit.as_ref()?;
            Some((target.commit_id, topo.owner_of(target.commit_id)?))
        })
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

/// Map every ref name found on a branch or one of its commits to that branch,
/// along with the commit the ref points to: the branch's first commit, or for
/// empty branches the commit they route to.
pub(crate) fn ref_index(
    topo: &Topology<'_>,
) -> BTreeMap<FullName, (SegmentIndex, Option<gix::ObjectId>)> {
    let mut index = BTreeMap::new();
    for (idx, branch) in topo.branches.iter().enumerate() {
        if let Some(name) = &branch.ref_name {
            index.insert(name.clone(), (idx, topo.tip_skip_empty(idx)));
        }
        for commit in &branch.commits {
            for info in &commit.refs {
                index
                    .entry(info.ref_name.clone())
                    .or_insert((idx, Some(commit.id)));
            }
        }
    }
    index
}

/// All segments reachable from `start`, including itself.
///
/// The result is computed once and shared as the excluded set across every branch's
/// [`count_outside()`] call, which is what keeps the listing linear in the number of branches.
pub(crate) fn reachable_from(topo: &Topology<'_>, start: SegmentIndex) -> BTreeSet<SegmentIndex> {
    let mut reachable = BTreeSet::new();
    let mut queue = vec![start];
    while let Some(idx) = queue.pop() {
        if !reachable.insert(idx) {
            continue;
        }
        queue.extend(topo.branch(idx).outgoing.iter().map(|(target, _)| *target));
    }
    reachable
}

/// Count the commits reachable from `start` but not through `excluded` segments,
/// or `None` if the count would be cut short by a traversal limit.
pub(crate) fn count_outside(
    topo: &Topology<'_>,
    excluded: &BTreeSet<SegmentIndex>,
    start: SegmentIndex,
) -> Option<usize> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![start];
    let mut count = 0;
    while let Some(idx) = queue.pop() {
        if excluded.contains(&idx) || !seen.insert(idx) {
            continue;
        }
        count += topo.branch(idx).commits.len();
        if traversal_was_clipped(topo, idx) {
            // The connection to the excluded set lies beyond the traversal limit;
            // an exact-looking count would be wrong.
            return None;
        }
        queue.extend(topo.branch(idx).outgoing.iter().map(|(target, _)| *target));
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
    topo: &Topology<'_>,
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
    let mut seen = BTreeSet::new();
    let mut cur = Some(start);
    while let Some(idx) = cur {
        if !seen.insert(idx) {
            break;
        }
        let branch = topo.branch(idx);
        if boundary_flags(branch, target_tip) {
            break;
        }
        if idx != start {
            if branch_has_foreign_name(branch, identity, remote_names) {
                if traversal_was_clipped(topo, idx) {
                    // The boundary segment was never actually walked; the full
                    // graph may have continued through it differently, so neither
                    // the count nor the boundary can be trusted.
                    out.clipped = true;
                } else {
                    out.boundary = Some(idx);
                }
                break;
            }
            if is_shared_history(topo, idx) {
                break;
            }
        }
        out.commit_count += branch.commits.len();
        if traversal_was_clipped(topo, idx) {
            // The fork point lies beyond the traversal limit.
            out.clipped = true;
            break;
        }
        cur = topo.first_parent(idx);
    }
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
pub(crate) fn traversal_was_clipped(topo: &Topology<'_>, segment: SegmentIndex) -> bool {
    let branch = topo.branch(segment);
    if let Some(last) = branch.commits.last() {
        return topo.commit_clipped(last.id);
    }
    // An empty segment without connections is a tip whose commit was never walked,
    // which only happens when the traversal was cut short before reaching it.
    topo.ws.hard_limit_hit && branch.outgoing.is_empty()
}

/// Return `true` if `branch` is named after a branch other than `identity`.
///
/// A branch named by the branch's own remote-tracking ref is transparent: a branch
/// whose remote lags behind still owns the commits below the remote's position.
fn branch_has_foreign_name(
    branch: &Branch,
    identity: &BString,
    remote_names: &gix::remote::Names,
) -> bool {
    branch
        .ref_name
        .as_ref()
        .is_some_and(|name| display_identity(name, remote_names) != *identity)
}

/// Return `true` if `branch` starts history that belongs to the workspace or target.
///
/// The comparison with `target_tip` matters when the target tip is also reachable
/// as local history, where its commit carries no integrated flag.
fn boundary_flags(branch: &Branch, target_tip: Option<gix::ObjectId>) -> bool {
    let Some(first_commit) = branch.commits.first() else {
        return false;
    };
    first_commit
        .flags
        .intersects(CommitFlags::InWorkspace | CommitFlags::Integrated)
        || Some(first_commit.id) == target_tip
}

/// Return `true` if `segment` is history shared with other branches, i.e. more than
/// one branch connects to it. This bounds commit counts at fork points in
/// repositories without a target, where no commit carries an integrated flag.
///
/// Connections from remote-tracking branches don't count as sharing: a branch whose
/// own remote lags behind still owns the commits below the remote's position.
fn is_shared_history(topo: &Topology<'_>, segment: SegmentIndex) -> bool {
    topo.incoming[segment]
        .iter()
        .filter(|&&source| {
            topo.branch(source)
                .ref_name
                .as_ref()
                .is_none_or(|name| name.category() != Some(Category::RemoteBranch))
        })
        .count()
        > 1
}
