//! A vector-backed Git commit and reference graph with workspace projection.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod types;
pub use types::{BoundaryKind, CommitFlags, RefInfo, ReferenceMetadata, Worktree, WorktreeKind};

mod node;
pub(crate) use node::NodeGraph;
pub use node::{
    Node, NodeGraph as Graph, NodeGraphEntrypoint, NodeIndex, NodeKind, Reference, child_most,
    children_of, collect_ordered_parents, expansion_slots, is_commit_like, resolve_to_commit,
    topological_order,
};

/// Mutate a graph and rewrite history: `NodeGraph -> MutableNodeGraph ->
/// rebase() -> NodeGraph -> materialize_changes()`.
pub mod edit;
pub use edit::{MutableNodeGraph, Rebased};

/// Construct a graph from repository commits, references, and workspace metadata.
pub mod init;

#[path = "projection/mod.rs"]
pub mod workspace;
pub use workspace::Workspace;
