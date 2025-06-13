use crate::init::PetGraph;
use crate::{CommitIndex, Edge, EntryPoint, Graph, Segment, SegmentIndex};
use anyhow::{Context, bail};
use petgraph::Direction;
use petgraph::graph::EdgeReference;
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
        self.inner[dst].id = dst.index();
        self.connect_segments_with_dst_id(src, src_commit, dst, dst_commit, dst_commit_id.into());
        dst
    }
}

impl Graph {
    /// Connect two existing segments `src` from `src_commit` to point `dst_commit` of `b`.
    pub(crate) fn connect_segments(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
    ) {
        self.connect_segments_with_dst_id(src, src_commit, dst, dst_commit, None)
    }

    pub(crate) fn connect_segments_with_dst_id(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
        dst_id: Option<gix::ObjectId>,
    ) {
        let src_commit = src_commit.into();
        let dst_commit = dst_commit.into();
        self.inner.add_edge(
            src,
            dst,
            Edge {
                src: src_commit,
                src_id: self[src].commit_id_by_index(src_commit),
                dst: dst_commit,
                dst_id: dst_id.or_else(|| self[dst].commit_id_by_index(dst_commit)),
            },
        );
    }
}

/// Query
impl Graph {
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
    /// Validate the graph for consistency and fail loudly when an issue was found.
    /// Use this before using the graph for anything serious, but particularly in testing.
    // TODO: maybe make this mandatory as part of post-processing.
    pub fn validated(self) -> anyhow::Result<Self> {
        for edge in self.inner.edge_references() {
            check_edge(&self.inner, edge)?;
        }
        Ok(self)
    }
    /// Output this graph in dot-format to stderr to allow copying it, and using like this for visualization:
    ///
    /// ```shell
    /// pbpaste | dot -Tsvg >graph.svg && open graph.svg
    /// ```
    ///
    /// Note that this may reveal additional debug information when invariants of the graph are violated.
    /// This often is more useful than seeing a hard error, which can be achieved with `Self::validated()`
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
                // Don't mark connections from the last commit to the first one,
                // but those that are 'splitting' a segment. These shouldn't exist.
                let Err(err) = check_edge(g, e) else {
                    return ", label = \"\"".into();
                };
                let e = e.weight();
                let src = src
                    .commit_id_by_index(e.src)
                    .map(|c| c.to_hex_with_len(HEX).to_string())
                    .unwrap_or_else(|| "src".into());
                let dst = dst
                    .commit_id_by_index(e.dst)
                    .map(|c| c.to_hex_with_len(HEX).to_string())
                    .unwrap_or_else(|| "dst".into());
                format!(", label = \"⚠️{src} → {dst} ({err})\"")
            },
            &|_, (sidx, s)| {
                format!(
                    ", shape = box, label = \":{id}:{name}\n{commits}\"",
                    id = sidx.index(),
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

/// Fail with an error if the `edge` isn't consistent.
fn check_edge(graph: &PetGraph, edge: EdgeReference<'_, Edge>) -> anyhow::Result<()> {
    let e = edge;
    let src = &graph[e.source()];
    let dst = &graph[e.target()];
    let w = e.weight();
    if w.src != src.last_commit_index() {
        bail!(
            "{w:?}: edge must start on last commit {last:?}",
            last = src.last_commit_index()
        );
    }
    if w.dst.unwrap_or_default() != 0 {
        bail!("{w:?}: edge must end on first commit 0");
    }

    let seg_cidx = src.commit_id_by_index(w.src);
    if w.src_id != seg_cidx {
        bail!("{w:?}: the desired source index didn't match the one in the segment {seg_cidx:?}");
    }
    let seg_cidx = dst.commit_id_by_index(w.dst);
    if w.dst_id != seg_cidx {
        bail!(
            "{w:?}: the desired destination index didn't match the one in the segment {seg_cidx:?}"
        );
    }
    Ok(())
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
