use std::path::PathBuf;

use crate::branch::SafeDelete;

/// The outcome of [`SafeDelete::delete_reference()`]
#[derive(Debug, Clone)]
pub struct Outcome<'parent> {
    /// The paths to the worktrees that have the to-be-deleted reference checked out or reserved
    /// for bisect or rebase, which is why the reference wasn't actually deleted.
    pub checked_out_in_worktree_dirs: Option<&'parent [PathBuf]>,
}

impl Outcome<'_> {
    /// Return `true` if the reference was deleted.
    pub fn was_deleted(&self) -> bool {
        self.checked_out_in_worktree_dirs.is_none()
    }
}

/// Lifecycle
impl SafeDelete {
    /// Create a new instance, fetching the required information from `repo`.
    pub fn new(repo: &gix::Repository) -> anyhow::Result<Self> {
        Ok(SafeDelete {
            worktrees_by_ref: repo.checked_out_branches()?,
        })
    }
}

impl SafeDelete {
    /// Return the worktree directories whose `HEAD` points to `rn` (directly or indirectly)
    /// or reserves it for bisect or rebase, or `None` if no worktree does.
    /// Unlike [`delete_reference()`](Self::delete_reference) this
    /// performs no mutation, so it can be used to check the precondition up front.
    pub fn worktree_dirs_with_ref(&self, rn: &gix::Reference) -> Option<&[PathBuf]> {
        self.worktrees_by_ref
            .get(&rn.inner.name)
            .map(|paths| paths.as_slice())
    }

    /// Delete the reference `rn` if no worktree has it checked out or reserves it for bisect or rebase.
    /// Return an outcome to indicate if it was deleted or not.
    pub fn delete_reference(&self, rn: &gix::Reference) -> anyhow::Result<Outcome<'_>> {
        let out = if let Some(paths) = self.worktrees_by_ref.get(&rn.inner.name) {
            Some(paths.as_slice())
        } else {
            rn.delete()?;
            None
        };

        Ok(Outcome {
            checked_out_in_worktree_dirs: out,
        })
    }
}
