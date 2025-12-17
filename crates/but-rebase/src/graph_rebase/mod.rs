#![deny(missing_docs)]
//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darknes bind them.

mod creation;
pub mod rebase;
use anyhow::{Result, bail};
use gix::refs::transaction::RefEdit;
use std::collections::HashMap;

use anyhow::Context;
pub use creation::GraphExt;
pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod mutate;
pub(crate) mod util;

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
        /// This is intended to be a private API
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
    order: usize,
}

type StepGraphIndex = petgraph::stable_graph::NodeIndex;
type StepGraph = petgraph::stable_graph::StableDiGraph<Step, Edge>;

/// Points to a step in the rebase editor.
#[derive(Debug, Clone, Copy)]
pub struct Selector {
    id: StepGraphIndex,
    revision: usize,
}

/// Represents places where `safe_checkout` should be called from
#[derive(Debug, Clone)]
pub(crate) enum Checkout {
    /// The HEAD of the `repo` the editor was created for.
    Head(Selector),
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
    checkouts: Vec<Checkout>,
    /// The in-memory repository that the rebase engine works with.
    repo: gix::Repository,
    history: RevisionHistory,
}

/// Represents a successful rebase, and any valid, but potentially conflicting scenarios it had.
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct SuccessfulRebase {
    pub(crate) repo: gix::Repository,
    /// Any reference edits that need to be committed as a result of the history
    /// rewrite
    pub(crate) ref_edits: Vec<RefEdit>,
    /// The new step graph
    pub(crate) graph: StepGraph,
    pub(crate) checkouts: Vec<Checkout>,
    pub(crate) history: RevisionHistory,
}

/// The outcome of a materialize
#[derive(Debug, Clone)]
pub struct MaterializeOutcome {
    pub(crate) graph: StepGraph,
    pub(crate) history: RevisionHistory,
}

/// An extenstion trait that provides lookup for different steps that a selector
/// might point to.
pub trait LookupStep {
    /// Look up the step that a given selector cooresponds to.
    fn lookup_step(&self, selector: Selector) -> Result<Step>;

    /// Look up the step a given selector and asserts it's a pick.
    fn lookup_pick(&self, selector: Selector) -> Result<gix::ObjectId> {
        match self.lookup_step(selector)? {
            Step::Pick { id, .. } => Ok(id),
            _ => bail!("Expected selector to point to be a pick"),
        }
    }

    /// Look up the step a given selector and asserts it's a pick.
    fn lookup_reference(&self, selector: Selector) -> Result<gix::refs::FullName> {
        match self.lookup_step(selector)? {
            Step::Reference { refname } => Ok(refname),
            _ => bail!("Expected selector to point to be a reference"),
        }
    }
}

impl LookupStep for Editor {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

impl LookupStep for SuccessfulRebase {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

impl LookupStep for MaterializeOutcome {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

fn lookup_step(graph: &StepGraph, history: &RevisionHistory, selector: Selector) -> Result<Step> {
    let normalized = history.normailze_selector(selector)?;
    Ok(graph[normalized.id].clone())
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RevisionHistory {
    mappings: Vec<HashMap<StepGraphIndex, StepGraphIndex>>,
}

impl RevisionHistory {
    pub(crate) fn new() -> Self {
        Default::default()
    }

    pub(crate) fn current_revision(&self) -> usize {
        self.mappings.len()
    }

    pub(crate) fn normailze_selector(&self, mut selector: Selector) -> Result<Selector> {
        while selector.revision < self.current_revision() {
            selector.id = *self.mappings[selector.revision]
                .get(&selector.id)
                .context("Failed to normalize selector, selector was missing from the mapping")?;
            selector.revision += 1;
        }
        Ok(selector)
    }

    pub(crate) fn add_revision(&mut self, mapping: HashMap<StepGraphIndex, StepGraphIndex>) {
        self.mappings.push(mapping);
    }
}
