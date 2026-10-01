use std::borrow::Cow;

use anyhow::{Context as _, Result, anyhow};
use but_api_macros::but_api;
use but_core::{branch, ref_metadata::StackId};
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
