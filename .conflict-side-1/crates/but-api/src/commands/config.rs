use but_core::{RepositoryExt, settings::git::ui::GitConfigSettings};
use gitbutler_project::ProjectId;
use gitbutler_serde::bstring_opt_lossy;
use gix::bstr::BString;
use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGbConfigParams {
    pub project_id: ProjectId,
}

pub fn get_gb_config(params: GetGbConfigParams) -> Result<GitConfigSettings, Error> {
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

pub fn set_gb_config(params: SetGbConfigParams) -> Result<(), Error> {
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

/// Represents the author information from the git configuration.
#[derive(Debug, Clone, Serialize)]
pub struct AuthorInfo {
    /// The name of the author.
    #[serde(with = "bstring_opt_lossy")]
    pub name: Option<BString>,
    /// The email of the author.
    #[serde(with = "bstring_opt_lossy")]
    pub email: Option<BString>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthorInfoParams {
    pub project_id: ProjectId,
}

/// Return the Git author information as the project repository would see it.
pub fn get_author_info(params: GetAuthorInfoParams) -> Result<AuthorInfo, Error> {
    let repo = but_core::open_repo(gitbutler_project::get(params.project_id)?.path)?;
    let (name, email) = repo
        .author()
        .transpose()
        .map_err(anyhow::Error::from)?
        .map(|author| (Some(author.name.to_owned()), Some(author.email.to_owned())))
        .unwrap_or_default();
    Ok(AuthorInfo { name, email })
}
