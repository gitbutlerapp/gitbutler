use std::{
    cmp::Reverse,
    collections::{BTreeSet, BinaryHeap},
    ops::{Deref, Index, IndexMut},
};

use anyhow::{Context as _, bail, ensure};
use petgraph::{
    Direction,
    prelude::EdgeRef,
    stable_graph::EdgeReference,
    visit::{IntoEdgeReferences, NodeIndexable, Visitable},
};

use crate::{
    Commit, CommitFlags, CommitIndex, Edge, EntryPoint, EntryPointCommit, Graph, Segment,
    SegmentFlags, SegmentIndex, SegmentRelation, StopCondition,
    init::PetGraph,
    utils::{SegmentTable, SegmentVisitScratch},
    workspace::commit::is_managed_workspace_by_message,
};

boolean_enums::gen_boolean_enum!(pub FirstParent);

/// Mutation
impl Graph {
    /// Insert `segment` to the graph so that it's not connected to any other segment, and return its index.
    ///
    /// Note that as a side effect, the [entrypoint](Self::entrypoint()) will also be set if it's not
    /// set yet.
    pub fn insert_segment_set_entrypoint(&mut self, segment: Segment) -> SegmentIndex {
        let entrypoint = segment
            .commits
            .first()
            .map(|commit| EntryPointCommit::AtCommit(commit.id))
            .unwrap_or(EntryPointCommit::Unborn);
        let index = self.insert_segment(segment);
        if self.entrypoint.is_none() {
            self.entrypoint = Some((index, entrypoint))
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
    /// `parent_order` is the 0-based position of `dst_commit_id` among the source commit's
    /// parents. For a merge commit with parents `[A, B]`, the edge to `A` must use `0`,
    /// and the edge to `B` must use `1`, even if traversal discovers `B` before `A`.
    ///
    /// Return the newly added segment.
    pub fn connect_new_segment(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: Segment,
        dst_commit: impl Into<Option<CommitIndex>>,
        dst_commit_id: impl Into<Option<gix::ObjectId>>,
        parent_order: u32,
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
            parent_order,
        );
        dst
    }
}

/// Merge-base computation
impl Graph {
    /// Determine the ancestry relationship of `a` relative to `b`.
    ///
    /// `Ancestor` means `a` is reachable from `b` when walking towards history,
    /// `Descendant` means the inverse, and `Diverged` means they share history
    /// but neither is ancestor of the other.
    pub fn relation_between(&self, a: SegmentIndex, b: SegmentIndex) -> SegmentRelation {
        if a == b {
            return SegmentRelation::Identity;
        }

        match self.find_merge_base(a, b) {
            Some(base) if base == a => SegmentRelation::Ancestor,
            Some(base) if base == b => SegmentRelation::Descendant,
            Some(_) => SegmentRelation::Diverged,
            None => SegmentRelation::Disjoint,
        }
    }

    /// Like [`Self::relation_between()`], but takes object ids of commits.
    pub fn relation_between_by_commit_id(
        &self,
        commit_a: gix::ObjectId,
        commit_b: gix::ObjectId,
    ) -> anyhow::Result<SegmentRelation> {
        let a = self.segment_id_by_commit_id(commit_a)?;
        let b = self.segment_id_by_commit_id(commit_b)?;
        Ok(self.relation_between(a, b))
    }

    /// Compute the merge-base just like Git would between segments `a` and `b`, but finding all possible merge-bases of a walk,
    /// which are then truncated to the highest merge-base that includes all the other merge-bases.
    ///
    /// Note that this implementation isn't 'stable' and different orders of inputs can change the outcome.
    ///
    /// Returns `None` if there is no merge-base as `a` and `b` don't share history.
    /// If `a == b`, `Some(a)` is returned immediately.
    pub fn find_merge_base(&self, a: SegmentIndex, b: SegmentIndex) -> Option<SegmentIndex> {
        if a == b {
            return Some(a);
        }

        let mut flags = SegmentTable::new(self.inner.node_bound(), SegmentFlags::empty());
        let bases = self.paint_down_to_common(a, b, &mut flags);

        if bases.is_empty() {
            return None;
        }

        let result = self.remove_redundant(&bases, &mut flags);
        result.first().copied()
    }

    /// Like [`Self::find_merge_base()`], but takes object ids of commits,
    /// returning the id of the commit that is the merge-base.
    pub fn find_merge_base_by_commit_id(
        &self,
        commit_a: gix::ObjectId,
        commit_b: gix::ObjectId,
    ) -> anyhow::Result<Option<gix::ObjectId>> {
        let a = self.segment_id_by_commit_id(commit_a)?;
        let b = self.segment_id_by_commit_id(commit_b)?;
        self.find_merge_base(a, b)
            .map(|base| self.commit_id_by_segment(base))
            .transpose()
    }

