//! Running operations while a branch is checked out on its own rather than GitButler's
//! workspace, which the CLI calls single-branch mode.
//!
//! An operation that makes a branch of its own enters the workspace at the end of its
//! transaction, as there's no workspace commit to hold two stacks otherwise, and one that can
//! check out another branch rolls back whole if that fails.

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

/// What's checked out in the main worktree, read before an operation that makes or moves a
/// branch, to run it as single-branch mode needs.
#[derive(Debug)]
pub struct SingleBranchMode {
    in_single_branch_mode: bool,
    switch: bool,
    /// The branch checked out, if one is rather than a detached `HEAD`.
    head_reference: Option<FullName>,
    /// The project's target, which a new branch of its own starts at and the oplog records against.
    target: Option<Target>,
}

/// The project's target, as [`SingleBranchMode`] reads it.
#[derive(Debug)]
struct Target {
    commit_id: ObjectId,
    checked_out: bool,
    head_at_commit: bool,
}

impl SingleBranchMode {
    /// Read what's checked out using `ctx` with shared `perm`ission, for an operation that
    /// checks out the branch it makes or moves to with `switch`.
    ///
    /// Without a target there's no workspace to enter, so a new branch of its own goes on top
    /// of the branch checked out instead. With `HEAD` detached there's no branch to enter it
    /// with either. Fails if it would `switch` with `HEAD` detached: that runs without the
    /// checkpoint that takes back a checkout that fails after the changes went in.
    pub fn new(ctx: &Context, perm: &RepoShared, switch: bool) -> anyhow::Result<Self> {
        let in_single_branch_mode = in_single_branch_mode_with_perm(ctx, perm)?;

        let repo = ctx.repo.get()?;

        let project_meta = ProjectMeta::resolve(&repo)?;
        let head_reference = (repo.head()?.referent_name())
            .filter(|name| name.category() == Some(gix::refs::Category::LocalBranch))
            .map(ToOwned::to_owned);
        if switch && head_reference.is_none() {
            anyhow::bail!("--switch needs a branch checked out (HEAD is detached)");
        }

        let target = match &project_meta.target_ref {
            Some(ref_name) => {
                let commit_id = project_meta.resolve_target_commit(&repo)?;
                let target_ref = ref_name.as_local_full_name_ref();
                Some(Target {
                    checked_out: (head_reference.as_ref())
                        .is_some_and(|head| head.as_ref() == target_ref),
                    head_at_commit: repo.head_id()?.detach() == commit_id,
                    commit_id,
                })
            }
            None => None,
        };

        Ok(Self {
            in_single_branch_mode,
            switch,
            head_reference,
            target,
        })
    }

