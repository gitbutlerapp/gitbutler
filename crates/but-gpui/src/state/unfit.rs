//! Locked changes held back from landing where they lack some of what they build on, till the
//! window asks the user ([`StoreEvent::Unfit`]).

use std::path::Path;

use anyhow::Result;
use gpui::Context;

use super::{ProjectId, Store, StoreEvent};
use crate::backend::locks::{Fit, Footing};
use crate::backend::{Checkout, CommitId, Conflicts, Selected, Stacking, Workspace};

/// Where locked changes land.
pub enum Lands {
    Amend(CommitId),
    Commit,
}

/// Locked changes landing in `project` as `lands` says, after stacking as `restack` says, and
/// taking what doesn't merge there as `conflicts` says.
pub struct Unfit {
    pub project: ProjectId,
    pub lands: Lands,
    pub restack: Option<Stacking>,
    pub conflicts: Conflicts,
}

/// Whether stacking as asked, in the project at the path, still lets changes land.
pub type Tried = Box<dyn FnOnce(&Path, &Stacking) -> Result<bool> + Send>;

/// What lands changes held back.
pub type Go = Box<dyn FnOnce(&mut Store, &mut Context<Store>)>;

/// What a landing held back does: `unfit` says where it goes, `tried` whether its stacking
/// still lets it land, and `go` lands it.
pub struct Held {
    pub unfit: Unfit,
    pub tried: Tried,
    pub go: Go,
}

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

    /// Hold `held` back for the window to ask about.
    pub(super) fn hold(&mut self, held: Held, cx: &mut Context<Self>) {
        self.held = Some(held);
        cx.emit(StoreEvent::Unfit);
    }

    /// The landing last held back, for the window to ask about, once.
    pub fn take_held(&mut self) -> Option<Held> {
        self.held.take()
    }
}
