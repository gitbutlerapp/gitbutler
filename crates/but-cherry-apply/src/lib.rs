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
#![expect(
    deprecated,
    reason = "calls but_workspace::legacy::stacks_v3 and legacy stack methods; these should be replaced with ctx.workspace_* helpers"
)]

use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use but_core::{RepositoryExt, ref_metadata::StackId};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_graph::edit::{InsertSide, MaterializeOptions};
use but_workspace::legacy::{StacksFilter, stacks_v3};
use gitbutler_branch_actions::stack::get_stack;
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
    _perm: &RepoShared,
    subject: ObjectId,
) -> Result<CherryApplyStatus> {
    let repo = ctx
        .repo
        .get()?
        .clone()
        .for_tree_diffing()?
        .with_object_memory();

    let meta = ctx.legacy_meta()?;
    let stacks = stacks_v3(
        &repo,
        &meta,
        &ctx.project_meta()?,
        StacksFilter::InWorkspace,
        None,
    )?;

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
    perm: &mut RepoExclusive,
    subject: ObjectId,
    target: StackId,
) -> Result<()> {
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
    let mut stack = get_stack(ctx, target)?;

    let meta = ctx.legacy_meta()?;
    let graph = but_graph::Graph::from_repo(
        &repo,
        &meta,
        ctx.project_meta()?,
        but_graph::init::Overlay::default(),
    )?;
    let mut mutable = graph.into_mut(&repo)?;

    // Insert the pick directly below the stack's top head reference so it
    // becomes the new topmost commit of the stack, right underneath the
    // (managed) workspace commit.
    let top_head_ref = full_head_ref_name(
        stack
            .heads
            .iter()
            .filter(|head| !head.archived)
            .next_back()
            .context("Stacks always have a head")?,
    )?;
    mutable.insert_commit(top_head_ref, subject, InsertSide::Below)?;
    let rebased = mutable.rebase()?;

    let mut new_heads = HashMap::new();
    for head in stack.heads.iter().filter(|head| !head.archived) {
        let ref_name = full_head_ref_name(head)?;
        let commit_id = rebased.reference_target(ref_name.as_ref())?;
        new_heads.insert(head.name().to_string(), commit_id);
    }

    // Persists the new commits, updates the references (including the
    // workspace commit) and safely checks out the rewritten `HEAD`, carrying
    // over uncommitted changes.
    rebased.materialize_changes(&meta, MaterializeOptions::default())?;

    // Sync the legacy stack metadata with the rewritten heads.
    stack.set_heads_by_name(ctx, new_heads)?;

    Ok(())
}

fn full_head_ref_name(head: &gitbutler_stack::StackBranch) -> Result<gix::refs::FullName> {
    Ok(gix::refs::FullName::try_from(format!(
        "refs/heads/{}",
        head.name()
    ))?)
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
