use petgraph::{Direction, prelude::EdgeRef, stable_graph::EdgeReference};
use std::{
    collections::{BTreeSet, VecDeque},
    ops::Range,
};

use crate::{CommitFlags, CommitIndex, Edge, SegmentIndex};

#[derive(Debug, Copy, Clone)]
pub struct Limit {
    inner: Option<usize>,
    /// The commit we want to see to be able to assume normal limits. Until then there is no limit.
    /// Each tracked commit is represented by bitflag, one for each goal, allowing commits to know
    /// if they can be reached by the tracked commit.
    /// The flag is empty if no goal is set.
    goal: CommitFlags,
}

/// Lifecycle and builders
impl Limit {
    pub fn new(value: Option<usize>) -> Self {
        Limit {
            inner: value,
            goal: CommitFlags::empty(),
        }
    }

    /// Keep queueing without limit until `goal` is seen in a commit that has **it ahead of itself**.
    /// Then stop searching for that goal.
    /// `goals` are used to keep track of existing bitflags.
    ///
    /// ### Note
    ///
    /// No goal will be set if we can't track more goals, effectively causing traversal to stop earlier,
    /// leaving potential isles in the graph.
    /// This can happen if we have to track a lot of remotes, but since these are queued later, they are also
    /// secondary and may just work for the typical remote.
    pub fn with_indirect_goal(mut self, goal: gix::ObjectId, goals: &mut Goals) -> Self {
        self.goal = goals.flag_for(goal).unwrap_or_default();
        self
    }

    /// Set two or more goals, by setting `goal` directly as previously obtained by [Goals::flag_for()].
    pub fn additional_goal(mut self, goal: CommitFlags) -> Self {
        self.goal |= goal;
        self
    }

    /// It's important to try to split the limit evenly so we don't create too
    /// much extra gas here. We do, however, make sure that we see each segment of a parent
    /// with one commit so we know exactly where it stops.
    /// The problem with this is that we never get back the split limit when segments re-unite,
    /// so effectively we loose gas here.
    pub fn per_parent(&self, num_parents: usize) -> Self {
        Limit {
            inner: self
                .inner
                .map(|l| if l == 0 { 0 } else { (l / num_parents).max(1) }),
            goal: self.goal,
        }
    }

    /// Assure this limit won't perform any traversal after reaching its goals.
    pub fn without_allowance(mut self) -> Self {
        self.set_but_keep_goal(Limit::new(Some(0)));
        self
    }
}

/// Limit-check
impl Limit {
    /// Return `true` if this limit is depleted, or decrement it by one otherwise.
    ///
    /// `flags` are used to selectively decrement this limit.
    /// Thanks to flag-propagation there can be no runaways.
    pub fn is_exhausted_or_decrement(&mut self, flags: CommitFlags, next: &Queue) -> bool {
        // Keep going if the goal wasn't seen yet, unlimited gas.
        if let Some(maybe_goal) = self.goal_reachable(flags)
            && (maybe_goal.is_empty() || self.set_single_goal_reached_keep_searching(maybe_goal))
        {
            return false;
        }
        // Do not let *any* non-goal tip consume gas as long as there is still anything with a goal in the queue
        // that need to meet their local branches.
        // This is effectively only affecting the entrypoint tips, which isn't setup with a goal.
        // TODO(perf): could we remember that we are a tip and look for our specific counterpart by matching the goal?
        //             That way unrelated tips wouldn't cause us to keep traversing.
        if self.goal_unset() && next.iter().any(|(_, _, _, limit)| !limit.goal_reached()) {
            return false;
        }
        if self.inner.is_some_and(|l| l == 0) {
            return true;
        }
        self.inner = self.inner.map(|l| l - 1);
        false
    }
}

/// Other access and mutation
impl Limit {
    /// Out-of-band way to use commit-flags differently - they never set the earlier flags, so we
    /// can use them.
    /// Return `true` if all goals are reached now.
    pub fn set_single_goal_reached_keep_searching(&mut self, goal: CommitFlags) -> bool {
        self.goal.remove(goal);
        if self.goal.is_empty() {
            self.goal.insert(CommitFlags::Integrated);
            false
        } else {
            true
        }
    }