    /// Return all commits reachable from `included`, but not reachable from `excluded`.
    ///
    /// This is equivalent to the reachable set of `excluded..included`, with segment ids
    /// instead of rev-specs. The returned commits follow the graph traversal order from
    /// `included` towards history.
    ///
    /// Unlike Git's revision walk, this does not sort pending commits by date or enforce
    /// global topo-order across commits. It walks segments by their graph generation,
    /// emits each segment's commits in stored tip-to-base order, and lazily paints the
    /// excluded side only as far as needed to prove emitted segments are not hidden.
    ///
    /// If `first_parent` is [`FirstParent::Yes`], both the included and excluded traversals follow
    /// only segment edges with `parent_order == 0`.
    pub fn find_segments_reachable_from_a_not_b(
        &self,
        included: SegmentIndex,
        excluded: SegmentIndex,
        first_parent: FirstParent,
    ) -> Vec<&Commit> {
        let first_parent: bool = first_parent.into();
        let mut flags = SegmentTable::new(self.inner.node_bound(), SegmentFlags::empty());
        let mut queue = BinaryHeap::new();
        let mut sequence = 0;
        self.queue_segment_for_reachable_difference(
            included,
            false,
            &mut flags,
            &mut queue,
            &mut sequence,
        );
        self.queue_segment_for_reachable_difference(
            excluded,
            true,
            &mut flags,
            &mut queue,
            &mut sequence,
        );

        let mut segments = Vec::new();
        let mut max_emitted_generation = None;
        while let Some((_, _, is_excluded, segment_id)) = queue.pop() {
            if is_excluded {
                let parent_ids =
                    self.parent_segments_for_reachable_difference(segment_id, first_parent);
                for parent_id in parent_ids {
                    self.queue_segment_for_reachable_difference(
                        parent_id,
                        true,
                        &mut flags,
                        &mut queue,
                        &mut sequence,
                    );
                }
                if self.excluded_frontier_is_past_emitted_segments(
                    &queue,
                    &flags,
                    max_emitted_generation,
                ) {
                    break;
                }
                continue;
            }

            if flags.get(segment_id).contains(SegmentFlags::SEGMENT2) {
                continue;
            }

            let generation = self[segment_id].generation;
            max_emitted_generation =
                Some(max_emitted_generation.map_or(generation, |max| max.max(generation)));
            segments.push(segment_id);
            let parent_ids =
                self.parent_segments_for_reachable_difference(segment_id, first_parent);
            for parent_id in parent_ids {
                self.queue_segment_for_reachable_difference(
                    parent_id,
                    false,
                    &mut flags,
                    &mut queue,
                    &mut sequence,
                );
            }
        }

        segments
            .into_iter()
            .filter(|segment_id| !flags.get(*segment_id).contains(SegmentFlags::SEGMENT2))
            .flat_map(|segment_id| self[segment_id].commits.iter())
            .collect()
    }

    /// Queue `segment_id` for the reachable-difference walk if this side has not seen it yet.
    ///
    /// `SEGMENT1` tracks the included side, `SEGMENT2` tracks the excluded side, and `STALE`
    /// is reused here as the excluded-side queued/processed bit. Included segments that are
    /// already known to be excluded are not queued because they cannot contribute to the result.
    /// Queue keys sort by ascending generation and then insertion order so both frontiers advance
    /// from tips toward history in a deterministic segment order.
    fn queue_segment_for_reachable_difference(
        &self,
        segment_id: SegmentIndex,
        is_excluded: bool,
        flags: &mut SegmentTable<SegmentFlags>,
        queue: &mut BinaryHeap<(Reverse<usize>, Reverse<usize>, bool, SegmentIndex)>,
        sequence: &mut usize,
    ) {
        let segment_flags = flags.get_mut(segment_id);
        if is_excluded {
            segment_flags.insert(SegmentFlags::SEGMENT2);
            if segment_flags.contains(SegmentFlags::STALE) {
                return;
            }
            segment_flags.insert(SegmentFlags::STALE);
        } else {
            if segment_flags.intersects(SegmentFlags::SEGMENT1 | SegmentFlags::SEGMENT2) {
                return;
            }
            segment_flags.insert(SegmentFlags::SEGMENT1);
        }

        queue.push((
            Reverse(self[segment_id].generation),
            Reverse(*sequence),
            is_excluded,
            segment_id,
        ));
        *sequence += 1;
    }

