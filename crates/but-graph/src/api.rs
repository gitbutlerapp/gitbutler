use crate::{CommitIndex, Graph, Segment, SegmentIndex};
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use std::ops::Index;

/// Mutation
impl Graph {
    /// Insert `segment` to the graph so that it's not connected to any other segment, and return its index.
    pub fn insert_root(&mut self, segment: Segment) -> SegmentIndex {
        self.inner.add_node(segment)
    }

    /// Put `segment` on top of `base`, connecting it at the `commit_index` specifically,
    /// an index valid for [`Segment::commits_unique_from_tip`].
    /// If `commit_index` is `None`, there must be no commit in `base` and it's connected directly,
    /// something that can happen for the root base of the graph which is usually empty.
    /// This is as if a tree would be growing upwards.
    ///
    /// Return the newly added segment.
    pub fn fork_on_top(
        &mut self,
        base: SegmentIndex,
        commit_index: impl Into<Option<CommitIndex>>,
        segment: Segment,
    ) -> SegmentIndex {
        let upper = self.inner.add_node(segment);
        self.inner.add_edge(base, upper, commit_index.into());
        upper
    }
}

/// Query
impl Graph {
    /// Return all segments which have no other segments below them, making them bases.
    ///
    /// Typically, there is only one, but there can be multiple.
    pub fn base_segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.inner.externals(Direction::Incoming)
    }

    /// Return all segments that sit on top of the segment identified by `sidx`, along with the exact commit at which
    /// the segment branches off as seen from `sidx`. Note that one `CommitIndex` can link to multiple segments.
    ///
    /// Thus, a [`CommitIndex`] of `0` indicates the paired segment sits directly on top of `sidx`, probably as part of
    /// a merge commit that is the last commit in the respective segment. The index is always valid in the
    /// [`Segment::commits_unique_from_tip`] field of `sidx`.
    ///
    /// Note that they are in reverse order, i.e., the segments that were added last will be returned first.
    pub fn segments_on_top(
        &self,
        sidx: SegmentIndex,
    ) -> impl Iterator<Item = (Option<CommitIndex>, SegmentIndex)> {
        self.inner
            .edges_directed(sidx, Direction::Outgoing)
            .map(|edge| (*edge.weight(), edge.target()))
    }

    /// Return the number of segments stored within the graph.
    pub fn num_segments(&self) -> usize {
        self.inner.node_count()
    }

    /// Return an iterator over all indices of segments in the graph.
    pub fn segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.inner.node_indices()
    }
}

impl Index<SegmentIndex> for Graph {
    type Output = Segment;

    fn index(&self, index: SegmentIndex) -> &Self::Output {
        &self.inner[index]
    }
}