    /// If `other` has a higher limit as ourselves, apply the higher limit to us.
    /// Nothing else is affected.
    pub fn adjust_limit_if_bigger(&mut self, other: Limit) {
        match (&mut self.inner, other.inner) {
            (inner @ Some(_), None) => *inner = None,
            (Some(x), Some(y)) => {
                if *x < y {
                    *x = y;
                }
            }
            (None, None) | (None, Some(_)) => {}
        }
    }

    pub fn goal_reached(&self) -> bool {
        self.goal_unset() || self.goal.contains(CommitFlags::Integrated)
    }

    fn goal_unset(&self) -> bool {
        self.goal.is_empty()
    }
    /// Return `None` if this limit has no goal set, otherwise return `!CommitFlags::empty()` if `flags` contains it,
    /// meaning it was reached through the commit the flags belong to.
    /// This is useful to determine if a commit that is ahead was seen during traversal.
    #[inline]
    pub fn goal_reachable(&self, flags: CommitFlags) -> Option<CommitFlags> {
        if self.goal_reached() {
            None
        } else {
            Some(flags.intersection(self.goal_flags()))
        }
    }

    /// Return the goal flags, which may be empty.
    pub fn goal_flags(&self) -> CommitFlags {
        // Should only be one, at a time
        let all_goals = self.goal.bits() & !CommitFlags::all().bits();
        CommitFlags::from_bits_retain(all_goals)
    }

    /// Set our limit from `other`, but do not alter our goal.
    pub fn set_but_keep_goal(&mut self, other: Limit) {
        self.inner = other.inner;
    }
}

/// Lifecycle
impl Queue {
    pub fn new_with_limit(limit: Option<usize>) -> Self {
        Queue {
            inner: Default::default(),
            count: 0,
            max: limit,
        }
    }
}

/// A queue to keep track of tips, which additionally counts how much was queued over time.
#[derive(Debug)]
pub struct Queue {
    pub inner: VecDeque<QueueItem>,
    /// The current number of queued items.
    count: usize,
    /// The maximum number of queuing operations, each representing one commit.
    max: Option<usize>,
}

/// Counted queuing
impl Queue {
    /// Sort the queue items so that young commits come first. This way, the traversal goes
    /// back in time continuously, which helps to avoid having too many graph traversals
    /// in disjoint regions happen at the same time.
    /// Note that traversals sorted like this are much less prone to run into the `propagate_flags_downward`
    /// bottleneck. While they may (depending on the graph) create their own bottleneck if they end up missing
    /// their point of interest and overshoot to the beginning of time, this is still preferable over the flag
    /// propagation bottleneck. This is true Particularly if a commit-graph exists which typically is the case
    /// where this starts to matter, as it speeds up traversal by factor 8 easily.
    pub fn sort(&mut self) {
        self.inner
            .make_contiguous()
            .sort_by(|a, b| a.0.gen_then_time.cmp(&b.0.gen_then_time));
    }
    #[must_use]
    pub fn push_back_exhausted(&mut self, item: QueueItem) -> bool {
        self.inner.push_back(item);
        self.is_exhausted_after_increment()
    }
    #[must_use]
    pub fn push_front_exhausted(&mut self, item: QueueItem) -> bool {
        self.inner.push_front(item);
        self.is_exhausted_after_increment()
    }

    fn is_exhausted_after_increment(&mut self) -> bool {
        self.count += 1;
        self.max.is_some_and(|l| self.count >= l)
    }

    /// Return `true` if `id` is on the queue.
    pub fn is_queued(&self, id: gix::ObjectId) -> bool {
        self.inner.iter().any(|(info, _, _, _)| info.id == id)
    }