    /// Run `callback` in a transaction recorded in the oplog as `snapshot_details`, naming
    /// `snapshot_refs` so undo deletes those it creates, using `ctx`, `meta` and exclusive
    /// `perm`ission.
    ///
    /// In single-branch mode, or when it may check out a branch, a failure partway restores the
    /// checkpoint taken before it began, which is also the undo entry. Without a target, which a
    /// checkpoint records against, or a branch checked out, it doesn't take one.
    ///
    /// `_will_create_independent_branch` is ignored: the transaction enters GitButler's
    /// workspace itself at the end when an operation makes a stack of its own beside the
    /// branch checked out, see [`Self::create_unstacked_reference()`] and
    /// [`Transaction::tear_off_branch()`].
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
        let (Some(target), Some(_)) = (&self.target, &self.head_reference) else {
            return crate::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
                snapshot_refs,
                DryRun::No,
                callback,
            );
        };
        // Single-branch operations can schedule an implicit checkout (for example when
        // creating above HEAD or moving the top branch), or enter the workspace. Protect those
        // just like --switch.
        let may_checkout = self.switch || self.in_single_branch_mode || target.checked_out;
        if !may_checkout {
            return crate::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
                snapshot_refs,
                DryRun::No,
                callback,
            );
        }

        // Switching can fail after materialization has consumed worktree changes,
        // even when no workspace setup was needed. Cover the whole operation.
        let original_metadata = meta.workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?;
        let checkpoint = but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
            ctx,
            snapshot_details,
            snapshot_refs,
            perm.read_permission(),
        )?;

        let outcome = crate::with_transaction_with_perm_only(ctx, meta, perm, DryRun::No, callback);
        if let Ok((false, value)) = outcome {
            checkpoint.commit(ctx, perm)?;
            return Ok(value);
        }

        // Restore the caller's metadata too: legacy handles write on drop and
        // must not re-persist the failed operation after the checkpoint is restored.
        let metadata_result = meta.set_workspace(&original_metadata);
        let rollback_result = checkpoint.rollback(ctx, perm);
        let rollback_result = metadata_result.and(rollback_result);
        match outcome {
            Ok((_, value)) => {
                rollback_result?;
                Ok(value)
            }
            Err(err) => {
                if let Err(rollback_err) = rollback_result {
                    return Err(
                        err.context(format!("Failed to roll back operation: {rollback_err:#}"))
                    );
                }
                Err(err)
            }
        }
    }

    /// Create `new_ref` in `tx` as a branch of its own: in the workspace beside the other
    /// stacks, at `order` among them. With the target checked out on its own, or no target,
    /// it goes on top of the branch checked out and is checked out, as `git checkout -b` would,
    /// and at the target's commit when that's where `HEAD` is, as the target isn't a stack
    /// member. When the operation switches to it, it starts at the target's commit. Beside
    /// another branch checked out on its own, it starts where that branch leaves the target,
    /// and the transaction enters the workspace at the end to hold both. With `HEAD` detached
    /// and no switch, it's independent, which only the workspace takes.
    pub fn create_unstacked_reference<M: RefMetadata>(
        &self,
        tx: &mut Transaction<'_, '_, M>,
        new_ref: &FullNameRef,
        order: Option<usize>,
    ) -> anyhow::Result<()> {
        let above_head = (self.head_reference.as_ref()).map(|head| Anchor::AtReference {
            ref_name: Cow::Borrowed(head.as_ref()),
            position: Position::Above,
        });
        match (&self.target, above_head) {
            (Some(target), Some(_)) if target.checked_out && target.head_at_commit => {
                // The integration base isn't a stack member. Create independently instead of
                // recording an explicit branch order that would retain the empty target branch.
                tx.create_reference_at_commit(new_ref, target.commit_id)?;
            }
            (Some(target), Some(above_head)) if target.checked_out => {
                tx.create_reference(new_ref, above_head, order)?;
            }
            (Some(target), _) if self.switch => {
                tx.create_reference_at_commit(new_ref, target.commit_id)?;
            }
            (Some(target), Some(_)) if self.in_single_branch_mode => {
                // Where the branch checked out leaves the target, so both merge in the workspace
                // commit, even when the target changed the same lines since.
                let repo = tx.repo();
                let base = repo
                    .merge_base(repo.head_id()?, target.commit_id)?
                    .map_or(target.commit_id, gix::Id::detach);
                tx.create_reference_at_commit(new_ref, base)?;
                tx.apply_new_stack_at_end(new_ref, order);
                return Ok(());
            }
            (None, Some(above_head)) if self.in_single_branch_mode => {
                tx.create_reference(new_ref, above_head, order)?;
            }
            (Some(_) | None, _) => return tx.create_reference(new_ref, None, order),
        }
        tx.checkout(new_ref)
    }

    /// Create `new_ref` in `tx` at `position` of the branch `relative_to`, in its stack. Above
    /// the branch checked out on its own, it's checked out.
    pub fn create_stacked_reference<M: RefMetadata>(
        &self,
        tx: &mut Transaction<'_, '_, M>,
        new_ref: &FullNameRef,
        relative_to: &FullNameRef,
        position: Position,
    ) -> anyhow::Result<()> {
        let check_out = self.in_single_branch_mode
            && matches!(position, Position::Above)
            && (self.head_reference.as_ref()).is_some_and(|head| head.as_ref() == relative_to);
        let anchor = match (self.in_single_branch_mode, position) {
            (true, Position::Above) => Anchor::AtReference {
                ref_name: Cow::Borrowed(relative_to),
                position,
            },
            (true, Position::Below) | (false, _) => Anchor::at_segment(relative_to, position),
        };
        tx.create_reference(new_ref, anchor, None)?;
        if check_out {
            tx.checkout(new_ref)?;
        }
        Ok(())
    }
}
