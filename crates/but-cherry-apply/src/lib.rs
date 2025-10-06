//! Cherry Apply - Applying individual commits into the workspace.
//!
//! For now this doesn't consider the single branch mode, but it hopfully
//! shouldn't be too much of a strech to adapt it to work.
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

use anyhow::{Context, Result};
use but_graph::VirtualBranchesTomlMetadata;
use but_workspace::{StackId, StacksFilter, stacks_v3};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::GixRepositoryExt;
use gix::{ObjectId, Repository};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum CherryApplyStatus {
    CausesWorkspaceConflict,
    /// This also means that when it gets applied to the stack, it will be in a conflicted state
    LockedToStack(StackId),
    ApplicableToAnyStack,
    NoStacks,
}

pub fn cherry_apply_status(ctx: &CommandContext, subject: ObjectId) -> Result<CherryApplyStatus> {
    let repo = ctx.gix_repo()?;
    let project = ctx.project();
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

    let (merge_options_fail_fast, conflict_kind) = repo.merge_options_no_rewrites_fail_fast()?;
    let result = repo.merge_trees(
        base.tree_id()?,
        from.tree_id()?,
        onto.tree_id()?,
        repo.default_merge_labels(),
        merge_options_fail_fast,
    )?;

    Ok(result.has_unresolved_conflicts(conflict_kind))
}
