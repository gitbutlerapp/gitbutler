use std::{
    cmp::Reverse,
    collections::{BTreeSet, BinaryHeap, HashMap, VecDeque},
    ops::{Deref, Index, IndexMut},
};

use anyhow::{Context as _, bail};
use petgraph::{
    Direction,
    prelude::EdgeRef,
    stable_graph::EdgeReference,
    visit::{IntoEdgeReferences, Visitable},
};

use crate::{
    Commit, CommitIndex, Edge, EntryPoint, Graph, Segment, SegmentFlags, SegmentIndex,
    init::PetGraph, projection::commit::is_managed_workspace_by_message,
};

/// Mutation
impl Graph {
    /// Insert `segment` to the graph so that it's not connected to any other segment, and return its index.
    ///
    /// Note that as a side effect, the [entrypoint](Self::lookup_entrypoint()) will also be set if it's not
    /// set yet.
    pub fn insert_segment_set_entrypoint(&mut self, segment: Segment) -> SegmentIndex {
        let index = self.insert_segment(segment);
        if self.entrypoint.is_none() {
            self.entrypoint = Some((index, None))
        }
        index
    }

    /// Insert `segment` to the graph so that it's not connected to any other segment, and return its index.
    pub fn insert_segment(&mut self, segment: Segment) -> SegmentIndex {
        let index = self.inner.add_node(segment);
        self.inner[index].id = index;
        index
    }

    /// Put `dst` on top of `src`, connecting it from the `src_commit` specifically,
    /// an index valid for [`Segment::commits`] in `src` to the commit at `dst_commit` in `dst`.
    ///
    /// If `src_commit` is `None`, there must be no commit in `base` and it's connected directly,
    /// something that can happen for the root base of the graph which is usually empty.
    /// This is as if a tree would be growing upwards, but it's a matter of perspective really, there
    /// is no up and down.
    ///
    /// `dst_commit_id` can be provided if the connection is to a future commit that isn't yet available
    /// in the `segment`. If `None`, it will be looked up in the `segment` itself.
    ///
    /// Return the newly added segment.
    pub fn connect_new_segment(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: Segment,
        dst_commit: impl Into<Option<CommitIndex>>,
        dst_commit_id: impl Into<Option<gix::ObjectId>>,
    ) -> SegmentIndex {
        let dst = self.inner.add_node(dst);
        self.inner[dst].id = dst;
        self.connect_segments_with_ids(
            src,
            src_commit,
            None,
            dst,
            dst_commit,
            dst_commit_id.into(),
        );
        dst
    }
}

/// Merge-base computation
impl Graph {
    /// Compute the merge-base just like Git would between segments `a` and `b`, but finding all possible merge-bases of a walk,
    /// which are then truncated to the highest merge-base that includes all the other merge-bases.
    ///
    /// Note that this implementation isn't 'stable' and different orders of inputs can change the outcome.
    ///
    /// Returns `None` if there is no merge-base as `a` and `b` don't share history.
    /// If `a == b`, `Some(a)` is returned immediately.
    pub fn find_git_merge_base(&self, a: SegmentIndex, b: SegmentIndex) -> Option<SegmentIndex> {
        if a == b {
            return Some(a);
        }

        let mut flags: HashMap<SegmentIndex, SegmentFlags> = Default::default();
        let bases = self.paint_down_to_common(a, b, &mut flags);

        if bases.is_empty() {
            return None;
        }

        let result = self.remove_redundant(&bases, &mut flags);
        result.first().copied()
    }

