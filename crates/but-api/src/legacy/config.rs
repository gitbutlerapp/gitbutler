use anyhow::Result;
use but_api_macros::api_cmd_tauri;
use but_core::{RepositoryExt, settings::git::ui::GitConfigSettings};
use but_serde::bstring_opt_lossy;
use gitbutler_project::ProjectId;
use gix::bstr::BString;
use serde::Serialize;
use tracing::instrument;

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn get_gb_config(project_id: ProjectId) -> Result<GitConfigSettings> {
    gitbutler_project::get(project_id)?
        .open_repo()?
        .git_settings()
        .map(Into::into)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn set_gb_config(project_id: ProjectId, config: GitConfigSettings) -> Result<()> {
    gitbutler_project::get(project_id)?
        .open_repo()?
        .set_git_settings(&config.into())
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn store_author_globally_if_unset(
    project_id: ProjectId,
    name: String,
    email: String,
) -> Result<()> {
    let repo = gitbutler_project::get(project_id)?.open_repo()?;
    but_rebase::commit::save_author_if_unset_in_repo(
        &repo,
        gix::config::Source::User,
        name.as_str(),
        email.as_str(),
    )?;
    Ok(())
}

/// Represents the author information from the git configuration.
#[derive(Clone, Serialize)]
pub struct AuthorInfo {
    /// The name of the author.
    #[serde(with = "bstring_opt_lossy")]
    pub name: Option<BString>,
    /// The email of the author.
    #[serde(with = "bstring_opt_lossy")]
    pub email: Option<BString>,
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
/// Return the Git author information as the project repository would see it.
pub fn get_author_info(project_id: ProjectId) -> Result<AuthorInfo> {
    let repo = gitbutler_project::get(project_id)?.open_repo()?;
    let (name, email) = repo
        .author()
        .transpose()
        .map_err(anyhow::Error::from)?
        .map(|author| (Some(author.name.to_owned()), Some(author.email.to_owned())))
        .unwrap_or_default();
    Ok(AuthorInfo { name, email })
}
