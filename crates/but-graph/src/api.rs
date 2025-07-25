use crate::init::PetGraph;
use crate::{CommitIndex, Edge, EntryPoint, Graph, Segment, SegmentIndex};
use anyhow::{Context, bail};
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::EdgeReference;
use petgraph::visit::{IntoEdgeReferences, Visitable};
use std::collections::{BTreeSet, VecDeque};
use std::ops::{Deref, Index, IndexMut};

/// Mutation
impl Graph {
    /// Insert `segment` to the graph so that it's not connected to any other segment, and return its index.
    pub fn insert_root(&mut self, segment: Segment) -> SegmentIndex {
        let index = self.inner.add_node(segment);
        self.inner[index].id = index;
        if self.entrypoint.is_none() {
            self.entrypoint = Some((index, None))
        }
        index
    }

    /// Put `dst` on top of `src`, connecting it from the `src_commit` specifically,
    /// an index valid for [`Segment::commits_unique_from_tip`] in `src` to the commit at `dst_commit` in `dst`.
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

/// Query
impl Graph {
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
    /// [`Segment::commits_unique_from_tip`] field of `sidx`.
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
