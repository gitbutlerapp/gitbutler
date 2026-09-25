use std::borrow::Cow;

use anyhow::Context as _;
use but_core::{
    DryRun, RefMetadata,
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

    pub fn transaction_with_workspace_setup<Meta, F, T>(
        &self,
        ctx: &mut Context,
        meta: &mut Meta,
        snapshot_details: SnapshotDetails,
        perm: &mut RepoExclusive,
        will_create_unstacked_reference: bool,
        callback: F,
    ) -> anyhow::Result<T::Outcome>
    where
        Meta: RefMetadata,
        F: FnOnce(Transaction<'_, '_, Meta>) -> anyhow::Result<T>,
        T: but_transaction::TransactionOutcome,
    {
        let needs_workspace_setup = self.in_single_branch_mode
            && !self.target_checked_out
            && will_create_unstacked_reference
            && !self.switch;
        if !needs_workspace_setup && !self.switch {
            return but_transaction::with_transaction_with_perm(
                ctx,
                meta,
                perm,
                snapshot_details,
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
            perm.read_permission(),
        )?;

        let setup = if needs_workspace_setup {
            self.setup_workspace(ctx, meta, perm)
        } else {
            Ok(())
        };
        let outcome = setup.and_then(|()| {
            but_transaction::with_transaction_with_perm_only(ctx, meta, perm, DryRun::No, callback)
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

    fn setup_workspace(
        &self,
        ctx: &mut Context,
        meta: &mut impl RefMetadata,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let needs_workspace = ctx
            .repo
            .get()?
            .try_find_reference(but_core::WORKSPACE_REF_NAME)?
            .is_none();
        if needs_workspace {
            let target_ref = self.target_ref.to_string().parse()?;
            gitbutler_branch_actions::set_base_branch_only(ctx, &target_ref, perm)?;
        }

        let (repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
        // Also apply an empty branch, which set_base_branch doesn't apply itself.
        let outcome = but_workspace::branch::apply(
            self.head_reference.as_ref(),
            ws.clone(),
            &repo,
            meta,
            but_workspace::branch::apply::Options {
                allow_applying_already_applied_branch_when_outside_workspace: true,
                ..Default::default()
            },
        )?;
        if outcome.status.persisted_mutation() {
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
