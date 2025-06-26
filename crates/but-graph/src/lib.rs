//! A graph data structure for seeing the Git commit graph as segments.
//!
//! This crate is part of the Correctness and Stability initiative.
//!
//! ### Before the Graph
//!
//! The application traditionally displays commits in lanes while allowing them to be segmented. Today a lane is a
//! stack of segments. This is great for users as it's easy to understand, but there is a problem with it: it's a complete lie.
//!
//! To generate stacks, one performed a graph traversal, following only the first parent, down to the merge-base of the workspace
//! commit with the target branch. It's clear how this degenerates information especially around merges that the application
//! helps to create by allowing to merge with the target branch.
//!
//! When trying to rebase the workspace onto updated target branches though, it still exclusively operates in stacks, ignoring
//! the complexities of the underlying graph, and the illusion starts to break down.
//!
//! Besides that, there is an inherent mental issue when programming with data structures that degenerate information, as the
//! resulting program will inherently be unfit for the task as it's based on over-simplified assumptions baked into its very core.
//!
//! With the current data structures, achieving correctness simply isn't possible.
//!
//! ### The Graph - the Solution
//!
//! The graph solves this problem by simplifying the commit-graph and optimizing it for traversal, without oversimplifying the
//! underlying commit-graph. That way, the notion of stacks with segments is merely a view of the graph specifically prepared
//! for presentation.
//!
//! When operations are supposed to be performed, we can now do things like this `git rebase --preserve-merges origin/main`,
//! creating a correctly ordered list of operations to do the transplantation correctly. Thanks to the additional metadata
//! collected into the Graph I'd expect us to be able to whatever it takes without limitation.
//!
//! Besides that, operating on a graph, despite its own complexities, is finally aligning the mental model of the programmer
//! with what's actually there, for algorithms suitable to perform the job correctly.
//!
//! All this makes the Graph the **new core data-structure** that is the world of GitButler and upon which visualisations and
//! mutation operations are based.
#![forbid(unsafe_code)]
#![deny(missing_docs, rust_2018_idioms)]

mod segment;
pub use segment::{Commit, CommitFlags, Segment, SegmentMetadata};

mod api;
/// Produce a graph from a Git repository.
pub mod init;
pub mod projection;

mod ref_metadata_legacy;
pub use ref_metadata_legacy::{VirtualBranchesTomlMetadata, is_workspace_ref_name};

mod statistics;
pub use statistics::Statistics;

mod debug;

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
    ///
    /// It's usually the first commit of the segment due to the way we split segments, and even though
    /// downstream code relies on this properly, the graph itself does not.
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
