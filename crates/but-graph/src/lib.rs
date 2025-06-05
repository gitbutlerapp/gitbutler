//! A graph data structure for seeing the Git commit graph as segments.

mod segment;
pub use segment::{
    Commit, LocalCommit, LocalCommitRelation, RefLocation, RemoteCommit, StackSegment,
};

/// A graph of connected segments that represent a section of the actual commit-graph.
#[derive(Debug, Default)]
pub struct Graph {
    inner: petgraph::Graph<StackSegment, ()>,
}
