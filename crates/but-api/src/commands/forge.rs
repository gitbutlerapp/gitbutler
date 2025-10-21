//! In place of commands.rs
use anyhow::Context;
use but_api_macros::api_cmd;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_forge::{
    forge::ForgeName,
    review::{ReviewTemplateFunctions, available_review_templates, get_review_template_functions},
};
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn pr_templates(project_id: ProjectId, forge: ForgeName) -> Result<Vec<String>, Error> {
    let project = gitbutler_project::get_validated(project_id)?;
    Ok(available_review_templates(project.worktree_dir()?, &forge))
}

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

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn determine_forge_from_url(url: String) -> Result<Option<ForgeName>, Error> {
    Ok(gitbutler_forge::determine_forge_from_url(&url))
}

#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub async fn list_reviews(
    project_id: ProjectId,
) -> Result<Vec<gitbutler_forge::review::ForgeReview>, Error> {
    list_reviews_cmd(project_id).await
}

pub async fn list_reviews_cmd(
    project_id: ProjectId,
) -> Result<Vec<gitbutler_forge::review::ForgeReview>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(&project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    gitbutler_forge::review::list_forge_reviews(
        &project.preferred_forge_user,
        &base_branch
            .forge_repo_info
            .context("No forge could be determined for this repository branch")?,
    )
    .await
    .map_err(Into::into)
}

#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub async fn publish_review(
    project_id: ProjectId,
    params: gitbutler_forge::review::CreateForgeReviewParams,
) -> Result<gitbutler_forge::review::ForgeReview, Error> {
    publish_review_cmd(PublishReviewParams { project_id, params }).await
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishReviewParams {
    pub project_id: ProjectId,
    pub params: gitbutler_forge::review::CreateForgeReviewParams,
}

pub async fn publish_review_cmd(
    PublishReviewParams { project_id, params }: PublishReviewParams,
) -> Result<gitbutler_forge::review::ForgeReview, Error> {
    let project = gitbutler_project::get(project_id)?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(&project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    gitbutler_forge::review::create_forge_review(
        &project.preferred_forge_user,
        &base_branch
            .forge_repo_info
            .context("No forge could be determined for this repository branch")?,
        &params,
    )
    .await
    .map_err(Into::into)
}
