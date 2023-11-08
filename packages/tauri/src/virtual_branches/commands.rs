use anyhow::Context;
use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::{
    assets,
    error::{Code, Error},
    git,
};

use super::{
    branch::BranchId,
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
    ownership: Option<&str>,
) -> Result<git::Oid, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    let ownership = ownership
        .map(str::parse)
        .transpose()
        .map_err(|_| Error::UserError {
            code: Code::Validation,
            message: "Malformed ownership".to_string(),
        })?;
    handle
        .state::<Controller>()
        .create_commit(&project_id, &branch_id, message, ownership.as_ref())
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_virtual_branches(
    handle: AppHandle,
    project_id: &str,
) -> Result<Vec<super::VirtualBranch>, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branches = handle
        .state::<Controller>()
        .list_virtual_branches(&project_id)
        .await?;

    let proxy = handle.state::<assets::Proxy>();
    let branches = proxy.proxy_virtual_branches(branches).await;
    Ok(branches)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn create_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch: super::branch::BranchCreateRequest,
) -> Result<BranchId, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .create_virtual_branch(&project_id, &branch)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn create_virtual_branch_from_branch(
    handle: AppHandle,
    project_id: &str,
    branch: git::BranchName,
) -> Result<BranchId, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .create_virtual_branch_from_branch(&project_id, &branch)
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
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .merge_virtual_branch_upstream(&project_id, &branch_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_base_branch_data(
    handle: AppHandle,
    project_id: &str,
) -> Result<Option<super::BaseBranch>, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    if let Some(base_branch) = handle
        .state::<Controller>()
        .get_base_branch_data(&project_id)?
    {
        let proxy = handle.state::<assets::Proxy>();
        let base_branch = proxy.proxy_base_branch(base_branch).await;
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
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_name = format!("refs/remotes/{}", branch)
        .parse()
        .context("Invalid branch name")?;
    let base_branch = handle
        .state::<Controller>()
        .set_base_branch(&project_id, &branch_name)?;
    let base_branch = handle
        .state::<assets::Proxy>()
        .proxy_base_branch(base_branch)
        .await;
    Ok(base_branch)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn update_base_branch(handle: AppHandle, project_id: &str) -> Result<(), Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;
    handle
        .state::<Controller>()
        .update_base_branch(&project_id)
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
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .update_virtual_branch(&project_id, branch)
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
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .delete_virtual_branch(&project_id, &branch_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn apply_branch(handle: AppHandle, project_id: &str, branch: &str) -> Result<(), Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .apply_virtual_branch(&project_id, &branch_id)
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
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .unapply_virtual_branch(&project_id, &branch_id)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn unapply_ownership(
    handle: AppHandle,
    project_id: &str,
    ownership: &str,
) -> Result<(), Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let ownership = ownership.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed ownership".to_string(),
    })?;
    handle
        .state::<Controller>()
        .unapply_ownership(&project_id, &ownership)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn push_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch_id: &str,
    with_force: bool,
) -> Result<(), Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .push_virtual_branch(&project_id, &branch_id, with_force)
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
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .can_apply_virtual_branch(&project_id, &branch_id)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn can_apply_remote_branch(
    handle: AppHandle,
    project_id: &str,
    branch: &str,
) -> Result<bool, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch = branch.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch name".to_string(),
    })?;
    handle
        .state::<Controller>()
        .can_apply_remote_branch(&project_id, &branch)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_remote_commit_files(
    handle: AppHandle,
    project_id: &str,
    commit_oid: &str,
) -> Result<Vec<RemoteBranchFile>, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let commit_oid = commit_oid.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed commit oid".to_string(),
    })?;
    handle
        .state::<Controller>()
        .list_remote_commit_files(&project_id, commit_oid)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn reset_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch_id: &str,
    target_commit_oid: &str,
) -> Result<(), Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    let target_commit_oid = target_commit_oid.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed commit oid".to_string(),
    })?;
    handle
        .state::<Controller>()
        .reset_virtual_branch(&project_id, &branch_id, target_commit_oid)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn cherry_pick_onto_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch_id: &str,
    target_commit_oid: &str,
) -> Result<Option<git::Oid>, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branch_id = branch_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".to_string(),
    })?;
    let target_commit_oid = target_commit_oid.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed commit oid".to_string(),
    })?;
    handle
        .state::<Controller>()
        .cherry_pick(&project_id, &branch_id, target_commit_oid)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn amend_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch_id: &str,
    ownership: &str,
) -> Result<git::Oid, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;
    let branch_id = branch_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed branch id".into(),
    })?;
    let ownership = ownership.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed ownership".into(),
    })?;
    handle
        .state::<Controller>()
        .amend(&project_id, &branch_id, &ownership)
        .await
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_remote_branches(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<super::RemoteBranch>, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let branches = handle
        .state::<Controller>()
        .list_remote_branches(&project_id)?;
    let branches = handle
        .state::<assets::Proxy>()
        .proxy_remote_branches(branches)
        .await;
    Ok(branches)
}
