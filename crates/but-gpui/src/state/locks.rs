//! What the main checkout's locks say of changes going somewhere, for the navigator to hint at
//! while it's being placed. The operation itself finds out for sure.

use super::{ProjectId, Store};
use crate::backend::locks::{Fit, Footing};
use crate::backend::{Checkout, Selected, Workspace};

impl Store {
    /// How `worktree`'s `files` put on the `footing` it reads of `project` sit with what they
    /// build on, if they're the main checkout's, whose locks are read.
    pub fn lock_fit(
        &self,
        project: &ProjectId,
        worktree: &Checkout,
        files: &[Selected],
        footing: impl FnOnce(&Workspace) -> Option<Footing>,
    ) -> Option<Fit> {
        let locks = self.locks.get(project).filter(|_| !worktree.is_linked())?;
        let workspace = self.workspace(project)?;
        Some(locks.fit(workspace, files, footing(workspace)?))
    }
}
