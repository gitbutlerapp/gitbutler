//! Running operations while a branch is checked out on its own rather than GitButler's
//! workspace, which the CLI calls single-branch mode.
//!
//! An operation that makes a branch of its own sets up the workspace first, as there's no
//! workspace commit to hold two stacks otherwise, and one that can check out another branch
//! rolls back whole if that fails.

use std::borrow::Cow;

use but_core::{
    DryRun, RefMetadata,
    ref_metadata::{LocalRefName, ProjectMeta},
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
    /// The project's target, which the workspace is set up on and the oplog records against.
    target: Option<Target>,
}

/// The project's target, as [`SingleBranchMode`] reads it.
#[derive(Debug)]
struct Target {
    ref_name: LocalRefName,
    commit_id: ObjectId,
    /// It's the branch checked out.
    checked_out: bool,
    /// `HEAD` is at its commit.
    head_at_commit: bool,
}

impl SingleBranchMode {
    /// Read what's checked out using `ctx` with shared `perm`ission, for an operation that
    /// checks out the branch it makes or moves to with `switch`.
    ///
    /// Without a target there's no workspace to set up, so a new branch of its own goes on top
    /// of the branch checked out instead. With `HEAD` detached there's no branch to set it up
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
                    ref_name: ref_name.clone(),
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
    /// When it `will_create_independent_branch` in single-branch mode, GitButler's workspace
    /// is set up first, with the branch checked out applied. When it may check out a branch,
    /// a failure partway restores the checkpoint taken before it began. Without a target,
    /// which the workspace is set up on and a checkpoint records against, or a branch checked
    /// out to set it up with, it does neither.
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
        will_create_independent_branch: bool,
        callback: F,
    ) -> anyhow::Result<T::Outcome>
    where
        Meta: RefMetadata,
        F: FnOnce(Transaction<'_, '_, Meta>) -> anyhow::Result<T>,
        T: TransactionOutcome,
    {
        let (Some(target), Some(head)) = (&self.target, &self.head_reference) else {
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
        let needs_workspace_setup = self.in_single_branch_mode
            && !target.checked_out
            && will_create_independent_branch
            && !self.switch;
        // Single-branch operations can schedule an implicit checkout (for example when
        // creating above HEAD or moving the top branch). Protect those just like --switch.
        let may_checkout = self.switch || self.in_single_branch_mode || target.checked_out;
        if !needs_workspace_setup && !may_checkout {
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

        let setup = if needs_workspace_setup {
            Self::setup_workspace(ctx, meta, perm, &target.ref_name, head)
        } else {
            Ok(())
        };
        let outcome = setup.and_then(|()| {
            crate::with_transaction_with_perm_only(ctx, meta, perm, DryRun::No, callback)
        });
        if let Ok((false, value)) = outcome {
            let snapshot_result = checkpoint.commit(ctx, perm);
            // Workspace setup already used best-effort recording; switching must
            // retain the regular transaction wrapper's snapshot error propagation.
            if !needs_workspace_setup {
                snapshot_result?;
            }
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

    /// Set up GitButler's workspace on `target_ref` using `ctx`, `meta` and exclusive
    /// `perm`ission, with `head`, the branch checked out on its own, applied.
    fn setup_workspace(
        ctx: &mut Context,
        meta: &mut impl RefMetadata,
        perm: &mut RepoExclusive,
        target_ref: &LocalRefName,
        head: &FullName,
    ) -> anyhow::Result<()> {
        let needs_workspace = ctx
            .repo
            .get()?
            .try_find_reference(but_core::WORKSPACE_REF_NAME)?
            .is_none();
        if needs_workspace {
            let target_ref = target_ref.to_string().parse()?;
            gitbutler_branch_actions::set_base_branch_only(ctx, &target_ref, perm)?;
            // Base setup writes metadata through its own handle. Keep the caller's handle in
            // sync so creating the destination doesn't overwrite the newly applied source stack.
            let workspace_ref = but_core::WORKSPACE_REF_NAME.try_into()?;
            let updated = ctx.meta()?.workspace(workspace_ref)?;
            let mut workspace = meta.workspace(workspace_ref)?;
            *workspace = (*updated).clone();
            meta.set_workspace(&workspace)?;
        }

        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        // Also apply an empty branch, which set_base_branch doesn't apply itself.
        // Non-empty branches may already have been applied by set_base_branch.
        let editor = but_rebase::graph_rebase::Editor::create(&mut ws, meta, &repo, &mut db)?;
        let outcome = but_workspace::branch::apply_stack(editor, head.as_ref())?;
        outcome.rebase.materialize(Default::default())?;
        if let Some(tracking) = outcome.tracking {
            tracking.write(&repo)?;
        }
        Ok(())
    }

    /// Create `new_ref` in `tx` as a branch of its own: in the workspace beside the other
    /// stacks, at `order` among them. With the target checked out on its own, or no target,
    /// it goes on top of the branch checked out and is checked out, as `git checkout -b` would,
    /// and at the target's commit when that's where `HEAD` is, as the target isn't a stack
    /// member. When the operation switches to it, it starts at the target's commit. With
    /// `HEAD` detached and no switch, it's independent, which only the workspace takes.
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
