//! Running operations while a branch is checked out on its own rather than GitButler's
//! workspace, which the CLI calls single-branch mode.

use std::borrow::Cow;

use but_core::{RefMetadata, ref_metadata::ProjectMeta, sync::RepoShared};
use but_ctx::Context;
use but_workspace::branch::create_reference::{Anchor, Position};
use gix::{
    ObjectId,
    refs::{FullName, FullNameRef},
};

use crate::Transaction;

/// Whether the main worktree has a branch checked out on its own rather than GitButler's
/// workspace, using `ctx` with shared `perm`ission.
pub fn in_single_branch_mode_with_perm(ctx: &Context, perm: &RepoShared) -> anyhow::Result<bool> {
    let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;

    Ok(match &ws.kind {
        but_graph::workspace::WorkspaceKind::AdHoc => true,
        but_graph::workspace::WorkspaceKind::Managed { .. }
        | but_graph::workspace::WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => false,
    })
}

#[derive(Debug)]
pub(crate) struct StartingCheckout {
    pub(crate) in_single_branch_mode: bool,
    pub(crate) head_reference: Option<FullName>,
    /// The local branch, as `HEAD` names it, not the remote one the project stores.
    target_ref: Option<FullName>,
}

impl StartingCheckout {
    pub(crate) fn read(ctx: &Context, perm: &RepoShared) -> anyhow::Result<Self> {
        let in_single_branch_mode = in_single_branch_mode_with_perm(ctx, perm)?;
        let repo = ctx.repo.get()?;
        let head_reference = (repo.head()?.referent_name())
            .filter(|name| name.category() == Some(gix::refs::Category::LocalBranch))
            .map(ToOwned::to_owned);
        let target_ref = (ProjectMeta::resolve(&repo)?.target_ref)
            .map(|target| target.as_local_full_name_ref().to_owned());
        Ok(Self {
            in_single_branch_mode,
            head_reference,
            target_ref,
        })
    }

    /// Whether a checkpoint can be taken: the oplog snapshots against a target, and restores
    /// the branch checked out.
    pub(crate) fn can_checkpoint(&self) -> bool {
        self.target_ref.is_some() && self.head_reference.is_some()
    }

    fn target_checked_out(&self) -> bool {
        self.target_ref.is_some() && self.head_reference == self.target_ref
    }

    pub(crate) fn new_stack_enters_workspace(&self) -> bool {
        self.in_single_branch_mode
            && self.head_reference.is_some()
            && self.target_ref.is_some()
            && !self.target_checked_out()
    }
}

struct Target {
    commit_id: ObjectId,
    checked_out: bool,
    head_at_commit: bool,
}

impl Target {
    fn read(repo: &gix::Repository, started: &StartingCheckout) -> anyhow::Result<Option<Self>> {
        if started.target_ref.is_none() {
            return Ok(None);
        }
        let commit_id = ProjectMeta::resolve(repo)?.resolve_target_commit(repo)?;
        Ok(Some(Target {
            checked_out: started.target_checked_out(),
            head_at_commit: repo.head_id()?.detach() == commit_id,
            commit_id,
        }))
    }
}

impl<M: RefMetadata> Transaction<'_, '_, M> {
    /// Create `new_ref` as a stack of its own, at `order` among the stacks.
    ///
    /// With the target checked out on its own, or no target, it goes on top of the branch checked
    /// out and is checked out, as `git checkout -b` would, and at the target's commit when that's
    /// where `HEAD` is, as the target isn't a stack member. Beside another branch checked out on
    /// its own, it starts where that branch leaves the target, and GitButler's workspace is
    /// entered at the end of the transaction to hold both. With `HEAD` detached it's independent,
    /// which only the workspace takes.
    pub fn create_unstacked_reference(
        &mut self,
        new_ref: &FullNameRef,
        order: Option<usize>,
    ) -> anyhow::Result<()> {
        self.place_unstacked_reference(new_ref, order, false)
    }

    /// Create `new_ref` as a branch of its own and check it out at the end of the transaction,
    /// leaving any workspace as it is.
    ///
    /// With the target checked out it goes on top, as [`Self::create_unstacked_reference()`]
    /// puts it. Otherwise it starts at the target's commit. Fails with `HEAD` detached, as
    /// [`Self::checkout()`] does.
    pub fn switch_to_new_reference(&mut self, new_ref: &FullNameRef) -> anyhow::Result<()> {
        self.place_unstacked_reference(new_ref, None, true)
    }

    fn place_unstacked_reference(
        &mut self,
        new_ref: &FullNameRef,
        order: Option<usize>,
        switch: bool,
    ) -> anyhow::Result<()> {
        let started = &self.inner.started;
        let in_single_branch_mode = started.in_single_branch_mode;
        let head_reference = started.head_reference.clone();
        let target = Target::read(self.repo(), started)?;
        let above_head = (head_reference.as_ref()).map(|head| Anchor::AtReference {
            ref_name: Cow::Borrowed(head.as_ref()),
            position: Position::Above,
        });
        match (&target, above_head) {
            (Some(target), Some(_)) if target.checked_out && target.head_at_commit => {
                // The integration base isn't a stack member. Create independently instead of
                // recording an explicit branch order that would retain the empty target branch.
                self.create_reference_at_commit(new_ref, target.commit_id)?;
            }
            (Some(target), Some(above_head)) if target.checked_out => {
                self.create_reference(new_ref, above_head, order)?;
            }
            (Some(target), _) if switch => {
                self.create_reference_at_commit(new_ref, target.commit_id)?;
            }
            (None, Some(above_head)) if in_single_branch_mode => {
                self.create_reference(new_ref, above_head, order)?;
            }
            (Some(_) | None, _) => {
                self.create_reference(new_ref, None, order)?;
                if !switch {
                    return Ok(());
                }
            }
        }
        self.checkout(new_ref)
    }

    /// Create `new_ref` at `position` of the branch `relative_to`, in its stack. Above the branch
    /// checked out on its own, it's checked out.
    pub fn create_stacked_reference(
        &mut self,
        new_ref: &FullNameRef,
        relative_to: &FullNameRef,
        position: Position,
    ) -> anyhow::Result<()> {
        let started = &self.inner.started;
        let in_single_branch_mode = started.in_single_branch_mode;
        let check_out = in_single_branch_mode
            && matches!(position, Position::Above)
            && (started.head_reference.as_ref()).is_some_and(|head| head.as_ref() == relative_to);
        let anchor = match (in_single_branch_mode, position) {
            (true, Position::Above) => Anchor::AtReference {
                ref_name: Cow::Borrowed(relative_to),
                position,
            },
            (true, Position::Below) | (false, _) => Anchor::at_segment(relative_to, position),
        };
        self.create_reference(new_ref, anchor, None)?;
        if check_out {
            self.checkout(new_ref)?;
        }
        Ok(())
    }
}