    /// Return the parent segments that should be traversed from `segment_id`.
    ///
    /// In all-parent mode this returns every outgoing segment edge. In first-parent mode it
    /// returns only edges whose destination is the source commit's first parent.
    fn parent_segments_for_reachable_difference(
        &self,
        segment_id: SegmentIndex,
        first_parent_only: bool,
    ) -> impl Iterator<Item = SegmentIndex> {
        self.inner
            .edges_directed(segment_id, Direction::Outgoing)
            .filter(move |edge| !first_parent_only || edge.weight().parent_order == 0)
            .map(|edge| edge.target())
    }

    /// Return `true` once the excluded-side frontier cannot still hide any segment already emitted.
    ///
    /// Segment generations are topological: edges point from lower generations toward higher
    /// generations. After all non-hidden included work has left the queue, an excluded frontier
    /// whose minimum generation is greater than the maximum emitted generation can only paint
    /// deeper ancestors, not any emitted segment. At that point the caller may stop walking the
    /// excluded side and filter the segments that were already marked hidden.
    fn excluded_frontier_is_past_emitted_segments(
        &self,
        queue: &BinaryHeap<(Reverse<usize>, Reverse<usize>, bool, SegmentIndex)>,
        flags: &SegmentTable<SegmentFlags>,
        max_emitted_generation: Option<usize>,
    ) -> bool {
        if queue.iter().any(|(_, _, is_excluded, segment_id)| {
            !*is_excluded && !flags.get(*segment_id).contains(SegmentFlags::SEGMENT2)
        }) {
            return false;
        }

        let Some(max_emitted_generation) = max_emitted_generation else {
            return true;
        };
        let Some(min_excluded_generation) = queue
            .iter()
            .filter_map(|(_, _, is_excluded, segment_id)| {
                is_excluded.then_some(self[*segment_id].generation)
            })
            .min()
        else {
            return true;
        };

        min_excluded_generation > max_emitted_generation
    }

    /// Return all commit ids reachable from `included`, but not reachable from `excluded`.
    ///
    /// This is a convenience wrapper around
    /// [`Self::find_segments_reachable_from_a_not_b()`], taking object ids of commits.
    /// If `first_parent` is [`FirstParent::Yes`], both traversals follow only first-parent edges.
    pub fn find_commit_ids_reachable_from_a_not_b(
        &self,
        included: gix::ObjectId,
        excluded: gix::ObjectId,
        first_parent: FirstParent,
    ) -> anyhow::Result<Vec<gix::ObjectId>> {
        let included = self.segment_id_by_commit_id(included)?;
        let excluded = self.segment_id_by_commit_id(excluded)?;
        Ok(self
            .find_segments_reachable_from_a_not_b(included, excluded, first_parent)
            .into_iter()
            .map(|commit| commit.id)
            .collect())
    }

    /// Compute an octopus merge-base from multiple `segments`.
    ///
    /// The first segment becomes the initial candidate. Each following segment is
    /// folded into that candidate by computing their pairwise
    /// [`Self::find_merge_base()`]. If any pair does not share history, there
    /// is no merge-base common to all segments.
    ///
    /// Returns `None` if `segments` is empty.
    /// If `segments` has one element, it returns that.
    pub fn find_merge_base_octopus(
        &self,
        segments: impl IntoIterator<Item = SegmentIndex>,
    ) -> Option<SegmentIndex> {
        let mut segments = segments.into_iter();
        let first = segments.next()?;
        segments.try_fold(first, |base, segment| self.find_merge_base(base, segment))
    }

    /// Like [`Self::find_merge_base_octopus()`], but works with object ids of `commits`,
    /// returning the id of the commit that is the merge-base.
    pub fn find_merge_base_octopus_by_commit_id(
        &self,
        commits: impl IntoIterator<Item = gix::ObjectId>,
    ) -> anyhow::Result<Option<gix::ObjectId>> {
        let mut segments = Vec::new();
        for commit_id in commits {
            segments.push(self.segment_id_by_commit_id(commit_id)?);
        }
        self.find_merge_base_octopus(segments)
            .map(|base| self.commit_id_by_segment(base))
            .transpose()
    }

