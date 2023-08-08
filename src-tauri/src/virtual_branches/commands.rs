use anyhow::Context;
use futures::future::join_all;
use tauri::{AppHandle, Manager};
use timed::timed;

use crate::{assets, error::Error, project_repository::branch};

use super::controller::Controller;

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn commit_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch: &str,
    message: &str,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .create_commit(project_id, branch, message)
        .await
        .context("failed to create commit")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn list_virtual_branches(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<super::VirtualBranch>, Error> {
    let branches = handle
        .state::<Controller>()
        .list_virtual_branches(project_id)
        .await
        .context("failed to list virtual branches")?;
    Ok(branches)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn create_virtual_branch(
    handle: tauri::AppHandle,
    project_id: &str,
    branch: super::branch::BranchCreateRequest,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .create_virtual_branch(project_id, &branch)
        .await
        .context("failed to create virtual branch")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn create_virtual_branch_from_branch(
    handle: tauri::AppHandle,
    project_id: &str,
    branch: branch::Name,
) -> Result<String, Error> {
    let branch_id = handle
        .state::<Controller>()
        .create_virtual_branch_from_branch(project_id, &branch)
        .await
        .context("failed to create virtual branch from branch")?;
    Ok(branch_id)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn get_base_branch_data(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Option<super::BaseBranch>, Error> {
    let controller = handle.state::<Controller>();
    let target = match controller
        .get_base_branch_data(project_id)
        .context("failed to get base branch data")?
    {
        None => return Ok(None),
        Some(target) => target,
    };

    let target = Some(super::BaseBranch {
        recent_commits: join_all(
            target
                .clone()
                .recent_commits
                .into_iter()
                .map(|commit| {
                    let proxy = handle.state::<assets::Proxy>();
                    async move {
                        super::VirtualBranchCommit {
                            author: super::Author {
                                gravatar_url: proxy
                                    .proxy(&commit.author.gravatar_url)
                                    .await
                                    .unwrap_or_else(|e| {
                                        log::error!("failed to proxy gravatar url: {:#}", e);
                                        commit.author.gravatar_url
                                    }),
                                ..commit.author
                            },
                            ..commit
                        }
                    }
                })
                .collect::<Vec<_>>(),
        )
        .await,
        upstream_commits: join_all(
            target
                .clone()
                .upstream_commits
                .into_iter()
                .map(|commit| {
                    let proxy = handle.state::<assets::Proxy>();
                    async move {
                        super::VirtualBranchCommit {
                            author: super::Author {
                                gravatar_url: proxy
                                    .proxy(&commit.author.gravatar_url)
                                    .await
                                    .unwrap_or_else(|e| {
                                        log::error!("failed to proxy gravatar url: {:#}", e);
                                        commit.author.gravatar_url
                                    }),
                                ..commit.author
                            },
                            ..commit
                        }
                    }
                })
                .collect::<Vec<_>>(),
        )
        .await,
        ..target
    });

    Ok(target)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn set_base_branch(
    handle: tauri::AppHandle,
    project_id: &str,
    branch: &str,
) -> Result<super::BaseBranch, Error> {
    let controller = handle.state::<Controller>();
    let target = controller
        .set_base_branch(project_id, branch)
        .await
        .context("failed to get target data")?;
    Ok(target)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn update_base_branch(handle: tauri::AppHandle, project_id: &str) -> Result<(), Error> {
    let controller = handle.state::<Controller>();
    controller
        .update_base_branch(project_id)
        .await
        .context("failed to update base branch")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn update_virtual_branch(
    handle: tauri::AppHandle,
    project_id: &str,
    branch: super::branch::BranchUpdateRequest,
) -> Result<(), Error> {
    let controller = handle.state::<Controller>();
    controller
        .update_virtual_branch(project_id, branch)
        .await
        .context("failed to update virtual branch")?;
    Ok(())
}
