//! In place of commands.rs
use anyhow::Context;
use but_api_macros::api_cmd;
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
