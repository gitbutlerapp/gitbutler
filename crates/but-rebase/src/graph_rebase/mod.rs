#![deny(missing_docs)]
//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darknes bind them.

use petgraph::graph::NodeIndex;

mod creation;
pub mod rebase;
pub use creation::GraphExt;
pub mod cherry_pick;

/// Utilities for testing
pub mod testing;

/// Describes what action the engine should take
#[derive(Debug, Clone)]
pub enum Step {
    /// Cherry picks the given commit into the new location in the graph
    Pick {
        /// The ID of the commit getting picked
        id: gix::ObjectId,
    },
    /// Represents applying a reference to the commit found at it's first parent
    Reference {
        /// The refname
        refname: gix::refs::FullName,
    },
    /// Used as a placeholder after removing a pick or reference
    None,
}

/// Used to represent a connection between a given commit.
#[derive(Debug, Clone)]
struct Edge {
    /// Represents in which order the `parent` fields should be written out
    ///
    /// A child commit should have edges that all have unique orders. In order
    /// to achive that we can employ the following semantics.
    ///
    /// When replacing a given parent with N other parents, the first in that list takes the old parent's order, and the rest take the
    order: usize,
}

type StepGraphIndex = petgraph::stable_graph::NodeIndex;
type StepGraph = petgraph::stable_graph::StableDiGraph<Step, Edge>;

/// Points to a step in the rebase editor.
#[derive(Debug, Clone)]
pub struct Selector {
    id: NodeIndex<StepGraphIndex>,
}

/// Used to manipulate a set of picks.
#[derive(Debug, Clone)]
pub struct Editor {
    /// The internal graph of steps
    graph: StepGraph,
    /// Initial references. This is used to track any references that might need
    /// deleted.
    initial_references: Vec<gix::refs::FullName>,
    heads: Vec<StepGraphIndex>,
}
