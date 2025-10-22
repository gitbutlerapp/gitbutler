use std::borrow::Cow;

use anyhow::{Context, anyhow};
use but_api_macros::api_cmd;
use but_settings::AppSettings;
use gitbutler_branch_actions::{internal::PushResult, stack::CreateSeriesRequest};
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::SnapshotExt;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use gix::refs::Category;
use tracing::instrument;

use crate::error::Error;

pub mod create_reference {
    use serde::{Deserialize, Serialize};

    use crate::hex_hash::HexHash;

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
            commit_id: HexHash,
            position: but_workspace::branch::create_reference::Position,
        },
        AtReference {
            short_name: String,
            position: but_workspace::branch::create_reference::Position,
        },
    }
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn create_reference(
    project_id: ProjectId,
    request: create_reference::Request,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let create_reference::Request { new_name, anchor } = request;
    let new_ref = Category::LocalBranch
        .to_full_name(new_name.as_str())
        .map_err(anyhow::Error::from)?;
    let anchor = anchor
        .map(|anchor| -> Result<_, Error> {
            Ok(match anchor {
                create_reference::Anchor::AtCommit {
                    commit_id,
                    position,
                } => but_workspace::branch::create_reference::Anchor::AtCommit {
                    commit_id: commit_id.into(),
                    position,
                },
                create_reference::Anchor::AtReference {
                    short_name,
                    position,
                } => but_workspace::branch::create_reference::Anchor::AtSegment {
                    ref_name: Cow::Owned(
                        Category::LocalBranch
                            .to_full_name(short_name.as_str())
                            .map_err(anyhow::Error::from)?,
                    ),
                    position,
                },
            })
        })
        .transpose()?;

    let mut guard = ctx.project().exclusive_worktree_access();
    let (repo, mut meta, graph) = ctx.graph_and_meta_mut_and_repo(guard.write_permission())?;
    _ = but_workspace::branch::create_reference(
        new_ref,
        anchor,
        &repo,
        &graph.to_workspace()?,
        &mut *meta,
    )?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn create_branch(
    project_id: ProjectId,
    stack_id: StackId,
    request: CreateSeriesRequest,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    if ctx.app_settings().feature_flags.ws3 {
        use but_workspace::branch::create_reference::Position::Above;
        let mut guard = project.exclusive_worktree_access();
        let (repo, mut meta, graph) = ctx.graph_and_meta_mut_and_repo(guard.write_permission())?;
        let ws = graph.to_workspace()?;
        let stack = ws.try_find_stack_by_id(stack_id)?;
        let new_ref = Category::LocalBranch
            .to_full_name(request.name.as_str())
            .map_err(anyhow::Error::from)?;
        if request.preceding_head.is_some() {
            return Err(anyhow!(
                "BUG: cannot have preceding head name set - let's use the new API instead"
            )
            .into());
        }

        ctx.snapshot_create_dependent_branch(&request.name, guard.write_permission())
            .ok();
        _ =
            but_workspace::branch::create_reference(
                new_ref.as_ref(),
                {
                    let segment = stack.segments.first().context("BUG: no empty stacks")?;
                    segment
                    .ref_name
                    .as_ref()
                    .map(|rn| but_workspace::branch::create_reference::Anchor::AtSegment {
                        ref_name: Cow::Borrowed(rn.as_ref()),
                        position: Above,
                    })
                    .or_else(|| {
                        Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                            commit_id: graph.tip_skip_empty(segment.id)?.id,
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
                &mut *meta,
            )?;
    } else {
        // NOTE: locking is built-in here.
        gitbutler_branch_actions::stack::create_branch(&ctx, stack_id, request)?;
    }
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn remove_branch(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();
    if ctx.app_settings().feature_flags.ws3 {
        let (repo, mut meta, graph) = ctx.graph_and_meta_mut_and_repo(guard.write_permission())?;
        let ws = graph.to_workspace()?;
        let ref_name = Category::LocalBranch
            .to_full_name(branch_name.as_str())
            .map_err(anyhow::Error::from)?;
        ctx.snapshot_remove_dependent_branch(&branch_name, guard.write_permission())
            .ok();
        but_workspace::branch::remove_reference(
            ref_name.as_ref(),
            &repo,
            &ws,
            &mut *meta,
            but_workspace::branch::remove_reference::Options {
                avoid_anonymous_stacks: true,
                // The UI kind of keeps it, but we can't do that somehow
                // the object id is null, and stuff breaks. Fine for now.
                // Delete is delete.
                keep_metadata: false,
            },
        )?;
    } else {
        gitbutler_branch_actions::stack::remove_branch(&ctx, stack_id, branch_name)?;
    }
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn update_branch_name(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::stack::update_branch_name(&ctx, stack_id, branch_name, new_name)?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn update_branch_description(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    description: Option<String>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::stack::update_branch_description(
        &ctx,
        stack_id,
        branch_name,
        description,
    )?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn update_branch_pr_number(
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::stack::update_branch_pr_number(
        &ctx,
        stack_id,
        branch_name,
        pr_number,
    )?;
    Ok(())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn push_stack(
    project_id: ProjectId,
    stack_id: StackId,
    with_force: bool,
    skip_force_push_protection: bool,
    branch: String,
    run_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
) -> Result<PushResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    gitbutler_branch_actions::stack::push_stack(
        &mut ctx,
        stack_id,
        with_force,
        skip_force_push_protection,
        branch,
        run_hooks,
        push_opts,
    )
    .map_err(|e| e.into())
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn push_stack_to_review(
    project_id: ProjectId,
    stack_id: StackId,
    top_branch: String,
    user: User,
) -> Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let review_id =
        gitbutler_sync::stack_upload::push_stack_to_review(&ctx, &user, stack_id, top_branch)?;

    Ok(review_id)
}
