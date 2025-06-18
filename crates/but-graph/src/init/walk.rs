use crate::{CommitIndex, SegmentIndex};
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use std::collections::{BTreeSet, VecDeque};
use std::ops::Range;

/// A custom topological walk that always goes in a given [direction](Direction),
/// does not need a graph reference, and which yields ranges of non-overlapping commit in a segment.
///
/// This walk assumes the worst-case graph where edges point to any commit, even from commits that
/// aren't the first one or target commits that aren't the first one in a segment.
///
/// ### Note
///
/// In theory, a normal [`petgraph::visit::Topo`] would do here, if we assume that everything works
/// as it should. So at some point, this code might be removed once it's clear we won't need it anymore.
/// TODO: one fine day remove this in favor of `petgraph::visit::Topo`.
pub(crate) struct TopoWalk {
    /// The segment we
    next: VecDeque<(SegmentIndex, Option<CommitIndex>)>,
    /// Commits we have already yielded.
    seen: gix::hashtable::HashSet,
    /// If segments have no commits we can't identify them unless we track them separately.
    seen_empty_segments: BTreeSet<SegmentIndex>,
    /// In which direction to traverse.
    direction: Direction,
    /// If `true`, don't return the first segment which is always the starting point.
    skip_tip: Option<()>,
}

/// Lifecycle
impl TopoWalk {
    /// Start a walk at `segment`, possibly only from `commit`.
    pub fn start_from(
        segment: SegmentIndex,
        commit: Option<CommitIndex>,
        direction: Direction,
    ) -> Self {
        TopoWalk {
            next: {
                let mut v = VecDeque::new();
                v.push_back((segment, commit));
                v
            },
            seen: Default::default(),
            seen_empty_segments: Default::default(),
            direction,
            skip_tip: None,
        }
    }
}

/// Builder
impl TopoWalk {
    /// Call to not return the tip as part of the iteration.
    pub fn skip_tip(mut self) -> Self {
        self.skip_tip = Some(());
        self
    }
}

/// Iteration
impl TopoWalk {
    /// Obtain the next segment and unseen commit range in the topo-walk.
    /// Note that the returned range may yield an empty slice, or a sub-slice
    /// of all available commits.
    pub fn next(
        &mut self,
        graph: &crate::init::PetGraph,
    ) -> Option<(SegmentIndex, Range<CommitIndex>)> {
        while !self.next.is_empty() {
            let res = self.next_inner(graph);
            if res.is_some() {
                if self.skip_tip.take().is_some() {
                    continue;
                }
                return res;
            }
        }
        None
    }

    fn next_inner(
        &mut self,
        graph: &crate::init::PetGraph,
    ) -> Option<(SegmentIndex, Range<CommitIndex>)> {
        let (segment, first_commit_index) = self.next.pop_front()?;
        let available_range = self.select_range(graph, segment, first_commit_index)?;

        for edge in graph.edges_directed(segment, self.direction) {
            match self.direction {
                Direction::Outgoing => {
                    if edge
                        .weight()
                        .src
                        .is_some_and(|src_cidx| !available_range.contains(&src_cidx))
                    {
                        continue;
                    }
                    self.next.push_back((edge.target(), edge.weight().dst));
                }
                Direction::Incoming => {
                    if edge
                        .weight()
                        .dst
                        .is_some_and(|dst_cidx| !available_range.contains(&dst_cidx))
                    {
                        continue;
                    }
                    self.next.push_back((edge.source(), edge.weight().src));
                }
            }
        }
        Some((segment, available_range))
    }

    fn select_range(
        &mut self,
        graph: &crate::init::PetGraph,
        segment: SegmentIndex,
        first_commit_index: Option<CommitIndex>,
    ) -> Option<Range<CommitIndex>> {
        match first_commit_index {
            None => {
                debug_assert!(
                    graph[segment].commits.is_empty(),
                    "BUG: we always assure that we set the commit index if a segment has commits: {segment:?}"
                );
                if !self.seen_empty_segments.insert(segment) {
                    return None;
                }
                Some(0..0)
            }
            Some(start_commit_idx) => {
                let segment = &graph[segment];
                let mut range = match self.direction {
                    Direction::Outgoing => start_commit_idx..segment.commits.len(),
                    Direction::Incoming => 0..start_commit_idx,
                };

                // NOTE: this assumes that we always consume all segments from a given starting position,
                //       which is assured by the lines above that set the initial range.
                let num_inserted_from_front = segment
                    .commits
                    .get(range.clone())?
                    .iter()
                    .take_while(|c| self.seen.insert(c.id))
                    .count();
                if num_inserted_from_front == 0 {
                    let num_inserted_from_back = segment
                        .commits
                        .get(range.clone())?
                        .iter()
                        .rev()
                        .take_while(|c| self.seen.insert(c.id))
                        .count();
                    if num_inserted_from_back == 0 {
                        return None;
                    }
                    range.start = range.end - num_inserted_from_back;
                } else {
                    range.end = range.start + num_inserted_from_front;
                }
                debug_assert!(
                    !range.is_empty(),
                    "BUG: empty ranges should already have been handled, must return None"
                );
                Some(range)
            }
        }
    }
}
