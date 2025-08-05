use but_core::{RepositoryExt, settings::git::ui::GitConfigSettings};
use gitbutler_project::ProjectId;
use serde::Deserialize;

use crate::{IpcContext, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGbConfigParams {
    pub project_id: ProjectId,
}

pub fn get_gb_config(
    _ipc_ctx: &IpcContext,
    params: GetGbConfigParams,
) -> Result<GitConfigSettings, Error> {
    but_core::open_repo(gitbutler_project::get(params.project_id)?.path)?
        .git_settings()
        .map(Into::into)
        .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetGbConfigParams {
    pub project_id: ProjectId,
    pub config: GitConfigSettings,
}

pub fn set_gb_config(_ipc_ctx: &IpcContext, params: SetGbConfigParams) -> Result<(), Error> {
    but_core::open_repo(gitbutler_project::get(params.project_id)?.path)?
        .set_git_settings(&params.config.into())
        .map_err(Into::into)
}
