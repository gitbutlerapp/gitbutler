//! Running operations while a branch is checked out on its own rather than GitButler's
//! workspace, which the CLI calls single-branch mode.

use std::borrow::Cow;

use but_core::{
    DryRun, RefMetadata,
    ref_metadata::ProjectMeta,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_oplog::legacy::SnapshotDetails;
use but_workspace::branch::create_reference::{Anchor, Position};
use gix::{
    ObjectId,
    refs::{FullName, FullNameRef},
};

use crate::{Transaction, TransactionOutcome};

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

/// What the main worktree had checked out when a transaction started.
#[derive(Debug)]
pub(crate) struct StartingCheckout {
    /// A branch was checked out on its own rather than GitButler's workspace.
    pub(crate) in_single_branch_mode: bool,
    /// The branch checked out, if one was rather than a detached `HEAD`.
    pub(crate) head_reference: Option<FullName>,
    /// The project has a target, which the oplog snapshots against.
    has_target: bool,
}

impl StartingCheckout {
    /// Read what's checked out using `ctx` with shared `perm`ission.
    pub(crate) fn read(ctx: &Context, perm: &RepoShared) -> anyhow::Result<Self> {
        let in_single_branch_mode = in_single_branch_mode_with_perm(ctx, perm)?;
        let repo = ctx.repo.get()?;
        let head_reference = (repo.head()?.referent_name())
            .filter(|name| name.category() == Some(gix::refs::Category::LocalBranch))
            .map(ToOwned::to_owned);
        let has_target = ProjectMeta::resolve(&repo)?.target_ref.is_some();
        Ok(Self {
            in_single_branch_mode,
            head_reference,
            has_target,
        })
    }

    /// Whether a checkpoint can be taken: the oplog snapshots against a target, and restores
    /// the branch checked out.
    pub(crate) fn can_checkpoint(&self) -> bool {
        self.has_target && self.head_reference.is_some()
    }
}

/// The project's target, as placing a new branch reads it.
struct Target {
    commit_id: ObjectId,
    checked_out: bool,
    head_at_commit: bool,
}

impl Target {
    /// Read the target from `repo`, with `head_reference` checked out, or `None` without one.
    fn read(
        repo: &gix::Repository,
        head_reference: Option<&FullName>,
    ) -> anyhow::Result<Option<Self>> {
        let project_meta = ProjectMeta::resolve(repo)?;
        let Some(ref_name) = &project_meta.target_ref else {
            return Ok(None);
        };
        let commit_id = project_meta.resolve_target_commit(repo)?;
        let target_ref = ref_name.as_local_full_name_ref();
        Ok(Some(Target {
            checked_out: head_reference.is_some_and(|head| head.as_ref() == target_ref),
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
        let target = Target::read(self.repo(), head_reference.as_ref())?;
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

/// An operation's choice to check out the branch it makes or moves to, before the callers use
/// [`Transaction`]'s methods directly.
#[derive(Debug)]
pub struct SingleBranchMode {
    switch: bool,
}

impl SingleBranchMode {
    /// Read what's checked out using `ctx` with shared `perm`ission, for an operation that
    /// checks out the branch it makes or moves to with `switch`. Fails if it would `switch`
    /// with `HEAD` detached.
    pub fn new(ctx: &Context, perm: &RepoShared, switch: bool) -> anyhow::Result<Self> {
        if switch && StartingCheckout::read(ctx, perm)?.head_reference.is_none() {
            anyhow::bail!("--switch needs a branch checked out (HEAD is detached)");
        }
        Ok(Self { switch })
    }

    /// Run `callback` in a transaction, as [`crate::with_transaction_with_perm()`] does.
    #[expect(
        clippy::too_many_arguments,
        reason = "a transaction's own arguments, and what single-branch mode adds"
    )]
    pub fn transaction_with_workspace_setup<Meta, F, T>(
        &self,
        ctx: &mut Context,
        meta: &mut Meta,
        snapshot_details: SnapshotDetails,
        snapshot_refs: &[&FullNameRef],
        perm: &mut RepoExclusive,
        _will_create_independent_branch: bool,
        callback: F,
    ) -> anyhow::Result<T::Outcome>
    where
        Meta: RefMetadata,
        F: FnOnce(Transaction<'_, '_, Meta>) -> anyhow::Result<T>,
        T: TransactionOutcome,
    {
        crate::with_transaction_with_perm(
            ctx,
            meta,
            perm,
            snapshot_details,
            snapshot_refs,
            DryRun::No,
            callback,
        )
    }

    /// Create `new_ref` in `tx` with [`Transaction::switch_to_new_reference()`] when switching,
    /// or else [`Transaction::create_unstacked_reference()`] at `order`.
    pub fn create_unstacked_reference<M: RefMetadata>(
        &self,
        tx: &mut Transaction<'_, '_, M>,
        new_ref: &FullNameRef,
        order: Option<usize>,
    ) -> anyhow::Result<()> {
        if self.switch {
            tx.switch_to_new_reference(new_ref)
        } else {
            tx.create_unstacked_reference(new_ref, order)
        }
    }

    /// Create `new_ref` in `tx` with [`Transaction::create_stacked_reference()`].
    pub fn create_stacked_reference<M: RefMetadata>(
        &self,
        tx: &mut Transaction<'_, '_, M>,
        new_ref: &FullNameRef,
        relative_to: &FullNameRef,
        position: Position,
    ) -> anyhow::Result<()> {
        tx.create_stacked_reference(new_ref, relative_to, position)
    }
}
