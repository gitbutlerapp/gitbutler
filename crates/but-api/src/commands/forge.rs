//! In place of commands.rs
use std::path::Path;

use anyhow::Context;
use gitbutler_forge::{
    forge::ForgeName,
    review::{ReviewTemplateFunctions, available_review_templates, get_review_template_functions},
};
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrTemplatesParams {
    pub project_id: ProjectId,
    pub forge: ForgeName,
}

pub fn pr_templates(_app: &App, params: PrTemplatesParams) -> Result<Vec<String>, Error> {
    let project = gitbutler_project::get_validated(params.project_id)?;
    Ok(available_review_templates(&project.path, &params.forge))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrTemplateParams {
    pub project_id: ProjectId,
    pub relative_path: std::path::PathBuf,
    pub forge: ForgeName,
}

pub fn pr_template(_app: &App, params: PrTemplateParams) -> Result<String, Error> {
    let project = gitbutler_project::get_validated(params.project_id)?;

    let ReviewTemplateFunctions {
        is_valid_review_template_path,
        ..
    } = get_review_template_functions(&params.forge);

    if !is_valid_review_template_path(&params.relative_path) {
        return Err(anyhow::format_err!(
            "Invalid review template path: {:?}",
            Path::join(&project.path, &params.relative_path)
        )
        .into());
    }
    Ok(project
        .read_file_from_workspace(&params.relative_path)?
        .content
        .context("PR template was not valid UTF-8")?)
}