    /// Paint segments reachable from `first` with SEGMENT1 and from `second` with SEGMENT2.
    /// When a segment has both flags, it's a potential merge-base.
    /// Returns all potential merge-bases with their generation numbers.
    fn paint_down_to_common(
        &self,
        first: SegmentIndex,
        second: SegmentIndex,
        flags: &mut HashMap<SegmentIndex, SegmentFlags>,
    ) -> Vec<(SegmentIndex, usize)> {
        // Priority queue ordered by generation (higher generation = closer to root = lower priority).
        // We use Reverse because BinaryHeap is a max-heap and we want segments with *lower* generation
        // (i.e. closer to tips) to be processed first.
        let mut queue: BinaryHeap<(Reverse<usize>, SegmentIndex)> = BinaryHeap::new();

        // Initialize first segment
        let first_flags = flags.entry(first).or_insert(SegmentFlags::empty());
        *first_flags |= SegmentFlags::SEGMENT1;
        queue.push((Reverse(self[first].generation), first));

        // Initialize second segment
        let second_flags = flags.entry(second).or_insert(SegmentFlags::empty());
        *second_flags |= SegmentFlags::SEGMENT2;
        queue.push((Reverse(self[second].generation), second));

        let mut out = Vec::new();

        // Continue while there are non-stale segments in the queue
        while queue.iter().any(|(_, sidx)| {
            !flags
                .get(sidx)
                .is_some_and(|f| f.contains(SegmentFlags::STALE))
        }) {
            let Some((Reverse(generation), segment_id)) = queue.pop() else {
                break;
            };

            let segment_flags = *flags.get(&segment_id).unwrap_or(&SegmentFlags::empty());
            let mut flags_without_result = segment_flags
                & (SegmentFlags::SEGMENT1 | SegmentFlags::SEGMENT2 | SegmentFlags::STALE);

            // If reachable from both sides, it's a merge-base candidate
            if flags_without_result == (SegmentFlags::SEGMENT1 | SegmentFlags::SEGMENT2) {
                if !segment_flags.contains(SegmentFlags::RESULT) {
                    flags
                        .entry(segment_id)
                        .or_default()
                        .insert(SegmentFlags::RESULT);
                    out.push((segment_id, generation));
                }
                flags_without_result |= SegmentFlags::STALE;
            }

            // Propagate flags to parents (outgoing direction = towards history)
            for parent_id in self
                .inner
                .neighbors_directed(segment_id, Direction::Outgoing)
            {
                let parent_flags = flags.entry(parent_id).or_insert(SegmentFlags::empty());
                if (*parent_flags & flags_without_result) != flags_without_result {
                    *parent_flags |= flags_without_result;
                    queue.push((Reverse(self[parent_id].generation), parent_id));
                }
            }
        }

        out
    }

    /// Remove all those segments from `segments` if they are in the history of another segment in `segments`.
    /// That way, we return only the topologically most recent segments in `segments`.
    fn remove_redundant(
        &self,
        segments: &[(SegmentIndex, usize)],
        flags: &mut HashMap<SegmentIndex, SegmentFlags>,
    ) -> Vec<SegmentIndex> {
        if segments.is_empty() {
            return Vec::new();
        }

        // Clear flags for the redundancy check
        flags.clear();

        let sorted_segments = {
            let mut v = segments.to_vec();
            // Sort by generation ascending (lower generation first = closer to tips)
            v.sort_by_key(|(_, generation)| *generation);
            v
        };

        let mut min_gen_pos = 0;
        let mut min_gen = sorted_segments[min_gen_pos].1;

        let mut walk_start: Vec<(SegmentIndex, usize)> = Vec::with_capacity(segments.len());

        // Mark all input segments with RESULT and collect their parents for walking
        for (sidx, _) in segments {
            flags.entry(*sidx).or_default().insert(SegmentFlags::RESULT);

            for parent_id in self.inner.neighbors_directed(*sidx, Direction::Outgoing) {
                let parent_flags = flags.entry(parent_id).or_insert(SegmentFlags::empty());
                // Prevent double-addition
                if !parent_flags.contains(SegmentFlags::STALE) {
                    parent_flags.insert(SegmentFlags::STALE);
                    walk_start.push((parent_id, self[parent_id].generation));
                }
            }
        }

        walk_start.sort_by_key(|(sidx, _)| sidx.index());

        // Allow walking everything at first (remove STALE from walk_start entries)
        for (sidx, _) in &walk_start {
            if let Some(f) = flags.get_mut(sidx) {
                f.remove(SegmentFlags::STALE);
            }
        }

        let mut count_still_independent = segments.len();
        let mut stack: Vec<(SegmentIndex, usize)> = Vec::new();

        while let Some((segment_id, segment_gen)) = walk_start.pop() {
            if count_still_independent <= 1 {
                break;
            }

            stack.clear();
            flags
                .entry(segment_id)
                .or_default()
                .insert(SegmentFlags::STALE);
            stack.push((segment_id, segment_gen));

            while let Some((current_id, current_gen)) = stack.last().copied() {
                let current_flags = *flags.get(&current_id).unwrap_or(&SegmentFlags::empty());

                if current_flags.contains(SegmentFlags::RESULT) {
                    if let Some(f) = flags.get_mut(&current_id) {
                        f.remove(SegmentFlags::RESULT);
                    }
                    count_still_independent -= 1;

                    if count_still_independent <= 1 {
                        break;
                    }

                    // Update min_gen if we just removed the minimum
                    if current_id == sorted_segments[min_gen_pos].0 {
                        while min_gen_pos < segments.len() - 1
                            && flags
                                .get(&sorted_segments[min_gen_pos].0)
                                .is_some_and(|f| f.contains(SegmentFlags::STALE))
                        {
                            min_gen_pos += 1;
                        }
                        min_gen = sorted_segments[min_gen_pos].1;
                    }
                }

                // Skip if generation is below minimum
                if current_gen > min_gen {
                    stack.pop();
                    continue;
                }

                let previous_len = stack.len();

                for parent_id in self
                    .inner
                    .neighbors_directed(current_id, Direction::Outgoing)
                {
                    let parent_flags = flags.entry(parent_id).or_insert(SegmentFlags::empty());
                    if !parent_flags.contains(SegmentFlags::STALE) {
                        parent_flags.insert(SegmentFlags::STALE);
                        stack.push((parent_id, self[parent_id].generation));
                    }
                }

                if previous_len == stack.len() {
                    stack.pop();
                }
            }
        }

        // Return segments that are not marked as STALE
        segments
            .iter()
            .filter_map(|(sidx, _)| {
                flags
                    .get(sidx)
                    .filter(|f| !f.contains(SegmentFlags::STALE))
                    .map(|_| *sidx)
            })
            .collect()
    }
}

