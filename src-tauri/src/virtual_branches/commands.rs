use anyhow::Context;
use tauri::{AppHandle, Manager};
use timed::timed;

use crate::{error::Error, project_repository::branch};

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
