use anyhow::Context;
use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::{assets, error::Error, git};

use super::{
    branch::Ownership,
    controller::{self, Controller},
    RemoteBranchFile,
};

impl From<controller::Error> for Error {
    fn from(value: controller::Error) -> Self {
        match value {
            controller::Error::GetProject(error) => Error::from(error),
            controller::Error::ProjectRemote(error) => Error::from(error),
            controller::Error::OpenProjectRepository(error) => Error::from(error),
            controller::Error::Verify(error) => Error::from(error),
            controller::Error::Conflicting => Error::UserError {
                code: crate::error::Code::ProjectConflict,
                message: "Project is in a conflicting state".to_string(),
            },
            controller::Error::Other(error) => {
                tracing::error!(?error);
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn commit_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch: &str,
    message: &str,
    ownership: Option<Ownership>,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .create_commit(project_id, branch, message, ownership.as_ref())
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
pub async fn merge_virtual_branch_upstream(
    handle: AppHandle,
    project_id: &str,
    branch: &str,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .merge_virtual_branch_upstream(project_id, branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_base_branch_data(
    handle: AppHandle,
    project_id: &str,
) -> Result<Option<super::BaseBranch>, Error> {
    if let Some(base_branch) = handle
        .state::<Controller>()
        .get_base_branch_data(project_id)
        .await?
    {
        let proxy = handle.state::<assets::Proxy>();
        let base_branch = proxy.proxy_base_branch(&base_branch).await;
        Ok(Some(base_branch))
    } else {
        Ok(None)
    }
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
    let base_branch = handle
        .state::<Controller>()
        .set_base_branch(project_id, &branch_name)
        .await?;
    let base_branch = handle
        .state::<assets::Proxy>()
        .proxy_base_branch(&base_branch)
        .await;
    Ok(base_branch)
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
pub async fn unapply_ownership(
    handle: AppHandle,
    project_id: &str,
    ownership: Ownership,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .unapply_ownership(project_id, &ownership)
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

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn can_apply_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch_id: &str,
) -> Result<bool, Error> {
    handle
        .state::<Controller>()
        .can_apply_virtual_branch(project_id, branch_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn can_apply_remote_branch(
    handle: AppHandle,
    project_id: &str,
    branch: git::BranchName,
) -> Result<bool, Error> {
    handle
        .state::<Controller>()
        .can_apply_remote_branch(project_id, &branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_remote_commit_files(
    handle: AppHandle,
    project_id: &str,
    commit_oid: git::Oid,
) -> Result<Vec<RemoteBranchFile>, Error> {
    handle
        .state::<Controller>()
        .list_remote_commit_files(project_id, commit_oid)
        .await
        .map_err(Into::into)
}
