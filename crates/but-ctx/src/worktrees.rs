//! Enumeration of linked worktrees and their archived state.

use anyhow::Result;
use gix::bstr::BStr;

pub use but_db::worktrees::{WorktreeEntry, WorktreeHead};

use crate::Context;

impl Context {
    /// Add active linked-worktree tips to `options` when worktree manipulation is enabled.
    ///
    /// Each tip's `HEAD` is resolved freshly via [`Self::worktree_head()`]; worktrees
    /// with nothing to seed are skipped.
    ///
    /// Like [`Self::active_worktrees()`], this must not be called while a database
    /// handle is borrowed.
    pub fn graph_options(
        &self,
        mut options: but_graph::init::Options,
    ) -> Result<but_graph::init::Options> {
        for worktree in self.active_worktrees()? {
            let Some(head) = self.worktree_head(worktree.name.as_ref())? else {
                continue;
            };
            options.worktree_tips.push(but_graph::init::WorktreeTip {
                name: worktree.name,
                ref_name: head.ref_name,
                id: head.id,
            });
        }
        Ok(options)
    }

    /// Resolve the `HEAD` of the linked worktree named `name` freshly from its
    /// repository, see [`but_db::worktrees::worktree_head()`].
    pub fn worktree_head(&self, name: &BStr) -> Result<Option<WorktreeHead>> {
        but_db::worktrees::worktree_head(&*self.repo.get()?, name)
    }

    /// List all usable linked worktrees with their archived state, see
    /// [`but_db::worktrees::worktrees_with_state()`].
    ///
    /// With the `worktreeManipulation` feature flag disabled this returns nothing and
    /// has no side effects.
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn worktrees_with_state(&self) -> Result<Vec<WorktreeEntry>> {
        if !self.settings.feature_flags.worktree_manipulation {
            return Ok(Vec::new());
        }
        let repo = self.repo.get()?;
        let mut db = self.db.get_cache_mut()?;
        but_db::worktrees::worktrees_with_state(&repo, &mut db)
    }

    /// Persist whether the linked worktree named `name` is archived.
    ///
    /// This runs the one-time adoption of [`Self::worktrees_with_state()`] first:
    /// adoption archives every worktree on disk, so letting it run afterwards would
    /// silently revert what was explicitly asked for here.
    ///
    /// Rows are never pruned, so this also succeeds for a worktree that isn't on
    /// disk (any more) - such a row simply stays invisible to listings.
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn set_worktree_archived(&self, name: &gix::bstr::BStr, archived: bool) -> Result<()> {
        self.worktrees_with_state()?;
        let mut db = self.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: name.to_vec(),
            archived,
        })?;
        Ok(())
    }

    /// List all non-archived linked worktrees; every returned entry has
    /// `archived == false`.
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
