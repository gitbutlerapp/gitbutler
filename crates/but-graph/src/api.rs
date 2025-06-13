use crate::{CommitIndex, Edge, EntryPoint, Graph, Segment, SegmentIndex};
use anyhow::{Context, bail};
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use std::ops::{Index, IndexMut};

/// Mutation
impl Graph {
    /// Insert `segment` to the graph so that it's not connected to any other segment, and return its index.
    pub fn insert_root(&mut self, segment: Segment) -> SegmentIndex {
        let index = self.inner.add_node(segment);
        self.inner[index].id = index.index();
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
    /// Return the newly added segment.
    pub fn connect_new_segment(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: Segment,
        dst_commit: impl Into<Option<CommitIndex>>,
    ) -> SegmentIndex {
        let dst = self.inner.add_node(dst);
        self.inner[dst].id = dst.index();
        self.connect_segments(src, src_commit, dst, dst_commit);
        dst
    }

    /// Just like [`Self::connect_new_segment()`], but assures that commit-constraints are upheld.
    pub fn connect_new_segment_validated(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: Segment,
        dst_commit: impl Into<Option<CommitIndex>>,
    ) -> anyhow::Result<SegmentIndex> {
        let dst = self.inner.add_node(dst);
        self.inner[dst].id = dst.index();
        self.connect_segments_validated(src, src_commit, dst, dst_commit)?;
        Ok(dst)
    }

    /// Connect two existing segments `src` from `src_commit` to point `dst_commit` of `b`.
    pub fn connect_segments(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
    ) {
        self.inner.add_edge(
            src,
            dst,
            Edge {
                src: src_commit.into(),
                dst: dst_commit.into(),
            },
        );
    }

    /// Connect two existing segments `src` from `src_commit` to point `dst_commit` of `b`.
    /// Assure `src_commit` is truly the last commit in `src` and that `dst_commit` is the first.
    pub fn connect_segments_validated(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
    ) -> anyhow::Result<()> {
        let src_commit = src_commit.into();
        if self[src].last_commit_index() != src_commit {
            bail!(
                "Source segment {src:?} tried to connect {src_commit:?}, which isn't the last one"
            );
        }
        let dst_commit = dst_commit.into();
        if dst_commit.unwrap_or_default() != 0 {
            bail!(
                "Destination segment {dst:?} tried to receive connection to commit {dst_commit:?}, but must be the first one"
            );
        }
        self.connect_segments(src, src_commit, dst, dst_commit);
        Ok(())
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
}

/// Query
impl Graph {
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

    /// Return all segments that sit on top of the `sidx` segment as `(source_commit_index(of sidx), destination_segment_index)`,
    /// along with the exact commit at which the segment branches off as seen from `sidx`, usually the last one.
    /// Also, **this will only return those segments where the incoming connection points to their first commit**.
    /// Note that a single `CommitIndex` can link to multiple segments, as happens with merge-commits.
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
            .filter_map(|edge| {
                let dst = edge.weight().dst;
                dst.is_none_or(|dst| dst == 0)
                    .then_some((edge.weight().src, edge.target()))
            })
    }

    /// Return the number of segments stored within the graph.
    pub fn num_segments(&self) -> usize {
        self.inner.node_count()
    }

    /// Return the number of edges that are connecting segments.
    pub fn num_edges(&self) -> usize {
        self.inner.edge_count()
    }

    /// Return an iterator over all indices of segments in the graph.
    pub fn segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.inner.node_indices()
    }
}

/// Debugging
impl Graph {
    /// Output this graph in dot-format to stderr to allow copying it, and using like this for visualization:
    ///
    /// ```shell
    /// pbpaste | dot -Tsvg >graph.svg && open graph.svg
    /// ```
    pub fn eprint_dot_graph(&self) {
        let dot = self.dot_graph();
        eprintln!("{dot}");
    }

    /// Produces a dot-version of the graph.
    pub fn dot_graph(&self) -> String {
        const HEX: usize = 7;
        let dot = petgraph::dot::Dot::with_attr_getters(
            &self.inner,
            &[],
            &|g, e| {
                let src = &g[e.source()];
                let dst = &g[e.target()];
                let e = e.weight();
                let src = src
                    .commit_by_index(e.src)
                    .map(|c| c.id.to_hex_with_len(HEX).to_string())
                    .unwrap_or_else(|| "src".into());
                let dst = dst
                    .commit_by_index(e.dst)
                    .map(|c| c.id.to_hex_with_len(HEX).to_string())
                    .unwrap_or_else(|| "dst".into());
                format!(", label = \"{src} → {dst}\"")
            },
            &|_, (_, s)| {
                format!(
                    ", shape = box, label = \"{name}\n{commits}\"",
                    name = s
                        .ref_name
                        .as_ref()
                        .map(|rn| rn.shorten())
                        .unwrap_or_else(|| "<anon>".into()),
                    commits = s
                        .commits
                        .iter()
                        .map(|c| c.id.to_hex_with_len(HEX).to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            },
        );
        format!("{dot:?}")
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
