#![deny(missing_docs)]
//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darknes bind them.

mod creation;
pub mod rebase;
use std::collections::HashMap;

use anyhow::{Context, Result, bail};
pub use creation::GraphExt;
use gix::refs::transaction::RefEdit;
pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod mutate;
pub(crate) mod util;

/// Utilities for testing
pub mod testing;

/// Represents a commit to be cherry-picked in a rebase operation.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    /// The ID of the commit getting picked
    pub id: gix::ObjectId,
    /// If we are dealing with a sub-graph with an incomplete history, we
    /// need to represent the bottom most commits in a way that we preserve
    /// their parents.
    ///
    /// If this is Some, the commit WILL NOT be picked onto the parents the
    /// graph implies but instead on to the parents listed here.
    pub(crate) preserved_parents: Option<Vec<gix::ObjectId>>,
    /// If set to false, a rebase will fail if this commit results in a
    /// conflicted state.
    pub conflictable: bool,
    /// If set to true, a rebase will fail if not all of the parents (outgoing
    /// nodes) are references.
    pub parents_must_be_references: bool,
    /// If set to true, the rebase engine will try to sign the commit if it
    /// gets cherry-picked and the user has configured signing.
    pub sign_if_configured: bool,
}

impl Pick {
    /// Creates a pick with the expected defaults
    pub fn new_pick(id: gix::ObjectId) -> Self {
        Self {
            id,
            preserved_parents: None,
            conflictable: true,
            parents_must_be_references: false,
            sign_if_configured: true,
        }
    }

    /// Creates a pick with the defaults set for a workspace commit
    pub fn new_workspace_pick(id: gix::ObjectId) -> Self {
        Self {
            id,
            preserved_parents: None,
            conflictable: false,
            parents_must_be_references: true,
            sign_if_configured: false,
        }
    }
}

/// Describes what action the engine should take
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Cherry picks the given commit into the new location in the graph
    Pick(Pick),
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
        Self::Pick(Pick::new_pick(id))
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
    pub(crate) initial_references: Vec<gix::refs::FullName>,
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

/// Provides lookup for different steps that a selector might point to.
pub trait LookupStep {
    /// Look up the step that a given selector corresponds to.
    fn lookup_step(&self, selector: Selector) -> Result<Step>;

    /// Look up the step a given selector and assert it's a pick.
    fn lookup_pick(&self, selector: Selector) -> Result<gix::ObjectId> {
        match self.lookup_step(selector)? {
            Step::Pick(Pick { id, .. }) => Ok(id),
            _ => bail!("Expected selector to point to a pick"),
        }
    }

    /// Look up the step a given selector and assert it's a pick.
    fn lookup_reference(&self, selector: Selector) -> Result<gix::refs::FullName> {
        match self.lookup_step(selector)? {
            Step::Reference { refname } => Ok(refname),
            _ => bail!("Expected selector to point to a reference"),
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
    let normalized = history.normalize_selector(selector)?;
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

    pub(crate) fn normalize_selector(&self, mut selector: Selector) -> Result<Selector> {
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

/// I wanted to assert _somewhere_ the defaults for non-workspace & workspace commits. It doesn't feel like the right place to do it in integration tests because we should assert behaviour rather than details there.
#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::graph_rebase::Pick;

    #[test]
    fn workspace_commit_defaults() -> anyhow::Result<()> {
        let object_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;

        assert_eq!(
            Pick::new_workspace_pick(object_id),
            Pick {
                id: object_id,
                preserved_parents: None,
                conflictable: false,
                parents_must_be_references: true,
                sign_if_configured: false
            }
        );

        Ok(())
    }

    #[test]
    fn regular_commit_defaults() -> anyhow::Result<()> {
        let object_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;

        assert_eq!(
            Pick::new_pick(object_id),
            Pick {
                id: object_id,
                preserved_parents: None,
                conflictable: true,
                parents_must_be_references: false,
                sign_if_configured: true
            }
        );

        Ok(())
    }
}
