#![deny(missing_docs)]
//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darknes bind them.

mod creation;
pub mod rebase;
pub use creation::GraphExt;
pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod mutate;

/// Utilities for testing
pub mod testing;

/// Describes what action the engine should take
#[derive(Debug, Clone)]
pub enum Step {
    /// Cherry picks the given commit into the new location in the graph
    Pick {
        /// The ID of the commit getting picked
        id: gix::ObjectId,
        /// If we are dealing with a sub-graph with an incomplete history, we
        /// need to represent the bottom most commits in a way that we preserve
        /// their parents.
        ///
        /// If this is Some, the commit WILL NOT be picked onto the parents the
        /// graph implies but instead on to the parents listed here.
        ///
        /// This is intened to be a private API
        preserved_parents: Option<Vec<gix::ObjectId>>,
    },
    /// Represents applying a reference to the commit found at it's first parent
    Reference {
        /// The refname
        refname: gix::refs::FullName,
    },
    /// Used as a placeholder after removing a pick or reference
    None,
}

impl Step {
    /// Creates a pick with the expected defaults
    pub fn new_pick(id: gix::ObjectId) -> Self {
        Self::Pick {
            id,
            preserved_parents: None,
        }
    }
}

/// Used to represent a connection between a given commit.
#[derive(Debug, Clone)]
pub(crate) struct Edge {
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
    id: StepGraphIndex,
}

/// Represents places where `safe_checkout` should be called from
#[derive(Debug, Clone)]
pub(crate) enum Checkouts {
    /// The HEAD of the `repo` the editor was created for.
    Head,
}

/// Used to manipulate a set of picks.
#[derive(Debug, Clone)]
pub struct Editor {
    /// The internal graph of steps
    graph: StepGraph,
    /// Initial references. This is used to track any references that might need
    /// deleted.
    initial_references: Vec<gix::refs::FullName>,
    /// Worktrees that we might need to perform `safe_checkout` on.
    checkouts: Vec<Checkouts>,
    /// The in-memory repository that the rebase engine works with.
    repo: gix::Repository,
}
