use but_api::{commands::repo, IpcContext};
use but_graph::virtual_branches_legacy_types::BranchOwnershipClaims;
use but_workspace::DiffSpec;
use gitbutler_branch_actions::RemoteBranchFile;
use gitbutler_project::ProjectId;
use gitbutler_repo::hooks::{HookResult, MessageHookResult};
use gitbutler_repo::FileInfo;
use std::path::Path;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_get_local_config(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    key: String,
) -> Result<Option<String>, Error> {
    repo::git_get_local_config(&ipc_ctx, repo::GitGetLocalConfigParams { project_id, key })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_set_local_config(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    key: String,
    value: String,
) -> Result<(), Error> {
    repo::git_set_local_config(
        &ipc_ctx,
        repo::GitSetLocalConfigParams {
            project_id,
            key,
            value,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn check_signing_settings(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<bool, Error> {
    repo::check_signing_settings(&ipc_ctx, repo::CheckSigningSettingsParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn git_clone_repository(
    ipc_ctx: State<IpcContext>,
    repository_url: String,
    target_dir: &Path,
) -> Result<(), Error> {
    repo::git_clone_repository(
        &ipc_ctx,
        repo::GitCloneRepositoryParams {
            repository_url,
            target_dir: target_dir.to_path_buf(),
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn get_uncommited_files(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<Vec<RemoteBranchFile>, Error> {
    repo::get_uncommitted_files(&ipc_ctx, repo::GetUncommittedFilesParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn get_commit_file(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    relative_path: &Path,
    commit_id: String,
) -> Result<FileInfo, Error> {
    repo::get_commit_file(
        &ipc_ctx,
        repo::GetCommitFileParams {
            project_id,
            relative_path: relative_path.to_path_buf(),
            commit_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn get_workspace_file(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    relative_path: &Path,
) -> Result<FileInfo, Error> {
    repo::get_workspace_file(
        &ipc_ctx,
        repo::GetWorkspaceFileParams {
            project_id,
            relative_path: relative_path.to_path_buf(),
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn pre_commit_hook(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    ownership: BranchOwnershipClaims,
) -> Result<HookResult, Error> {
    repo::pre_commit_hook(
        &ipc_ctx,
        repo::PreCommitHookParams {
            project_id,
            ownership,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn pre_commit_hook_diffspecs(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    changes: Vec<DiffSpec>,
) -> Result<HookResult, Error> {
    repo::pre_commit_hook_diffspecs(
        &ipc_ctx,
        repo::PreCommitHookDiffspecsParams {
            project_id,
            changes,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn post_commit_hook(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
) -> Result<HookResult, Error> {
    repo::post_commit_hook(&ipc_ctx, repo::PostCommitHookParams { project_id })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn message_hook(
    ipc_ctx: State<IpcContext>,
    project_id: ProjectId,
    message: String,
) -> Result<MessageHookResult, Error> {
    repo::message_hook(
        &ipc_ctx,
        repo::MessageHookParams {
            project_id,
            message,
        },
    )
}