/// Query
/// ‼️Useful only if one knows the graph traversal was started where one expects, or else the graph may be partial.
impl Graph {
    /// Return the `(segment, commit)` that is either named `name`,
    /// or has a commit with `name` in its [refs](Commit::refs).
    /// The returned `commit` is the commit at which the reference with `name`
    /// is pointing to directly or indirectly.
    ///
    /// Note that tags may or may not be included in the graph, depending on how it was created.
    ///
    /// ### Performance
    ///
    /// This is a brute-force search through all nodes and all data in the graph - beware of hot-loop usage.
    pub fn segment_and_commit_by_ref_name(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> Option<(&Segment, &Commit)> {
        self.inner.node_weights().find_map(|s| {
            if s.ref_name().is_some_and(|rn| rn == name) {
                self.tip_skip_empty(s.id).map(|c| (s, c))
            } else {
                s.commits.iter().find_map(|c| {
                    c.refs
                        .iter()
                        .any(|ri| ri.ref_name.as_ref() == name)
                        .then_some((s, c))
                })
            }
        })
    }

    /// Return the segment that is named `name`,
    ///
    /// Note that tags may or may not be included in the graph, depending on how it was created.
    ///
    /// ### Performance
    ///
    /// This is a brute-force search through all nodes and all data in the graph - beware of hot-loop usage.
    pub fn named_segment_by_ref_name(&self, name: &gix::refs::FullNameRef) -> Option<&Segment> {
        self.inner
            .node_weights()
            .find(|s| s.ref_name().is_some_and(|rn| rn == name))
    }

    /// Starting a `segment`, ignore all segments that have no commit and return the first commit
    /// of a non-empty segment.
    ///
    /// This is useful to counter the fact that multiple empty segments could be stacked, to ultimately
    /// point to a segment that owns the commit.
    ///
    /// Note that we will **visit the first parent only**.
    pub fn tip_skip_empty(&self, segment: SegmentIndex) -> Option<&Commit> {
        if let Some(tip) = self[segment].commits.first() {
            return Some(tip);
        }

        let mut sidx_with_commits = None;
        self.visit_segments_downward_along_first_parent_exclude_start(segment, |s| {
            if s.commits.is_empty() {
                return false;
            }
            sidx_with_commits = Some(s.id);
            true
        });
        sidx_with_commits.and_then(|sidx| self[sidx].commits.first())
    }

    /// The first commit reachable by skipping over empty segments starting at the entrypoint segment.
    pub fn entrypoint_commit(&self) -> Option<&Commit> {
        self.tip_skip_empty(self.entrypoint?.0)
    }

    /// Return the entry-point commit of this graph if it is a
    /// [managed](is_managed_workspace_by_message) workspace commit.
    ///
    /// The entry-point commit is obtained via [`Self::entrypoint_commit()`].
    /// Note that managed workspace commits are owned by GitButler.
    /// The `repo` is used to look up the entrypoint commit and to obtain its message.
    pub fn managed_entrypoint_commit(
        &self,
        repo: &gix::Repository,
    ) -> anyhow::Result<Option<&Commit>> {
        let Some(ec) = self.entrypoint_commit() else {
            return Ok(None);
        };

        let commit = repo.find_commit(ec.id)?;
        let message = commit.message_raw()?;
        Ok(is_managed_workspace_by_message(message).then_some(ec))
    }

    /// Visit the ancestry of `start` along the first parents, itself excluded, until `stop` returns `true`.
    /// Also return the segment that we stopped at.
    /// **Important**: `stop` is not called with `start`, this is a feature.
    ///
    /// Note that the traversal assumes as well-segmented graph without cycles.
    pub fn visit_segments_downward_along_first_parent_exclude_start(
        &self,
        start: SegmentIndex,
        mut stop: impl FnMut(&Segment) -> bool,
    ) {
        let mut edge = self.inner.edges_directed(start, Direction::Outgoing).last();
        let mut seen = BTreeSet::new();
        while let Some(first_edge) = edge {
            let next = &self[first_edge.target()];
            if stop(next) {
                break;
            }
            if seen.insert(next.id) {
                edge = self
                    .inner
                    .edges_directed(next.id, Direction::Outgoing)
                    .last();
            }
        }
    }

    /// Return `true` if this graph is possibly partial as the hard limit was hit,
    /// meaning that the core traversal algorithm was interrupted without necessarily
    /// satisfying all constraints.
    ///
    /// Such a graph is possibly partial, which can affect algorithms
    /// relying on it being complete.
    pub fn hard_limit_hit(&self) -> bool {
        self.hard_limit_hit
    }

    /// Claim that the graph was pruned without regard to the core graph algorithm.
    pub fn set_hard_limit_hit(&mut self) {
        self.hard_limit_hit = true;
    }

    /// Lookup the segment of `sidx` and then find its sibling segment, if it has one.
    pub fn lookup_sibling_segment(&self, sidx: SegmentIndex) -> Option<&Segment> {
        self.inner
            .node_weight(self.inner.node_weight(sidx)?.sibling_segment_id?)
    }

    /// Lookup the segment of `sidx` and then find its remote tracking branch segment, if it has one.
    pub fn lookup_remote_tracking_branch_segment(&self, sidx: SegmentIndex) -> Option<&Segment> {
        self.inner.node_weight(
            self.inner
                .node_weight(sidx)?
                .remote_tracking_branch_segment_id?,
        )
    }

    /// Return the entry-point of the graph as configured during traversal.
    /// It's useful for when one wants to know which commit was used to discover the entire graph.
    ///
    /// Note that this method only fails if the entrypoint wasn't set correctly due to a bug.
    pub fn lookup_entrypoint(&self) -> anyhow::Result<EntryPoint<'_>> {
        let (segment_index, commit_index) = self
            .entrypoint
            .context("BUG: must always set the entrypoint")?;
        let segment = &self.inner.node_weight(segment_index).with_context(|| {
            format!("BUG: entrypoint segment at {segment_index:?} wasn't present")
        })?;
        Ok(EntryPoint {
            segment_index,
            commit_index,
            segment,
            commit: commit_index.and_then(|idx| segment.commits.get(idx)),
        })
    }
    /// Return all segments which have no other segments *above* them, making them tips.
    ///
    /// Typically, there is only one, but there *can* be multiple technically.
    pub fn tip_segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.inner.externals(Direction::Incoming)
    }

    /// Return all segments which have no other segments *below* them, making them bases.
    ///
    /// Typically, there is only one, but there can easily be multiple.
    pub fn base_segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.inner.externals(Direction::Outgoing)
    }

    /// Return all segments that are both [base segments](Self::base_segments) and which
    /// aren't fully defined as traversal stopped due to some abort condition being met.
    /// Valid partial segments always have at least one commit.
    pub fn partial_segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.base_segments().filter(|s| self.is_partial_segment(*s))
    }

    /// Return `true` if the segment behind `sidx`
    /// isn't fully defined as traversal stopped due to some abort condition.
    /// Valid partial segments always have at least one commit.
    fn is_partial_segment(&self, sidx: SegmentIndex) -> bool {
        let has_outgoing = self
            .inner
            .edges_directed(sidx, Direction::Outgoing)
            .next()
            .is_some();
        if has_outgoing {
            return false;
        }
        self[sidx]
            .commits
            .last()
            .is_none_or(|c| !c.parent_ids.is_empty())
    }

    /// Return all segments that sit on top of the `sidx` segment as `(source_commit_index(of sidx), destination_segment_index)`,
    /// along with the exact commit at which the segment branches off as seen from `sidx`, usually the last one.
    /// Also, **this will only return those segments where the incoming connection points to their first commit**.
    /// Note that a single `CommitIndex` can link to multiple segments, as happens with merge-commits.
    ///
    /// Thus, a [`CommitIndex`] of `0` indicates the paired segment sits directly on top of `sidx`, probably as part of
    /// a merge commit that is the last commit in the respective segment. The index is always valid in the
    /// [`Segment::commits`] field of `sidx`.
    pub fn segments_below_in_order(
        &self,
        sidx: SegmentIndex,
    ) -> impl Iterator<Item = (Option<CommitIndex>, SegmentIndex)> {
        self.edges_directed_in_order_of_creation(sidx, Direction::Outgoing)
            .into_iter()
            .filter_map(|edge| {
                let dst = edge.weight().dst;
                dst.is_none_or(|dst| dst == 0)
                    .then_some((edge.weight().src, edge.target()))
            })
    }

    /// Just like [petgraph::Graph::edges_directed()](petgraph::Graph::edges_directed()), but it returns the edges
    /// in the order in which they were added, and *not* in reverse.
    ///
    /// Use this whenever you need to maintain a certain order of operation.
    pub fn edges_directed_in_order_of_creation(
        &self,
        sidx: SegmentIndex,
        direction: Direction,
    ) -> Vec<EdgeReference<'_, Edge>> {
        let mut edges: Vec<_> = self.inner.edges_directed(sidx, direction).collect();
        edges.reverse();
        edges
    }

    /// Return `true` if commit `sidx` is 'cut off', i.e. the traversal finished at
    /// its last commit due to an abort condition.
    pub fn is_early_end_of_traversal(&self, sidx: SegmentIndex) -> bool {
        if self
            .inner
            .edges_directed(sidx, Direction::Outgoing)
            .next()
            .is_some()
        {
            return false;
        }
        self[sidx]
            .commits
            .last()
            .is_some_and(|c| !c.parent_ids.is_empty())
    }

    /// Return the number of segments stored within the graph.
    pub fn num_segments(&self) -> usize {
        self.inner.node_count()
    }

    /// Return the number of edges that are connecting segments.
    pub fn num_connections(&self) -> usize {
        self.inner.edge_count()
    }

    /// Return the number of commits in all segments.
    pub fn num_commits(&self) -> usize {
        self.inner
            .node_indices()
            .map(|n| self[n].commits.len())
            .sum::<usize>()
    }

    /// Return an iterator over all indices of segments in the graph.
    pub fn segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.inner.node_indices()
    }

    /// Visit all segments, including `start`, until `visit_and_prune(segment)` returns `true`.
    /// Pruned segments aren't returned and not traversed, but note that `visit_and_prune` may
    /// be called multiple times until the traversal stops.
    pub fn visit_all_segments_including_start_until(
        &self,
        start: SegmentIndex,
        direction: Direction,
        mut visit_and_prune: impl FnMut(&Segment) -> bool,
    ) {
        let mut next = VecDeque::new();
        next.push_back(start);
        let mut seen = BTreeSet::new();
        while let Some(next_sidx) = next.pop_front() {
            if !visit_and_prune(&self[next_sidx]) {
                next.extend(
                    self.inner
                        .neighbors_directed(next_sidx, direction)
                        .filter(|n| seen.insert(*n)),
                )
            }
        }
    }

    /// Visit all segments, excluding `start`, until `visit_and_prune(segment)` returns `true`.
    /// Pruned segments aren't returned and not traversed, but note that `visit_and_prune` may
    /// be called multiple times until the traversal stops.
    pub fn visit_all_segments_excluding_start_until(
        &self,
        start: SegmentIndex,
        direction: Direction,
        mut visit_and_prune: impl FnMut(&Segment) -> bool,
    ) {
        let mut next = VecDeque::new();
        next.push_back(start);
        let mut seen = BTreeSet::new();
        while let Some(next_sidx) = next.pop_front() {
            if start == next_sidx || !visit_and_prune(&self[next_sidx]) {
                next.extend(
                    self.inner
                        .neighbors_directed(next_sidx, direction)
                        .filter(|n| seen.insert(*n)),
                )
            }
        }
    }
}

