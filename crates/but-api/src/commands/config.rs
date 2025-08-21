use but_core::{RepositoryExt, settings::git::ui::GitConfigSettings};
use gitbutler_project::ProjectId;
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGbConfigParams {
    pub project_id: ProjectId,
}

pub fn get_gb_config(_app: &App, params: GetGbConfigParams) -> Result<GitConfigSettings, Error> {
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

pub fn set_gb_config(_app: &App, params: SetGbConfigParams) -> Result<(), Error> {
    but_core::open_repo(gitbutler_project::get(params.project_id)?.path)?
        .set_git_settings(&params.config.into())
        .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreAuthorGloballyParams {
    pub project_id: ProjectId,
    pub name: String,
    pub email: String,
}

pub fn store_author_globally_if_unset(
    _app: &App,
    StoreAuthorGloballyParams {
        project_id,
        name,
        email,
    }: StoreAuthorGloballyParams,
) -> Result<(), Error> {
    let repo = but_core::open_repo(gitbutler_project::get(project_id)?.path)?;
    but_rebase::commit::save_author_if_unset_in_repo(
        &repo,
        gix::config::Source::User,
        name.as_str(),
        email.as_str(),
    )?;
    Ok(())
}