    /// Return `(commit, owner_sidx_of_commit)` for `start` as long as it can unambiguously be attributed
    /// to belong to the segment at `start` even if it doesn't own it.
    ///
    /// Empty virtual segments can be considered to *point* to a commit even if they don't own it as
    /// long as it can be found by following the only outgoing edge of `start` and subsequent
    /// segments. This lets real refs resolve even when another segment was prioritized to own the
    /// shared commit.
    ///
    /// This helper intentionally stops at ambiguous segments with more than one outgoing connection.
    pub fn resolve_to_unambiguously_pointed_to_commit(
        &self,
        start: SegmentIndex,
    ) -> Option<(&crate::Commit, SegmentIndex)> {
        if let Some(commit) = self[start].commits.first() {
            return Some((commit, start));
        }

        let mut current = start;
        let mut seen = BTreeSet::new(); // SeenTable isn't worth it here.
        while seen.insert(current) {
            let mut parents = self.inner.neighbors_directed(current, Direction::Outgoing);
            let Some(parent) = parents.next() else {
                tracing::warn!(
                    start = start.index(),
                    current = current.index(),
                    "Could not resolve empty segment as it has no outgoing parent segment"
                );
                return None;
            };
            if parents.next().is_some() {
                tracing::warn!(
                    start = start.index(),
                    current = current.index(),
                    "Could not resolve empty segment as it has multiple outgoing parent segments"
                );
                return None;
            }

            current = parent;
            if let Some(commit) = self[current].commits.first() {
                return Some((commit, current));
            }
        }
        tracing::warn!(
            start = start.index(),
            current = current.index(),
            "Could not resolve empty segment as traversal ended, there were only empty segments or none at all"
        );
        None
    }

    fn commit_id_by_segment(&self, segment: SegmentIndex) -> anyhow::Result<gix::ObjectId> {
        self.tip_skip_empty(segment)
            .map(|commit| commit.id)
            .with_context(|| {
                format!("BUG: Segment {segment:?} does not contain a reachable tip commit")
            })
    }

    /// Return the id of the segment that owns `commit_id`, or error if it wasn't found.
    /// That is unexpected as the traversal is supposed to find all commits of interest.
    pub fn segment_id_by_commit_id(
        &self,
        commit_id: gix::ObjectId,
    ) -> anyhow::Result<SegmentIndex> {
        self.segment_by_commit_id(commit_id).map(|s| s.id)
    }

    /// Return the segment that owns `commit_id`, or error if it wasn't found.
    /// That is unexpected as the traversal is supposed to find all commits of interest.
    pub fn segment_by_commit_id(&self, commit_id: gix::ObjectId) -> anyhow::Result<&Segment> {
        self.inner
            .node_weights()
            .find(|s| s.commits.iter().any(|c| c.id == commit_id))
            .with_context(|| {
                format!("Commit {commit_id} not found in any segment, it wasn't traversed")
            })
    }