/// Query
///
/// The query relies on the segmentation of the graph being as advertised, something we assure as part
/// of the initial creation.
impl Graph {
    /// Return a utility to perform topological walks on the graph.
    pub fn topo_walk(&self) -> petgraph::visit::Topo<SegmentIndex, <PetGraph as Visitable>::Map> {
        petgraph::visit::Topo::new(&self.inner)
    }
}

/// Validation
impl Graph {
    /// Validate the graph for consistency and fail loudly when an issue was found.
    /// Use this before using the graph for anything serious, but particularly in testing.
    // TODO: maybe make this mandatory as part of post-processing.
    pub fn validated(self) -> anyhow::Result<Self> {
        for edge in self.inner.edge_references() {
            Self::check_edge(&self.inner, edge, false)?;
        }
        Ok(self)
    }

    /// Validate the graph for consistency and return all errors.
    pub fn validation_errors(&self) -> Vec<anyhow::Error> {
        self.inner
            .edge_references()
            .filter_map(|edge| Self::check_edge(&self.inner, edge, false).err())
            .collect()
    }

    /// Fail with an error if the `edge` isn't consistent.
    pub(crate) fn check_edge(
        graph: &PetGraph,
        edge: EdgeReference<'_, Edge>,
        weight_only: bool,
    ) -> anyhow::Result<()> {
        let e = edge;
        let src = &graph[e.source()];
        let dst = &graph[e.target()];
        let w = e.weight();
        let display = if weight_only {
            w as &dyn std::fmt::Debug
        } else {
            &e as &dyn std::fmt::Debug
        };
        if w.src != src.last_commit_index() {
            bail!(
                "{display:?}: edge must start on last commit {last:?}",
                last = src.last_commit_index()
            );
        }
        let first_index = dst.commits.first().map(|_| 0);
        if w.dst != first_index {
            bail!("{display:?}: edge must end on {first_index:?}");
        }

        let seg_cidx = src.commit_id_by_index(w.src);
        if w.src_id != seg_cidx {
            bail!(
                "{display:?}: the desired source index didn't match the one in the segment {seg_cidx:?}"
            );
        }
        let seg_cidx = dst.commit_id_by_index(w.dst);
        if w.dst_id != seg_cidx {
            bail!(
                "{display:?}: the desired destination index didn't match the one in the segment {seg_cidx:?}"
            );
        }
        Ok(())
    }
}

impl Index<SegmentIndex> for Graph {
    type Output = Segment;

    fn index(&self, index: SegmentIndex) -> &Self::Output {
        &self.inner[index]
    }
}

impl IndexMut<SegmentIndex> for Graph {
    fn index_mut(&mut self, index: SegmentIndex) -> &mut Self::Output {
        &mut self.inner[index]
    }
}

impl Deref for Graph {
    type Target = PetGraph;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// This in particular is only for those who know what they are doing.
impl std::ops::DerefMut for Graph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
