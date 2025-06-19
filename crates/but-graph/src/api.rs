use crate::init::PetGraph;
use crate::{CommitFlags, CommitIndex, Edge, EntryPoint, Graph, Segment, SegmentIndex};
use anyhow::{Context, bail};
use bstr::ByteSlice;
use gix::refs::Category;
use petgraph::Direction;
use petgraph::graph::EdgeReference;
use petgraph::prelude::EdgeRef;
use std::ops::{Index, IndexMut};

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

impl Graph {
    /// Connect two existing segments `src` from `src_commit` to point `dst_commit` of `b`.
    pub(crate) fn connect_segments(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
    ) {
        self.connect_segments_with_ids(src, src_commit, None, dst, dst_commit, None)
    }

    pub(crate) fn connect_segments_with_ids(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        src_id: Option<gix::ObjectId>,
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
                src_id: src_id.or_else(|| self[src].commit_id_by_index(src_commit)),
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

    /// Return all segments that are both [base segments](Self::base_segments) and which
    /// aren't fully defined as traversal stopped due to some abort condition being met.
    /// Valid partial segments always have at least one commit.
    pub fn partial_segments(&self) -> impl Iterator<Item = SegmentIndex> {
        self.base_segments().filter(|s| {
            let has_outgoing = self
                .inner
                .edges_directed(*s, Direction::Outgoing)
                .next()
                .is_some();
            if has_outgoing {
                return false;
            }
            self[*s]
                .commits
                .first()
                .is_none_or(|c| !c.parent_ids.is_empty())
        })
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

    /// Return the number of commits in all segments.
    pub fn num_commits(&self) -> usize {
        self.inner
            .raw_nodes()
            .iter()
            .map(|n| n.weight.commits.len())
            .sum::<usize>()
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

    /// Produce a string that concisely represents `commit`, adding `extra` information as needed.
    pub fn commit_debug_string(
        commit: &crate::Commit,
        has_conflicts: bool,
        is_entrypoint: bool,
        show_message: bool,
        is_early_end: bool,
    ) -> String {
        format!(
            "{ep}{end}{kind}{conflict}{hex}{flags}{msg}{refs}",
            ep = if is_entrypoint { "👉" } else { "" },
            end = if is_early_end { "✂️" } else { "" },
            kind = if commit.flags.contains(CommitFlags::NotInRemote) {
                "·"
            } else {
                "🟣"
            },
            conflict = if has_conflicts { "💥" } else { "" },
            flags = if !commit.flags.is_empty() {
                format!(" ({})", commit.flags.debug_string())
            } else {
                "".to_string()
            },
            hex = commit.id.to_hex_with_len(7),
            msg = if show_message {
                format!("❱{:?}", commit.message.trim().as_bstr())
            } else {
                "".into()
            },
            refs = if commit.refs.is_empty() {
                "".to_string()
            } else {
                format!(
                    " {}",
                    commit
                        .refs
                        .iter()
                        .map(|rn| format!("►{}", { Self::ref_debug_string(rn) }))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )
    }

    /// Shorten the given `name` so it's still clear if it is a special ref (like tag) or not.
    pub fn ref_debug_string(name: &gix::refs::FullName) -> String {
        let (cat, sn) = name.category_and_short_name().expect("valid refs");
        // Only shorten those that look good and are unambiguous enough.
        if matches!(cat, Category::LocalBranch | Category::RemoteBranch) {
            sn
        } else {
            name.as_bstr()
                .strip_prefix(b"refs/")
                .map(|n| n.as_bstr())
                .unwrap_or(name.as_bstr())
        }
        .to_string()
    }

    /// Validate the graph for consistency and fail loudly when an issue was found, after printing the dot graph.
    /// Mostly useful for debugging to stop early when a connection wasn't created correctly.
    #[cfg(unix)]
    pub fn validated_or_open_as_svg(self) -> anyhow::Result<Self> {
        for edge in self.inner.edge_references() {
            let res = check_edge(&self.inner, edge);
            if res.is_err() {
                self.open_as_svg();
            }
            res?;
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

    /// Open an SVG dot visualization in the browser or panic if the `dot` or `open` tool can't be found.
    #[cfg(unix)]
    #[tracing::instrument(skip(self))]
    pub fn open_as_svg(&self) {
        use std::io::Write;
        use std::process::Stdio;
        use std::sync::atomic::AtomicUsize;

        static SUFFIX: AtomicUsize = AtomicUsize::new(0);
        let suffix = SUFFIX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let svg_name = format!("debug-graph-{suffix:02}.svg");
        let mut dot = std::process::Command::new("dot")
            .args(["-Tsvg", "-o", &svg_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("'dot' (graphviz) must be installed on the system");
        dot.stdin
            .as_mut()
            .unwrap()
            .write_all(self.dot_graph().as_bytes())
            .unwrap();
        let mut out = dot.wait_with_output().unwrap();
        out.stdout.extend(out.stderr);
        assert!(
            out.status.success(),
            "dot failed: {out}",
            out = out.stdout.as_bstr()
        );

        assert!(
            std::process::Command::new("open")
                .arg(&svg_name)
                .status()
                .unwrap()
                .success(),
            "Opening of {svg_name} failed"
        );
    }

    /// Return `true` if commit `cidx` in `sidx` is 'cut off', i.e. the traversal finished at this
    /// commit due to an abort condition.
    pub fn is_early_end_of_traversal(&self, sidx: SegmentIndex, cidx: CommitIndex) -> bool {
        if cidx + 1 == self[sidx].commits.len() {
            !self[sidx]
                .commits
                .last()
                .expect("length check above works")
                .parent_ids
                .is_empty()
                && self
                    .inner
                    .edges_directed(sidx, Direction::Outgoing)
                    .next()
                    .is_none()
        } else {
            false
        }
    }

    /// Produces a dot-version of the graph.
    pub fn dot_graph(&self) -> String {
        const HEX: usize = 7;
        let entrypoint = self.entrypoint;
        let node_attrs = |_: &PetGraph, (sidx, s): (SegmentIndex, &Segment)| {
            let name = format!(
                "{}{maybe_centering_newline}",
                s.ref_name
                    .as_ref()
                    .map(Self::ref_debug_string)
                    .unwrap_or_else(|| "<anon>".into()),
                maybe_centering_newline = if s.commits.is_empty() { "" } else { "\n" }
            );
            // Reduce noise by preferring ref-based entry-points.
            let show_segment_entrypoint = s.ref_name.is_some()
                && entrypoint.is_some_and(|(s, cidx)| s == sidx && matches!(cidx, None | Some(0)));
            let commits = s
                .commits
                .iter()
                .enumerate()
                .map(|(cidx, c)| {
                    Self::commit_debug_string(
                        c,
                        c.has_conflicts,
                        !show_segment_entrypoint && Some((sidx, Some(cidx))) == entrypoint,
                        false,
                        self.is_early_end_of_traversal(sidx, cidx),
                    )
                })
                .collect::<Vec<_>>()
                .join("\\l");
            format!(
                ", shape = box, label = \"{entrypoint}:{id}:{name}{commits}\\l\", fontname = Courier, margin = 0.2",
                entrypoint = if show_segment_entrypoint { "👉" } else { "" },
                id = sidx.index(),
            )
        };
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
                format!(", label = \"⚠️{src} → {dst} ({err})\", fontname = Courier")
            },
            &node_attrs,
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
    let first_index = dst.commits.first().map(|_| 0);
    if w.dst != first_index {
        bail!("{w:?}: edge must end on {first_index:?}");
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