    /// Add `goal` as additional goal to `id` or panic if `id` was not found.
    pub fn add_goal_to(&mut self, id: gix::ObjectId, goal: CommitFlags) {
        let limit = self
            .inner
            .iter_mut()
            .find_map(|(info, _, _, limit)| (info.id == id).then_some(limit))
            .expect("BUG: id is queued");
        *limit = limit.additional_goal(goal);
    }
}

/// Various other - good to know what we need though.
impl Queue {
    pub fn pop_front(&mut self) -> Option<QueueItem> {
        self.inner.pop_front()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut QueueItem> {
        self.inner.iter_mut()
    }
    pub fn iter(&self) -> impl Iterator<Item = &QueueItem> {
        self.inner.iter()
    }
}
/// A set of commits to keep track of in bitflags.
#[derive(Default)]
pub struct Goals(Vec<gix::ObjectId>);

impl Goals {
    /// Return the bitflag for `goal`, or `None` if we can't track any more goals.
    pub fn flag_for(&mut self, goal: gix::ObjectId) -> Option<CommitFlags> {
        let existing_flags = CommitFlags::all().iter().count();
        let max_goals = size_of::<CommitFlags>() * 8 - existing_flags;

        let goals = &mut self.0;
        let goal_index = match goals.iter().position(|existing| existing == &goal) {
            None => {
                let idx = goals.len();
                goals.push(goal);
                idx
            }
            Some(idx) => idx,
        };
        if goal_index >= max_goals {
            tracing::warn!("Goals limit reached, cannot track {goal}");
            None
        } else {
            Some(CommitFlags::from_bits_retain(
                1 << (existing_flags + goal_index),
            ))
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Instruction {
    /// Contains the segment into which to place this commit.
    CollectCommit { into: SegmentIndex },
    /// This is the first commit in a new segment which is below `parent_above` and which should be placed
    /// at the last commit (at the time) via `at_commit`.
    ConnectNewSegment {
        parent_above: SegmentIndex,
        at_commit: CommitIndex,
    },
}

impl Instruction {
    /// Returns any segment index we may be referring to.
    pub fn segment_idx(&self) -> SegmentIndex {
        match self {
            Instruction::CollectCommit { into } => *into,
            Instruction::ConnectNewSegment { parent_above, .. } => *parent_above,
        }
    }

    pub fn with_replaced_sidx(self, sidx: SegmentIndex) -> Self {
        match self {
            Instruction::CollectCommit { into: _ } => Instruction::CollectCommit { into: sidx },
            Instruction::ConnectNewSegment {
                parent_above: _,
                at_commit,
            } => Instruction::ConnectNewSegment {
                parent_above: sidx,
                at_commit,
            },
        }
    }
}

pub type QueueItem = (super::walk::TraverseInfo, CommitFlags, Instruction, Limit);

#[derive(Debug)]
pub(crate) struct EdgeOwned {
    pub source: SegmentIndex,
    pub target: SegmentIndex,
    pub weight: Edge,
    pub id: petgraph::graph::EdgeIndex,
}

impl From<EdgeReference<'_, Edge>> for EdgeOwned {
    fn from(e: EdgeReference<'_, Edge>) -> Self {
        EdgeOwned {
            source: e.source(),
            target: e.target(),
            weight: *e.weight(),
            id: e.id(),
        }
    }
}

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
pub struct TopoWalk {
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
    /// If this is set during the iteration, we will store segment ids which didn't have any outgoing or
    /// incoming connections, depending on the direction of traversal.
    pub leafs: Option<Vec<SegmentIndex>>,
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
            leafs: None,
        }
    }
}

/// Builder
impl TopoWalk {
    /// Call to not return the tip as part of the iteration.
    pub fn skip_tip_segment(mut self) -> Self {
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

        let mut count = 0;
        for edge in graph.edges_directed(segment, self.direction) {
            count += 1;
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
                    self.next
                        .push_back((edge.source(), edge.weight().src.map(|cidx| cidx + 1)));
                }
            }
        }
        if let Some(leafs) = self.leafs.as_mut().filter(|_| count == 0) {
            leafs.push(segment);
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
