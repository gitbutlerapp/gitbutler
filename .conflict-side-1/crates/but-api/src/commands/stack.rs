use crate::commands::stack::create_reference::Anchor;
use crate::{App, error::Error};
use anyhow::{Context, anyhow};
use but_workspace::branch::{ReferenceAnchor, ReferencePosition};
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::SnapshotExt;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use gitbutler_user::User;
use gix::refs::Category;
use serde::Deserialize;
use std::borrow::Cow;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub request: CreateSeriesRequest,
}

pub mod create_reference {
    use crate::hex_hash::HexHash;
    use gitbutler_project::ProjectId;
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
            commit_id: HexHash,
            position: but_workspace::branch::ReferencePosition,
        },
        AtReference {
            short_name: String,
            position: but_workspace::branch::ReferencePosition,
        },
    }

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Params {
        pub project_id: ProjectId,
        pub request: Request,
    }
}

pub fn create_reference(app: &App, params: create_reference::Params) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let create_reference::Request { new_name, anchor } = params.request;
    let new_ref = Category::LocalBranch
        .to_full_name(new_name.as_str())
        .map_err(anyhow::Error::from)?;
    let anchor = anchor
        .map(|anchor| -> Result<_, Error> {
            Ok(match anchor {
                Anchor::AtCommit {
                    commit_id,
                    position,
                } => but_workspace::branch::ReferenceAnchor::AtCommit {
                    commit_id: commit_id.into(),
                    position,
                },
                Anchor::AtReference {
                    short_name,
                    position,
                } => but_workspace::branch::ReferenceAnchor::AtSegment {
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

    let (repo, mut meta, graph) = ctx.graph_and_meta_and_repo()?;
    _ = but_workspace::branch::create_reference(
        new_ref,
        anchor,
        &repo,
        &graph.to_workspace()?,
        &mut meta,
    )?;
    Ok(())
}

pub fn create_branch(app: &App, params: CreateBranchParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    if app.app_settings.get()?.feature_flags.ws3 {
        use ReferencePosition::Above;
        let (repo, mut meta, graph) = ctx.graph_and_meta_and_repo()?;
        let ws = graph.to_workspace()?;
        let stack = ws.try_find_stack_by_id(params.stack_id)?;
        let new_ref = Category::LocalBranch
            .to_full_name(params.request.name.as_str())
            .map_err(anyhow::Error::from)?;
        if params.request.preceding_head.is_some() {
            return Err(anyhow!(
                "BUG: cannot have preceding head name set - let's use the new API instead"
            )
            .into());
        }

        let mut guard = project.exclusive_worktree_access();
        ctx.snapshot_create_dependent_branch(&params.request.name, guard.write_permission())
            .ok();
        _ = but_workspace::branch::create_reference(
            new_ref.as_ref(),
            {
                let segment = stack.segments.first().context("BUG: no empty stacks")?;
                segment
                    .ref_name
                    .as_ref()
                    .map(|rn| ReferenceAnchor::AtSegment {
                        ref_name: Cow::Borrowed(rn.as_ref()),
                        position: Above,
                    })
                    .or_else(|| {
                        Some(ReferenceAnchor::AtCommit {
                            commit_id: graph.tip_skip_empty(segment.id)?.id,
                            position: Above,
                        })
                    })
                    .with_context(|| {
                        format!(
                            "TODO: UI should migrate to new version of `create_branch()` instead,\
                            couldn't handle {params:?}"
                        )
                    })?
            },
            &repo,
            &ws,
            &mut meta,
        )?;
    } else {
        // NOTE: locking is built-in here.
        gitbutler_branch_actions::stack::create_branch(&ctx, params.stack_id, params.request)?;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveBranchParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
}

pub fn remove_branch(app: &App, params: RemoveBranchParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    if app.app_settings.get()?.feature_flags.ws3 {
        let (repo, mut meta, graph) = ctx.graph_and_meta_and_repo()?;
        let ws = graph.to_workspace()?;
        let ref_name = Category::LocalBranch
            .to_full_name(params.branch_name.as_str())
            .map_err(anyhow::Error::from)?;
        let mut guard = project.exclusive_worktree_access();
        ctx.snapshot_remove_dependent_branch(&params.branch_name, guard.write_permission())
            .ok();
        but_workspace::branch::remove_reference(
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
    } else {
        gitbutler_branch_actions::stack::remove_branch(&ctx, params.stack_id, params.branch_name)?;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBranchNameParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
    pub new_name: String,
}

pub fn update_branch_name(app: &App, params: UpdateBranchNameParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_name(
        &ctx,
        params.stack_id,
        params.branch_name,
        params.new_name,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBranchDescriptionParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
    pub description: Option<String>,
}

pub fn update_branch_description(
    app: &App,
    params: UpdateBranchDescriptionParams,
) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_description(
        &ctx,
        params.stack_id,
        params.branch_name,
        params.description,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBranchPrNumberParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub branch_name: String,
    pub pr_number: Option<usize>,
}

pub fn update_branch_pr_number(app: &App, params: UpdateBranchPrNumberParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::update_branch_pr_number(
        &ctx,
        params.stack_id,
        params.branch_name,
        params.pr_number,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushStackParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub with_force: bool,
    pub force_push_protection: bool,
    pub branch: String,
}

pub fn push_stack(app: &App, params: PushStackParams) -> Result<PushResult, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    gitbutler_branch_actions::stack::push_stack(
        &ctx,
        params.stack_id,
        params.with_force,
        params.force_push_protection,
        params.branch,
    )
    .map_err(|e| e.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushStackToReviewParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub top_branch: String,
    pub user: User,
}

pub fn push_stack_to_review(app: &App, params: PushStackToReviewParams) -> Result<String, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let review_id = gitbutler_sync::stack_upload::push_stack_to_review(
        &ctx,
        &params.user,
        params.stack_id,
        params.top_branch,
    )?;

    Ok(review_id)
}
