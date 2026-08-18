//! Enumeration of linked worktrees and their archived state.

use anyhow::Result;
use gix::bstr::BStr;

pub use but_db::worktrees::{WorktreeEntry, WorktreeHead};

use crate::Context;

impl Context {
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
}
