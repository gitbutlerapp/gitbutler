use anyhow::Context;
use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::{error::Error, git};

use super::controller::Controller;

#[tauri::command(async)]
#[instrument(skip(handle))]
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
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_virtual_branches(
    handle: AppHandle,
    project_id: &str,
) -> Result<Vec<super::VirtualBranch>, Error> {
    handle
        .state::<Controller>()
        .list_virtual_branches(project_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn create_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch: super::branch::BranchCreateRequest,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .create_virtual_branch(project_id, &branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn create_virtual_branch_from_branch(
    handle: AppHandle,
    project_id: &str,
    branch: git::BranchName,
) -> Result<String, Error> {
    handle
        .state::<Controller>()
        .create_virtual_branch_from_branch(project_id, &branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_base_branch_data(
    handle: AppHandle,
    project_id: &str,
) -> Result<Option<super::BaseBranch>, Error> {
    handle
        .state::<Controller>()
        .get_base_branch_data(project_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn set_base_branch(
    handle: AppHandle,
    project_id: &str,
    branch: &str,
) -> Result<super::BaseBranch, Error> {
    let branch_name = format!("refs/remotes/{}", branch)
        .parse()
        .context("Invalid branch name")?;
    handle
        .state::<Controller>()
        .set_base_branch(project_id, &branch_name)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn update_base_branch(handle: AppHandle, project_id: &str) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .update_base_branch(project_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn update_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch: super::branch::BranchUpdateRequest,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .update_virtual_branch(project_id, branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn delete_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch_id: &str,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .delete_virtual_branch(project_id, branch_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn apply_branch(handle: AppHandle, project_id: &str, branch: &str) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .apply_virtual_branch(project_id, branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn unapply_branch(
    handle: AppHandle,
    project_id: &str,
    branch: &str,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .unapply_virtual_branch(project_id, branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn push_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch_id: &str,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .push_virtual_branch(project_id, branch_id)
        .await
        .map_err(Into::into)
}
