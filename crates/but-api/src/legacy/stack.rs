use std::borrow::Cow;

use anyhow::{Context as _, Result, anyhow};
use but_api_macros::but_api;
use but_core::branch;
use but_ctx::Context;
use gitbutler_branch_actions::{internal::PushResult, stack::CreateSeriesRequest};
use gitbutler_oplog::SnapshotExt;
use gitbutler_stack::StackId;
use gix::refs::Category;
use tracing::instrument;

pub mod create_reference {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        /// The short name of the new branch, i.e. `foo` or `features/bar`
        pub new_name: String,
        /// If `None`, it's a new stack.
        pub anchor: Option<Anchor>,
    }

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(tag = "type", content = "subject", rename_all = "camelCase")]
    pub enum Anchor {
        AtCommit {
            commit_id: gix::ObjectId,
            position: but_workspace::branch::create_reference::Position,
        },
        AtReference {
            short_name: String,
            position: but_workspace::branch::create_reference::Position,
        },
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn create_reference(
    ctx: &mut Context,
    request: create_reference::Request,
) -> Result<(Option<StackId>, gix::refs::FullName)> {
    use bstr::ByteSlice;
    let create_reference::Request { new_name, anchor } = request;
    let new_ref = Category::LocalBranch
        .to_full_name(branch::normalize_short_name(new_name.as_str())?.as_bstr())
        .map_err(anyhow::Error::from)?;
    let anchor = anchor
        .map(|anchor| -> Result<_> {
            Ok(match anchor {
                create_reference::Anchor::AtCommit { commit_id, position } => {
                    but_workspace::branch::create_reference::Anchor::AtCommit { commit_id, position }
                }
                create_reference::Anchor::AtReference { short_name, position } => {
                    but_workspace::branch::create_reference::Anchor::AtSegment {
                        ref_name: Cow::Owned(
                            Category::LocalBranch
                                .to_full_name(short_name.as_str())
                                .map_err(anyhow::Error::from)?,
                        ),
                        position,
                    }
                }
            })
        })
        .transpose()?;

    let mut meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
    let new_ws = but_workspace::branch::create_reference(
        new_ref.clone(),
        anchor,
        &repo,
        &ws,
        &mut meta,
        |_| StackId::generate(),
        None,
    )?;

    let stack_id = new_ws
        .find_segment_and_stack_by_refname(new_ref.as_ref())
        .and_then(|(stack, _)| stack.id);

    *ws = new_ws.into_owned();
    Ok((stack_id, new_ref))
}

#[but_api]
#[instrument(err(Debug))]
pub fn create_branch(ctx: &mut Context, stack_id: StackId, request: CreateSeriesRequest) -> Result<()> {
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
                .map(|ri| but_workspace::branch::create_reference::Anchor::AtSegment {
                    ref_name: Cow::Borrowed(ri.ref_name.as_ref()),
                    position: Above,
                })
                .or_else(|| {
                    Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                        commit_id: ws.graph.tip_skip_empty(segment.id)?.id,
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

#[but_api]
#[instrument(err(Debug))]
pub fn remove_branch(ctx: &mut Context, stack_id: StackId, branch_name: String) -> Result<()> {
    let ref_name = Category::LocalBranch
        .to_full_name(branch_name.as_str())
        .map_err(anyhow::Error::from)?;
    let mut guard = ctx.exclusive_worktree_access();
    ctx.snapshot_remove_dependent_branch(&branch_name, guard.write_permission())
        .ok();
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
    let new_ws = but_workspace::branch::remove_reference(
        ref_name.as_ref(),
        &repo,
        &ws,
        &mut meta,
        but_workspace::branch::remove_reference::Options {
            avoid_anonymous_stacks: true,
            // The UI kind of keeps it, but we can't do that somehow
            // the object id is null, and stuff breaks. Fine for now.
            // Delete is delete.
            keep_metadata: false,
        },
    )?;

    if let Some(new_ws) = new_ws {
        *ws = new_ws;
    }
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_branch_name(ctx: &mut Context, stack_id: StackId, branch_name: String, new_name: String) -> Result<()> {
    gitbutler_branch_actions::stack::update_branch_name(ctx, stack_id, branch_name, new_name)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_branch_pr_number(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<()> {
    gitbutler_branch_actions::stack::update_branch_pr_number(ctx, stack_id, branch_name, pr_number)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn push_stack(
    ctx: &mut Context,
    stack_id: StackId,
    with_force: bool,
    skip_force_push_protection: bool,
    branch: String,
    run_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
) -> Result<PushResult> {
    gitbutler_branch_actions::stack::push_stack(
        ctx,
        stack_id,
        with_force,
        skip_force_push_protection,
        branch,
        run_hooks,
        push_opts,
    )
}
