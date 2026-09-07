use std::borrow::Cow;

use anyhow::Context as _;
use but_core::{
    DryRun,
    ref_metadata::ProjectMeta,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_oplog::legacy::SnapshotDetails;
use but_transaction::Transaction;
use but_workspace::branch::create_reference::Anchor;
use gix::{
    ObjectId,
    refs::{FullName, FullNameRef},
};

use crate::utils::{head_name, in_single_branch_mode_with_perm, targeting::Side};

#[derive(Debug)]
pub struct SingleBranchMode {
    in_single_branch_mode: bool,
    target_checked_out: bool,
    target_ref: FullName,
    target_commit_id: ObjectId,
    switch: bool,
    head_reference: FullName,
}

impl SingleBranchMode {
    pub fn new(ctx: &Context, perm: &RepoShared, switch: bool) -> anyhow::Result<Self> {
        let in_single_branch_mode = in_single_branch_mode_with_perm(ctx, perm)?;

        let (repo, ..) = ctx.workspace_and_db_mut_with_perm(perm)?;

        let project_meta = ProjectMeta::resolve(&repo)?;
        let head_reference = head_name(&repo)?;

        let target_commit_id = project_meta.target_commit_id_or_err()?;

        let target_ref = project_meta
            .target_ref
            .context("BUG: target ref is missing")?;

        let target_checked_out =
            but_core::branch::resolve_tracking_branch_ref_name(head_reference.as_ref(), &repo)
                .is_ok_and(|upstream| &*upstream == target_ref.as_ref());

        Ok(Self {
            in_single_branch_mode,
            target_checked_out,
            target_ref,
            target_commit_id,
            switch,
            head_reference,
        })
    }

    pub fn transaction_with_workspace_setup<F, T>(
        &self,
        ctx: &mut Context,
        snapshot_details: SnapshotDetails,
        perm: &mut RepoExclusive,
        will_create_independent_branch: bool,
        callback: F,
    ) -> anyhow::Result<T::Outcome>
    where
        F: FnOnce(Transaction<'_, '_, '_>) -> anyhow::Result<T>,
        T: but_transaction::TransactionOutcome,
    {
        let needs_workspace_setup = self.in_single_branch_mode
            && !self.target_checked_out
            && will_create_independent_branch
            && !self.switch;
        // Single-branch operations can schedule an implicit checkout (for example when
        // creating above HEAD or moving the top branch). Protect those just like --switch.
        let may_checkout = self.switch || self.in_single_branch_mode || self.target_checked_out;
        if !needs_workspace_setup && !may_checkout {
            return but_transaction::with_transaction_with_perm(
                ctx,
                perm,
                snapshot_details,
                DryRun::No,
                callback,
            );
        }

        // Switching can fail after materialization has consumed worktree changes,
        // even when no workspace setup was needed. Cover the whole operation.
        let checkpoint = but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
            ctx,
            snapshot_details,
            perm.read_permission(),
        )?;

        let setup = if needs_workspace_setup {
            self.setup_workspace(ctx, perm)
        } else {
            Ok(())
        };
        let outcome = setup.and_then(|()| {
            but_transaction::with_transaction_with_perm_only(ctx, perm, DryRun::No, callback)
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

        let rollback_result = checkpoint.rollback(ctx, perm);
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

    fn setup_workspace(&self, ctx: &mut Context, perm: &mut RepoExclusive) -> anyhow::Result<()> {
        let needs_workspace = ctx
            .repo
            .get()?
            .try_find_reference(but_core::WORKSPACE_REF_NAME)?
            .is_none();
        if needs_workspace {
            let target_ref = self.target_ref.to_string().parse()?;
            gitbutler_branch_actions::set_base_branch_only(ctx, &target_ref, perm)?;
        }

        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        let mut transaction = db.immediate_transaction()?;
        // Also apply an empty branch, which set_base_branch doesn't apply itself.
        // Non-empty branches may already have been applied by set_base_branch.
        let outcome = but_workspace::branch::apply(
            self.head_reference.as_ref(),
            ws.clone(),
            &repo,
            &mut transaction.connection_mut(),
            but_workspace::branch::apply::Options {
                allow_applying_already_applied_branch_when_outside_workspace: true,
                ..Default::default()
            },
        )?;
        if outcome.status.persisted_mutation()
            || matches!(
                outcome.status,
                but_workspace::branch::apply::OutcomeStatus::AlreadyApplied
            )
        {
            transaction.commit()?;
            *ws = outcome.workspace;
        } else {
            anyhow::bail!(
                "BUG: failed to apply head ref ({}). Failed with {:?}",
                self.head_reference,
                outcome.status
            )
        }
        Ok(())
    }

    pub fn how_to_create_unstacked_reference(&self) -> HowToCreateUnstackedReference<'_> {
        if self.target_checked_out {
            let anchor = Anchor::AtReference {
                ref_name: Cow::Borrowed(self.head_reference.as_ref()),
                position: Side::Above.into(),
            };
            HowToCreateUnstackedReference::CreateRefAtAnchorThenCheckout(anchor)
        } else if self.switch {
            HowToCreateUnstackedReference::CreateRefAtCommitThenCheckout {
                target_commit_id: self.target_commit_id,
            }
        } else {
            HowToCreateUnstackedReference::Normally
        }
    }

    pub fn how_to_create_stacked_reference<'a>(
        &'a self,
        create_ref_relative_to: &'a FullNameRef,
        side: Side,
    ) -> HowToCreateStackedReference<'a> {
        if self.in_single_branch_mode {
            match side {
                Side::Above => {
                    let anchor = Anchor::AtReference {
                        ref_name: Cow::Borrowed(create_ref_relative_to),
                        position: side.into(),
                    };
                    if create_ref_relative_to == self.head_reference.as_ref() {
                        HowToCreateStackedReference::CreateRefAtAnchorThenCheckout(anchor)
                    } else {
                        HowToCreateStackedReference::Normally(anchor)
                    }
                }
                Side::Below => {
                    let anchor = Anchor::at_segment(create_ref_relative_to, side.into());
                    HowToCreateStackedReference::Normally(anchor)
                }
            }
        } else {
            let anchor = Anchor::at_segment(create_ref_relative_to, side.into());
            HowToCreateStackedReference::Normally(anchor)
        }
    }
}

#[derive(Debug)]
pub enum HowToCreateUnstackedReference<'a> {
    Normally,
    CreateRefAtAnchorThenCheckout(Anchor<'a>),
    CreateRefAtCommitThenCheckout { target_commit_id: ObjectId },
}

#[derive(Debug)]
pub enum HowToCreateStackedReference<'a> {
    Normally(Anchor<'a>),
    CreateRefAtAnchorThenCheckout(Anchor<'a>),
}
