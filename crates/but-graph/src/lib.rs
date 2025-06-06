//! A graph data structure for seeing the Git commit graph as segments.

mod segment;

pub use segment::{Commit, LocalCommit, LocalCommitRelation, RefLocation, RemoteCommit, Segment};

/// Edges to other segments are the index into the list of local commits of the parent segment.
/// That way we can tell where a segment branches off, despite the graph only connecting segments, and not commits.
pub type CommitIndex = usize;

/// A graph of connected segments that represent a section of the actual commit-graph.
#[derive(Debug, Default)]
pub struct Graph {
    inner: petgraph::Graph<Segment, Option<CommitIndex>>,
}

/// An index into the [`Graph`].
pub type SegmentIndex = petgraph::graph::NodeIndex;

mod api;

mod ref_metadata_legacy;
pub use ref_metadata_legacy::{VirtualBranchesTomlMetadata, is_workspace_ref_name};