    /// Paint segments reachable from `first` with SEGMENT1 and from `second` with SEGMENT2.
    /// When a segment has both flags, it's a potential merge-base.
    /// Returns all potential merge-bases with their generation numbers.
    fn paint_down_to_common(
        &self,
        first: SegmentIndex,
        second: SegmentIndex,
        flags: &mut SegmentTable<SegmentFlags>,
    ) -> Vec<(SegmentIndex, usize)> {
        // Priority queue ordered by generation (higher generation = closer to root = lower priority).
        // We use Reverse because BinaryHeap is a max-heap and we want segments with *lower* generation
        // (i.e. closer to tips) to be processed first.
        let mut queue: BinaryHeap<(Reverse<usize>, SegmentIndex)> = BinaryHeap::new();

        // Initialize first segment
        let first_flags = flags.get_mut(first);
        *first_flags |= SegmentFlags::SEGMENT1;
        queue.push((Reverse(self[first].generation), first));

        // Initialize second segment
        let second_flags = flags.get_mut(second);
        *second_flags |= SegmentFlags::SEGMENT2;
        queue.push((Reverse(self[second].generation), second));

        let mut out = Vec::new();

        // Keep processing while there are potentially useful entries.
        //
        // Stale entries still need to propagate their stale marker to their
        // parents if other non-stale queue entries remain. Once everything left
        // in the queue is stale, no better merge-base can be found.
        while queue
            .iter()
            .any(|(_, segment_id)| !flags.get(*segment_id).contains(SegmentFlags::STALE))
        {
            let Some((Reverse(generation), segment_id)) = queue.pop() else {
                break;
            };
            let segment_flags = flags.get(segment_id);

            let mut flags_without_result = segment_flags
                & (SegmentFlags::SEGMENT1 | SegmentFlags::SEGMENT2 | SegmentFlags::STALE);

            // If reachable from both sides, it's a merge-base candidate
            if flags_without_result == (SegmentFlags::SEGMENT1 | SegmentFlags::SEGMENT2) {
                if !segment_flags.contains(SegmentFlags::RESULT) {
                    flags.get_mut(segment_id).insert(SegmentFlags::RESULT);
                    out.push((segment_id, generation));
                }
                flags_without_result |= SegmentFlags::STALE;
            }

            // Propagate flags to parents (outgoing direction = towards history)
            for parent_id in self
                .inner
                .neighbors_directed(segment_id, Direction::Outgoing)
            {
                let parent_flags = flags.get_mut(parent_id);
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
        flags: &mut SegmentTable<SegmentFlags>,
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
            flags.get_mut(*sidx).insert(SegmentFlags::RESULT);

            for parent_id in self.inner.neighbors_directed(*sidx, Direction::Outgoing) {
                let parent_flags = flags.get_mut(parent_id);
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
            flags.get_mut(*sidx).remove(SegmentFlags::STALE);
        }

        let mut count_still_independent = segments.len();
        let mut stack: Vec<(SegmentIndex, usize)> = Vec::new();

        while let Some((segment_id, segment_gen)) = walk_start.pop() {
            if count_still_independent <= 1 {
                break;
            }

            stack.clear();
            flags.get_mut(segment_id).insert(SegmentFlags::STALE);
            stack.push((segment_id, segment_gen));

            while let Some((current_id, current_gen)) = stack.last().copied() {
                let current_flags = flags.get(current_id);

                if current_flags.contains(SegmentFlags::RESULT) {
                    flags.get_mut(current_id).remove(SegmentFlags::RESULT);
                    count_still_independent -= 1;

                    if count_still_independent <= 1 {
                        break;
                    }

                    // Update min_gen if we just removed the minimum
                    if current_id == sorted_segments[min_gen_pos].0 {
                        while min_gen_pos < segments.len() - 1
                            && flags
                                .get(sorted_segments[min_gen_pos].0)
                                .contains(SegmentFlags::STALE)
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
                    let parent_flags = flags.get_mut(parent_id);
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
                (!flags.get(*sidx).contains(SegmentFlags::STALE)).then_some(*sidx)
            })
            .collect()
    }
}

/// # Points of interest
impl Graph {
    /// Return the entry-point commit of this graph if it is a
    /// [managed](is_managed_workspace_by_message) workspace commit.
    ///
    /// Note that managed workspace commits are owned by GitButler.
    /// The `repo` is used to look up the entrypoint commit and to obtain its message
    /// and only return it if it seems to be owned by GitButler.
    pub fn managed_entrypoint_commit(
        &self,
        repo: &gix::Repository,
    ) -> anyhow::Result<Option<&Commit>> {
        let Some(ec) = self.entrypoint()?.commit() else {
            return Ok(None);
        };

        let commit = repo.find_commit(ec.id)?;
        let message = commit.message_raw()?;
        Ok(is_managed_workspace_by_message(message).then_some(ec))
    }

    /// Return the entry-point of the graph as configured during traversal.
    /// It's useful for when one wants to know which commit was used to discover the entire graph.
    ///
    /// Note that this method only fails if the entrypoint wasn't set correctly due to a bug.
    pub fn entrypoint(&self) -> anyhow::Result<EntryPoint<'_>> {
        let (segment_index, commit) = self
            .entrypoint
            .context("BUG: must always set the entrypoint")?;
        let segment = self.inner.node_weight(segment_index).with_context(|| {
            format!("BUG: entrypoint segment at {segment_index:?} wasn't present")
        })?;
        let commit_and_owner = match commit {
            EntryPointCommit::Unborn => None,
            EntryPointCommit::AtCommit(id) => {
                // We don't check invariants here and are more flexible than we have to,
                // validation takes care of the details.
                if let Some(t) = segment.commit_by_id(id).map(|c| (c, segment)) {
                    Some(t)
                } else {
                    let owner = self.segment_by_commit_id(id)?;
                    let commit = owner.commit_by_id(id)
                        .with_context(|| {
                            format!(
                                "BUG: owner segment {owner_id:?} did not contain remembered entrypoint commit {id}",
                                owner_id = owner.id
                            )
                        })?;
                    Some((commit, owner))
                }
            }
        };
        Ok(EntryPoint {
            segment,
            commit_and_owner,
        })
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
    pub fn segment_by_ref_name(&self, name: &gix::refs::FullNameRef) -> Option<&Segment> {
        self.inner
            .node_weights()
            .find(|s| s.ref_name().is_some_and(|rn| rn == name))
    }

    /// Starting at `segment`, return the commit it owns, or the commit it
    /// unambiguously points to through empty segments.
    ///
    /// Empty virtual segments can stand in for refs whose commit is owned by a
    /// different segment. This follows the only outgoing connection through any
    /// subsequent empty segments until it reaches the first non-empty segment.
    ///
    /// Returns `None` if any empty segment on the path has zero or multiple
    /// outgoing connections, as there is no unambiguous commit to return.
    pub fn tip_skip_empty(&self, segment: SegmentIndex) -> Option<&Commit> {
        self.resolve_to_unambiguously_pointed_to_commit(segment)
            .map(|(c, _)| c)
    }

    /// Visit the ancestry of `start` along the first parents, itself excluded, until `stop` returns `true`.
    /// Also return the segment that we stopped at.
    /// **Important**: `stop` is not called with `start`, this is a feature.
    pub fn visit_segments_downward_along_first_parent_exclude_start(
        &self,
        start: SegmentIndex,
        stop: impl FnMut(&Segment) -> bool,
    ) {
        self.visit_segments_downward_along_first_parent(start, false, stop);
    }

    /// Visit the ancestry of `start` along the first parents, including `start`, until `stop` returns `true`.
    pub fn visit_segments_downward_along_first_parent_include_start(
        &self,
        start: SegmentIndex,
        stop: impl FnMut(&Segment) -> bool,
    ) {
        self.visit_segments_downward_along_first_parent(start, true, stop);
    }

    fn visit_segments_downward_along_first_parent(
        &self,
        start: SegmentIndex,
        include_start: bool,
        mut stop: impl FnMut(&Segment) -> bool,
    ) -> Option<SegmentIndex> {
        let mut next = if include_start {
            Some(start)
        } else {
            self.inner
                .edges_directed(start, Direction::Outgoing)
                .next()
                .map(|edge| edge.target())
        };
        let mut seen = self.seen_table();
        while let Some(sidx) = next {
            let segment = &self[sidx];
            if stop(segment) {
                return Some(sidx);
            }
            next = if seen.insert_unseen(sidx) {
                self.inner
                    .edges_directed(sidx, Direction::Outgoing)
                    .next()
                    .map(|edge| edge.target())
            } else {
                None
            };
        }
        None
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
        self.stop_condition(sidx)
            .is_some_and(|condition| condition.is_unnatural())
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
        self.inner
            .edges_directed(sidx, Direction::Outgoing)
            .filter_map(|edge| {
                let dst = edge.weight().dst;
                dst.is_none_or(|dst| dst == 0)
                    .then_some((edge.weight().src, edge.target()))
            })
    }

    /// Return the condition under which traversal stopped at `sidx`,
    /// or `None` if the traversal didn't stop.
    pub fn stop_condition(&self, sidx: SegmentIndex) -> Option<StopCondition> {
        if self
            .inner
            .edges_directed(sidx, Direction::Outgoing)
            .next()
            .is_some()
        {
            return None;
        }
        let commit = self[sidx].commits.last()?;
        let mut condition = StopCondition::empty();
        if commit.parent_ids.is_empty() {
            condition |= StopCondition::FirstCommit;
        }
        if commit.flags.contains(CommitFlags::ShallowBoundary) {
            condition |= StopCondition::ShallowBoundary;
        }
        if !commit.parent_ids.is_empty() && !condition.contains(StopCondition::ShallowBoundary) {
            condition |= StopCondition::Limit;
        }
        (!condition.is_empty()).then_some(condition)
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
        visit_and_prune: impl FnMut(&Segment) -> bool,
    ) {
        let mut scratch = SegmentVisitScratch::new(self);
        scratch.visit_including_start_until(self, start, direction, visit_and_prune);
    }

    /// Visit all segments, excluding `start`, until `visit_and_prune(segment)` returns `true`.
    /// Pruned segments aren't returned and not traversed, but note that `visit_and_prune` may
    /// be called multiple times until the traversal stops.
    pub fn visit_all_segments_excluding_start_until(
        &self,
        start: SegmentIndex,
        direction: Direction,
        visit_and_prune: impl FnMut(&Segment) -> bool,
    ) {
        let mut scratch = SegmentVisitScratch::new(self);
        scratch.visit_excluding_start_until(self, start, direction, visit_and_prune);
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
    /// Final-graph invariants are skipped if the traversal hit the hard limit,
    /// as these graphs are explicitly partial.
    pub fn validated(self) -> anyhow::Result<Self> {
        if !self.hard_limit_hit {
            self.check_entrypoint_invariants()?;
            for segment_index in self.inner.node_indices() {
                let segment = &self.inner[segment_index];
                let outgoing = self
                    .inner
                    .neighbors_directed(segment_index, Direction::Outgoing)
                    .count();
                self.check_virtual_segments_are_empty_and_connected(
                    segment_index,
                    segment,
                    outgoing,
                )?;
                self.check_multi_parent_tip_or_ancestor_segments_have_commits(
                    segment_index,
                    segment,
                    outgoing,
                )?;
            }
            for tip in &self.traversal_tips {
                self.check_traversal_tip_points_to_first_commit(tip)?;
            }
        }
        for edge in self.inner.edge_references() {
            Self::check_edge(&self.inner, edge, false)?;
        }
        Ok(self)
    }

    /// Validate the graph for consistency and return all errors.
    ///
    /// If the graph didn't hit the hard limit, this checks:
    ///
    /// - The graph entrypoint exists, points to an existing segment, matches the
    ///   remembered entrypoint ref when one exists, and remembers a commit id
    ///   that is still represented in the graph.
    /// - Virtual segments are empty, named graph nodes for real Git refs whose
    ///   commit is owned elsewhere, and connected according to their
    ///   GitButler-only role.
    /// - Tip and ancestor segments with multiple parents own at least one commit.
    /// - Traversal tips resolve to the first commit they describe.
    ///
    /// This always checks:
    ///
    /// - Edge weights still match the source and destination segment endpoints.
    pub fn validation_errors(&self) -> Vec<anyhow::Error> {
        let mut out = Vec::new();
        if !self.hard_limit_hit {
            out.extend(self.check_entrypoint_invariants().err());
            for segment_index in self.inner.node_indices() {
                let segment = &self.inner[segment_index];
                let outgoing = self
                    .inner
                    .neighbors_directed(segment_index, Direction::Outgoing)
                    .count();
                out.extend(
                    [
                        self.check_virtual_segments_are_empty_and_connected(
                            segment_index,
                            segment,
                            outgoing,
                        ),
                        self.check_multi_parent_tip_or_ancestor_segments_have_commits(
                            segment_index,
                            segment,
                            outgoing,
                        ),
                    ]
                    .into_iter()
                    .flat_map(Result::err),
                );
            }
            out.extend(
                self.traversal_tips
                    .iter()
                    .filter_map(|tip| self.check_traversal_tip_points_to_first_commit(tip).err()),
            );
        }
        out.extend(
            self.inner
                .edge_references()
                .filter_map(|edge| Self::check_edge(&self.inner, edge, false).err()),
        );
        out
    }

    /// The entrypoint is the user-facing traversal anchor.
    ///
    /// It must always point at an existing segment in completed graphs. If a
    /// ref name was remembered for it, post-processing must have moved the
    /// entrypoint to the segment with that name.
    ///
    /// If the entrypoint remembers a commit id, that id must either be the first
    /// commit of its segment or be owned elsewhere as the first commit of another segment.
    ///
    /// The latter is valid for empty virtual workspace tip segments: they can fan out to
    /// multiple stack segments and cannot resolve through a unique outgoing
    /// edge, but the original traversal commit is still known.
    fn check_entrypoint_invariants(&self) -> anyhow::Result<()> {
        let (entrypoint_sidx, entrypoint_commit) = self
            .entrypoint
            .context("completed graph must have an entrypoint")?;
        let segment = self
            .inner
            .node_weight(entrypoint_sidx)
            .with_context(|| format!("entrypoint segment at {entrypoint_sidx:?} wasn't present"))?;

        if let Some(entrypoint_ref) = self.entrypoint_ref.as_ref() {
            ensure!(
                segment
                    .ref_name()
                    .is_some_and(|rn| rn == entrypoint_ref.as_ref()),
                "{entrypoint_sidx:?}: entrypoint segment must be named {entrypoint_ref}, got {actual:?}",
                actual = segment.ref_name()
            );
        }

        match entrypoint_commit {
            EntryPointCommit::Unborn => {
                ensure!(
                    segment.commits.is_empty(),
                    "{entrypoint_sidx:?}: unborn entrypoint segment must not contain commits"
                );
            }
            EntryPointCommit::AtCommit(id) => {
                if let Some(first_commit) = segment.commits.first() {
                    ensure!(
                        first_commit.id == id,
                        "{entrypoint_sidx:?}: entrypoint segment first commit is {actual}, not remembered entrypoint commit {id}",
                        actual = first_commit.id
                    );
                } else {
                    ensure!(
                        segment.ref_name().is_some(),
                        "{entrypoint_sidx:?}: empty entrypoint segment with remembered commit {id} must be named"
                    );
                    let owner_segment = self.segment_by_commit_id(id).with_context(|| {
                        format!(
                            "{entrypoint_sidx:?}: empty entrypoint segment remembers {id}, but no segment owns that commit"
                        )
                    })?;
                    ensure!(
                        owner_segment.commit_index_of(id) == Some(0),
                        "{entrypoint_sidx:?}: empty entrypoint segment remembers {id}, but owner {owner_segment_id:?} does not have it as first commit",
                        owner_segment_id = owner_segment.id
                    );
                }
            }
        }
        Ok(())
    }

    /// Virtual segments are empty graph nodes that correspond to real Git
    /// references whose commits are owned by another segment.
    ///
    /// Ordinary virtual segments can resolve to the commit named by their refs by
    /// following their outgoing edge with
    /// [`Self::resolve_to_unambiguously_pointed_to_commit()`]. Virtual workspace
    /// tip segments are the exception: they can fan out to multiple stack tips
    /// and are therefore ambiguous. Such segments are empty because the commit
    /// they name is owned elsewhere, sometimes because another segment was
    /// prioritized to own the commit when multiple refs point to it. They are
    /// virtual because their relationships to other segments are not represented
    /// by the Git commit-graph or references. To Git, these are refs pointing at
    /// the same commit; GitButler sees one or more ordered stacks of branches.
    fn check_virtual_segments_are_empty_and_connected(
        &self,
        segment_index: SegmentIndex,
        segment: &Segment,
        outgoing: usize,
    ) -> anyhow::Result<()> {
        if !segment.commits.is_empty() {
            return Ok(());
        }

        let is_virtual_workspace_tip = segment.workspace_metadata().is_some();
        if is_virtual_workspace_tip {
            ensure!(
                segment.ref_name().is_some(),
                "{segment_index:?}: virtual workspace tip segment must be named - we don't want empty anonymous segments"
            );
            ensure!(
                outgoing > 0,
                "{segment_index:?}: virtual workspace tip segment must have at least one outgoing connection"
            );
        } else if segment.ref_name().is_some() {
            ensure!(
                outgoing == 1,
                "{segment_index:?}: virtual segment must have exactly one outgoing connection, got {outgoing}"
            );
        }
        Ok(())
    }

    /// Tip and ancestor segments with multiple parents must own a commit.
    ///
    /// Empty virtual workspace tip segments are excluded because they can fan
    /// out from one real workspace ref to multiple stack tips.
    fn check_multi_parent_tip_or_ancestor_segments_have_commits(
        &self,
        segment_index: SegmentIndex,
        segment: &Segment,
        outgoing: usize,
    ) -> anyhow::Result<()> {
        if segment.workspace_metadata().is_some() {
            return Ok(());
        }

        ensure!(
            outgoing <= 1 || !segment.commits.is_empty(),
            "{segment_index:?}: tip or ancestor segment with {outgoing} outgoing connections must own at least one commit"
        );
        Ok(())
    }

    /// A retained traversal tip must resolve to its first commit.
    ///
    /// If the segment named by the tip is empty, following its single outgoing
    /// edge chain must reach a non-empty segment whose first commit is the tip
    /// id. This validates the final-graph form of the initial tip-segment
    /// invariant.
    fn check_traversal_tip_points_to_first_commit(
        &self,
        tip: &crate::init::Tip,
    ) -> anyhow::Result<()> {
        if let Ok(segment_owning_tip_id) = self.segment_by_commit_id(tip.id) {
            ensure!(
                segment_owning_tip_id.commit_index_of(tip.id) == Some(0),
                "{tip:?}: tip segment {segment_id:?} must contain the tip id as its first commit",
                segment_id = segment_owning_tip_id.id
            );
            return Ok(());
        }

        let Some(ref_name) = tip.ref_name.as_ref() else {
            bail!(
                "{tip:?}: tip id is not owned by any segment, but it must as no segment was found by commit-id"
            );
        };
        let segment = self
            .segment_by_ref_name(ref_name.as_ref())
            .with_context(|| format!("{tip:?}: tip ref {ref_name} is not owned by any segment"))?;
        ensure!(
            segment.commits.is_empty(),
            "{tip:?}: named tip segment {segment_id:?} must be empty if it does not own the tip id",
            segment_id = segment.id
        );
        let (commit, owner_segment_index) = self
            .resolve_to_unambiguously_pointed_to_commit(segment.id)
            .with_context(|| {
                format!(
                    "{tip:?}: empty tip segment {segment_id:?} must resolve through one outgoing path to a commit",
                    segment_id = segment.id
                )
            })?;
        ensure!(
            commit.id == tip.id,
            "{tip:?}: empty tip segment {segment_id:?} resolves to {actual}, not {expected}",
            segment_id = segment.id,
            actual = commit.id,
            expected = tip.id
        );
        // Extra-check - this invariant is also enforced by `resolve_to_unambiguously_pointed_to_commit()`.
        ensure!(
            self[owner_segment_index].commit_index_of(tip.id) == Some(0),
            "{tip:?}: resolved tip owner {owner_segment_index:?} must contain the tip id as its first commit"
        );
        Ok(())
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
