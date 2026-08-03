use std::borrow::Cow;

use anyhow::{Context as _, Result, anyhow};
use but_api_macros::but_api;
use but_core::{branch, ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_oplog::SnapshotExt;
use gix::refs::Category;
use tracing::instrument;

/// Create a dependent branch named by `request.name` in the stack identified by
/// `stack_id`.
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// dependent-branch snapshot and mutating the workspace.
#[but_api]
#[instrument(err(Debug))]
pub fn create_branch(
    ctx: &mut Context,
    stack_id: StackId,
    request: CreateSeriesRequest,
) -> Result<()> {
    let normalized_name = branch::normalize_short_name(request.name.as_str())?.to_string();
    let new_ref = Category::LocalBranch
        .to_full_name(normalized_name.as_str())
        .map_err(anyhow::Error::from)?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    ctx.snapshot_create_dependent_branch(&normalized_name, guard.write_permission())
        .ok();

    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
    let stack = ws.try_find_stack_by_id(stack_id)?;
    if request.preceding_head.is_some() {
        return Err(anyhow!(
            "BUG: cannot have preceding head name set - let's use the new API instead"
        ));
    }

    let new_ws = but_workspace::branch::create_reference(
        new_ref.as_ref(),
        {
            use but_workspace::branch::create_reference::Position::Above;
            let segment = stack.segments.first().context("BUG: no empty stacks")?;
            segment
                .ref_info
                .as_ref()
                .map(
                    |ri| but_workspace::branch::create_reference::Anchor::AtSegment {
                        ref_name: Cow::Borrowed(ri.ref_name.as_ref()),
                        position: Above,
                    },
                )
                .or_else(|| {
                    Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                        commit_id: ws.tip_commit_by_segment_id(segment.id)?.id,
                        position: Above,
                    })
                })
                .with_context(|| {
                    format!(
                        "TODO: UI should migrate to new version of `create_branch()` instead,\
                            couldn't handle stack_id={stack_id:?}, request={request:?}"
                    )
                })?
        },
        &repo,
        &ws,
        &mut meta,
        |_| StackId::generate(),
        None, // order - not used for dependent branches
    )?;

    *ws = new_ws.into_owned();
    Ok(())
}

/// Remove a branch without creating an oplog snapshot.
///
/// This is the core implementation used by both [`remove_branch`] (which creates its own snapshot)
/// and batch operations like `but clean` (which create a single snapshot for multiple removals).
pub fn remove_branch_only(
    ctx: &mut Context,
    branch_name: &str,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let ref_name = Category::LocalBranch
        .to_full_name(branch_name)
        .map_err(anyhow::Error::from)?;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let new_ws = but_workspace::branch::remove_reference(
        ref_name.as_ref(),
        &repo,
        &ws,
        &mut meta,
        but_workspace::branch::remove_reference::Options {
            avoid_anonymous_stacks: true,
            keep_metadata: false,
        },
    )?;

    if let Some(new_ws) = new_ws {
        *ws = new_ws;
    }
    Ok(())
}

/// Remove a branch from a stack.
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// removal snapshot and detaching the branch.
///
/// This can only be called on a branch that's inside of a stack of multiple branches and is not the top branch,
/// or on a branch that's empty.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn remove_branch(ctx: &mut Context, stack_id: StackId, branch_name: String) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    remove_branch_with_perm(ctx, stack_id, branch_name, guard.write_permission())
}

/// Remove a branch from a stack while reusing caller-held exclusive access.
///
/// This records the dependent-branch removal snapshot and then delegates to
/// [`remove_branch_only()`] for the actual workspace mutation.
pub fn remove_branch_with_perm(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let _ = stack_id;
    ctx.snapshot_remove_dependent_branch(&branch_name, perm)
        .ok();
    remove_branch_only(ctx, &branch_name, perm)
}
