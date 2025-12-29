//! Cherry Apply - Applying individual commits into the workspace.
//!
//! For now this doesn't consider the single branch mode, but it hopefully
//! shouldn't be too much of a stretch to adapt it to work.
//!
//! We want to have two steps:
//! - cherry_apply_status: Returns a list of stack IDs where a given commit can
//!   be applied to
//! - cherry_apply: Executes the apply
//!
//! ## Getting the status
//!
//! - list out the applied stacks with stacks_v3
//! - simulate cherry picking the desired commit on to each of the stacks
//!   - if the cherry pick results in a conflict with one of the stacks, it MUST
//!     be applied there
//!   - if the cherry pick results in conflicts with multiple stacks, it can't
//!     be applied since it will cause a workspace conflict.
//!     There is the chance that this looks like this because the commit is
//!     instead conflicting your workspace's base, but this is hard to
//!     disambiguate accurately.
//!
//!   - otherwise, it can be applied anywhere

use anyhow::{Context as _, Result, bail};
use but_core::RepositoryExt;
use but_core::ref_metadata::StackId;
use but_ctx::{
    Context,
    access::{WorktreeReadPermission, WorktreeWritePermission},
};
use but_meta::VirtualBranchesTomlMetadata;
use but_rebase::Rebase;
use but_workspace::legacy::{StacksFilter, stack_ext::StackExt, stacks_v3};
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_stack::VirtualBranchesHandle;
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommitted_changes};
use gix::{ObjectId, Repository};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum CherryApplyStatus {
    CausesWorkspaceConflict,
    /// This also means that when it gets applied to the stack, it will be in a conflicted state
    LockedToStack(StackId),
    ApplicableToAnyStack,
    NoStacks,
}

pub fn cherry_apply_status(
    ctx: &Context,
    _perm: &WorktreeReadPermission,
    subject: ObjectId,
) -> Result<CherryApplyStatus> {
    let repo = ctx
        .repo
        .get()?
        .clone()
        .for_tree_diffing()?
        .with_object_memory();

    let project = &ctx.legacy_project;
    let meta =
        VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))?;
    let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;

    if stacks.is_empty() {
        return Ok(CherryApplyStatus::NoStacks);
    }

    let mut locked_stack = None;
    for stack in stacks {
        let tip = stack
            .heads
            .first()
            .context("Stacks always have a head")?
            .tip;
        if cherry_pick_conflicts(&repo, subject, tip)? {
            if locked_stack.is_some() {
                // Locked stack has already been set to another stack. Now there
                // are at least two stacks that it should be locked to, so we
                // can return early.
                return Ok(CherryApplyStatus::CausesWorkspaceConflict);
            } else {
                locked_stack = Some(
                    stack
                        .id
                        .context("Currently cherry-apply only works with stacks that have ids")?,
                );
            }
        }
    }

    if let Some(stack) = locked_stack {
        Ok(CherryApplyStatus::LockedToStack(stack))
    } else {
        Ok(CherryApplyStatus::ApplicableToAnyStack)
    }
}

pub fn cherry_apply(
    ctx: &Context,
    perm: &mut WorktreeWritePermission,
    subject: ObjectId,
    target: StackId,
) -> Result<()> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let status = cherry_apply_status(ctx, perm.read_permission(), subject)?;
    // Has the frontend told us to do something naughty?
    match status {
        CherryApplyStatus::ApplicableToAnyStack => (),
        CherryApplyStatus::CausesWorkspaceConflict => {
            bail!("Attempting to cherry pick commit that causes workspace conflicts.")
        }
        CherryApplyStatus::NoStacks => {
            bail!("Attempting to cherry pick into a workspace with no applied stacks")
        }
        CherryApplyStatus::LockedToStack(stack) => {
            if stack != target {
                bail!(
                    "Attempting to cherry pick into a different branch that which it is locked to"
                )
            }
        }
    };

    let repo = ctx.repo.get()?.clone().for_tree_diffing()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let mut stack = vb_state.get_stack(target)?;
    let mut steps = stack.as_rebase_steps(ctx, &repo)?;
    // Insert before the head references (len - 1)
    steps.insert(
        steps.len() - 1,
        but_rebase::RebaseStep::Pick {
            commit_id: subject,
            new_message: None,
        },
    );
    let mut rebase = Rebase::new(&repo, stack.merge_base(ctx)?, None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let output = rebase.rebase()?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    {
        let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
        update_uncommitted_changes(ctx, old_workspace, new_workspace, perm)?;
    }

    update_workspace_commit(&vb_state, ctx, false)?;

    Ok(())
}

// Can a given commit be cleanly cherry picked onto another commit
fn cherry_pick_conflicts(repo: &Repository, from: ObjectId, onto: ObjectId) -> Result<bool> {
    let from = repo.find_commit(from)?;
    let onto = repo.find_commit(onto)?;
    let base = from
        .parent_ids()
        .next()
        .context("The commit to be cherry picked must have a parent")?
        .object()?
        .into_commit();

    Ok(!repo.merges_cleanly(
        base.tree_id()?.detach(),
        from.tree_id()?.detach(),
        onto.tree_id()?.detach(),
    )?)
}
