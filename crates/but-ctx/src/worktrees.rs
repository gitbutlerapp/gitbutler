//! Access to linked worktrees and their archived state, gated on the feature flag.
//!
//! The enumeration and archived-state reconciliation itself lives in
//! [`but_graph::worktrees`] - the graph reads it directly while traversing, so this is
//! only the context-shaped way in.

use anyhow::Result;

use crate::Context;

/// A usable linked worktree with its archived state and resolved `HEAD`.
pub use but_graph::worktrees::Worktree as WorktreeEntry;

impl Context {
    /// Let `options` read linked-worktree state from the database handle passed to the
    /// traversal, unless the `worktreeManipulation` feature flag is off - then no
    /// traversal will look at worktrees at all.
    ///
    /// The database is read on *every* traversal the resulting graph performs, so a
    /// long-lived workspace keeps up with worktrees that appear, move, or get archived.
    pub fn graph_options(&self, mut options: but_graph::init::Options) -> but_graph::init::Options {
        options.worktrees = self.settings.feature_flags.worktree_manipulation;
        options
    }

    /// List all usable linked worktrees with their archived state and resolved `HEAD`s,
    /// see [`but_graph::worktrees::with_state()`] for what that entails.
    ///
    /// With the `worktreeManipulation` feature flag disabled this returns nothing and
    /// has no side effects.
    ///
    /// Errors when the context repository is itself a linked worktree: such a context
    /// stores its database in the worktree's private git dir, so adoption and archived
    /// state would silently diverge from the main worktree's database.
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn worktrees_with_state(&self) -> Result<Vec<WorktreeEntry>> {
        if !self.settings.feature_flags.worktree_manipulation {
            return Ok(Vec::new());
        }
        let repo = self.repo.get()?;
        let mut db = self.db.get_cache_mut()?;
        but_graph::worktrees::with_state(&repo, &mut db)
    }

    /// Persist whether the linked worktree named `name` is archived, see
    /// [`but_graph::worktrees::set_archived()`].
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn set_worktree_archived(&self, name: &gix::bstr::BStr, archived: bool) -> Result<()> {
        let repo = self.repo.get()?;
        let mut db = self.db.get_cache_mut()?;
        but_graph::worktrees::set_archived(&repo, &mut db, name, archived)
    }

    /// List all non-archived linked worktrees with their resolved `HEAD`s; every
    /// returned entry has `archived == false`.
    ///
    /// This is [`Self::worktrees_with_state()`] filtered down to active worktrees,
    /// including its adoption side-effect, flag gating, and linked-worktree error;
    /// the same caveats apply.
    pub fn active_worktrees(&self) -> Result<Vec<WorktreeEntry>> {
        Ok(self
            .worktrees_with_state()?
            .into_iter()
            .filter(|wt| !wt.archived)
            .collect())
    }
}
