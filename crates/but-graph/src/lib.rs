//! A graph data structure for seeing the Git commit graph as segments.
#![forbid(unsafe_code)]
#![deny(missing_docs, rust_2018_idioms)]

mod segment;
pub use segment::{Commit, CommitFlags, Segment, SegmentMetadata};

/// Edges to other segments are the index into the list of local commits of the parent segment.
/// That way we can tell where a segment branches off, despite the graph only connecting segments, and not commits.
pub type CommitIndex = usize;

/// A graph of connected segments that represent a section of the actual commit-graph.
#[derive(Default, Debug)]
pub struct Graph {
    inner: init::PetGraph,
    /// From where the graph was created. This is useful if one wants to focus on a subset of the graph.
    ///
    /// The [`CommitIndex`] is empty if the entry point is an empty segment, one that is supposed to receive
    /// commits later.
    entrypoint: Option<(SegmentIndex, Option<CommitIndex>)>,
    /// It's `true` only if we have stopped the traversal due to a hard limit.
    hard_limit_hit: bool,
}

/// A resolved entry point into the graph for easy access to the segment, commit,
/// and the respective indices for later traversal.
#[derive(Debug, Copy, Clone)]
pub struct EntryPoint<'graph> {
    /// The index to the segment that served starting point for the traversal into this graph.
    pub segment_index: SegmentIndex,
    /// If present, the index of the commit that started the traversal in the segment denoted by `segment_index`.
    pub commit_index: Option<CommitIndex>,
    /// The segment that served starting point for the traversal into this graph.
    pub segment: &'graph Segment,
    /// If present, the commit that started the traversal in the `segment`.
    pub commit: Option<&'graph Commit>,
}

/// This structure is used as data associated with each edge and is mainly for collecting
/// the intent of an edge, which should always represent the connection of a commit to another.
/// Sometimes, it represents the connection from a commit (or segment) to an empty segment which
/// doesn't yet have a commit.
/// The idea is to write code that keeps edge information consistent, and our visualization tools hightlights
/// issues with the inherent invariants.
#[derive(Debug, Copy, Clone)]
struct Edge {
    /// `None` if the source segment has no commit.
    src: Option<CommitIndex>,
    /// The commit id at `src` in the segment commit list.
    src_id: Option<gix::ObjectId>,
    dst: Option<CommitIndex>,
    /// The commit id at `dst` in the segment commit list.
    dst_id: Option<gix::ObjectId>,
}

/// An index into the [`Graph`].
pub type SegmentIndex = petgraph::graph::NodeIndex;

mod api;
/// Produce a graph from a Git repository.
pub mod init;

mod ref_metadata_legacy;
pub use ref_metadata_legacy::{VirtualBranchesTomlMetadata, is_workspace_ref_name};
