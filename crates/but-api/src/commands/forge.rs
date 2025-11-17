//! In place of commands.rs
use anyhow::Context;
use but_api_macros::api_cmd;
use but_core::RepositoryExt;
use but_forge::{
    ForgeName, ReviewTemplateFunctions, available_review_templates, get_review_template_functions,
};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use tracing::instrument;

use crate::json::Error;

/// (Deprecated) Get the list of PR template paths for the given project and forge.
/// This function is deprecated in favor of `list_available_review_templates`.
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn pr_templates(project_id: ProjectId, forge: ForgeName) -> Result<Vec<String>, Error> {
    let project = gitbutler_project::get_validated(project_id)?;
    Ok(available_review_templates(project.worktree_dir()?, &forge))
}

/// Get the list of review template paths for the given project.
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn list_available_review_templates(project_id: ProjectId) -> Result<Vec<String>, Error> {
    let project = gitbutler_project::get_validated(project_id)?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(&project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let forge = &base_branch
        .forge_repo_info
        .as_ref()
        .context("No forge could be determined for this repository branch")?
        .forge;

    Ok(available_review_templates(project.worktree_dir()?, forge))
}

/// (Deprecated) Get the PR template content for the given project and relative path.
///
/// This function is deprecated in favor of `review_template`, which serves the same purpose
/// but uses the updated storage location.
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn pr_template(
    project_id: ProjectId,
    relative_path: std::path::PathBuf,
    forge: ForgeName,
) -> Result<String, Error> {
    let project = gitbutler_project::get_validated(project_id)?;

    let ReviewTemplateFunctions {
        is_valid_review_template_path,
        ..
    } = get_review_template_functions(&forge);

    if !is_valid_review_template_path(&relative_path) {
        return Err(anyhow::format_err!(
            "Invalid review template path: {:?}",
            project.worktree_dir()?.join(relative_path),
        )
        .into());
    }
    Ok(project
        .read_file_from_workspace(&relative_path)?
        .content
        .context("PR template was not valid UTF-8")?)
}

/// Information about the project's review template.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewTemplateInfo {
    /// The relative path to the review template within the repository.
    pub path: String,
    /// The content of the review template.
    pub content: String,
}

/// Get the review template content for the given project and relative path.
///
/// This function determines the forge of a project and retrieves the review template
/// from the git config.
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn review_template(project_id: ProjectId) -> Result<Option<ReviewTemplateInfo>, Error> {
    let project = gitbutler_project::get_validated(project_id)?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(&project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let forge = &base_branch
        .forge_repo_info
        .as_ref()
        .context("No forge could be determined for this repository branch")?
        .forge;

    match ctx
        .gix_repo()?
        .git_settings()?
        .gitbutler_forge_review_template_path
    {
        Some(review_template_path) => {
            let ReviewTemplateFunctions {
                is_valid_review_template_path,
                ..
            } = get_review_template_functions(forge);
            let template_path = review_template_path.to_string();
            let path = std::path::PathBuf::from(&template_path);

            if !is_valid_review_template_path(&path) {
                return Err(anyhow::format_err!(
                    "Invalid review template path: {:?}",
                    project.worktree_dir()?.join(path),
                )
                .into());
            }
            let content = project
                .read_file_from_workspace(&path)?
                .content
                .context("PR template was not valid UTF-8")?;

            Ok(Some(ReviewTemplateInfo {
                path: template_path,
                content,
            }))
        }
        None => Ok(None),
    }
}

/// Set the review template path in the git configuration for the given project.
/// The template path will be validated.
#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn set_review_template(
    project_id: ProjectId,
    template_path: Option<String>,
) -> Result<(), Error> {
    let project = gitbutler_project::get_validated(project_id)?;
    let repo = project.open_isolated()?;
    let mut git_config = repo.git_settings()?;

    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(&project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let forge = &base_branch
        .forge_repo_info
        .as_ref()
        .context("No forge could be determined for this repository branch")?
        .forge;

    let ReviewTemplateFunctions {
        is_valid_review_template_path,
        ..
    } = get_review_template_functions(forge);

    if let Some(ref path) = template_path {
        let path_buf = std::path::PathBuf::from(path);
        if !is_valid_review_template_path(&path_buf) {
            return Err(anyhow::format_err!(
                "Invalid review template path: {:?}",
                project.worktree_dir()?.join(&path_buf),
            )
            .into());
        }
    }

    git_config.gitbutler_forge_review_template_path = template_path.map(|p| p.into());
    repo.set_git_settings(&git_config).map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn determine_forge_from_url(url: String) -> Result<Option<ForgeName>, Error> {
    Ok(but_forge::determine_forge_from_url(&url))
}

#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub async fn list_reviews(project_id: ProjectId) -> Result<Vec<but_forge::ForgeReview>, Error> {
    list_reviews_cmd(ListReviewsParams { project_id }).await
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListReviewsParams {
    pub project_id: ProjectId,
}

pub async fn list_reviews_cmd(
    ListReviewsParams { project_id }: ListReviewsParams,
) -> Result<Vec<but_forge::ForgeReview>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(&project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forge::list_forge_reviews(
        &project.preferred_forge_user,
        &base_branch
            .forge_repo_info
            .context("No forge could be determined for this repository branch")?,
        &storage,
    )
    .await
    .map_err(Into::into)
}

#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub async fn publish_review(
    project_id: ProjectId,
    params: but_forge::CreateForgeReviewParams,
) -> Result<but_forge::ForgeReview, Error> {
    publish_review_cmd(PublishReviewParams { project_id, params }).await
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishReviewParams {
    pub project_id: ProjectId,
    pub params: but_forge::CreateForgeReviewParams,
}

pub async fn publish_review_cmd(
    PublishReviewParams { project_id, params }: PublishReviewParams,
) -> Result<but_forge::ForgeReview, Error> {
    let project = gitbutler_project::get(project_id)?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(&project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    but_forge::create_forge_review(
        &project.preferred_forge_user,
        &base_branch
            .forge_repo_info
            .context("No forge could be determined for this repository branch")?,
        &params,
        &storage,
    )
    .await
    .map_err(Into::into)
}
