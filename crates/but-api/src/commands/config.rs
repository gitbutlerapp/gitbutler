use but_core::{RepositoryExt, settings::git::ui::GitConfigSettings};
use gitbutler_project::ProjectId;
use gitbutler_serde::bstring_opt_lossy;
use gix::bstr::BString;
use serde::Serialize;

use crate::error::Error;
use but_api_macros::api_cmd;

#[api_cmd]
pub fn get_gb_config(project_id: ProjectId) -> Result<GitConfigSettings, Error> {
    but_core::open_repo(gitbutler_project::get(project_id)?.path)?
        .git_settings()
        .map(Into::into)
        .map_err(Into::into)
}

#[api_cmd]
pub fn set_gb_config(project_id: ProjectId, config: GitConfigSettings) -> Result<(), Error> {
    but_core::open_repo(gitbutler_project::get(project_id)?.path)?
        .set_git_settings(&config.into())
        .map_err(Into::into)
}

#[api_cmd]
pub fn store_author_globally_if_unset(
    project_id: ProjectId,
    name: String,
    email: String,
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

#[api_cmd]
/// Return the Git author information as the project repository would see it.
pub fn get_author_info(project_id: ProjectId) -> Result<AuthorInfo, Error> {
    let repo = but_core::open_repo(gitbutler_project::get(project_id)?.path)?;
    let (name, email) = repo
        .author()
        .transpose()
        .map_err(anyhow::Error::from)?
        .map(|author| (Some(author.name.to_owned()), Some(author.email.to_owned())))
        .unwrap_or_default();
    Ok(AuthorInfo { name, email })
}
