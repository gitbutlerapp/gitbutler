use crate::RequestContext;
use anyhow::Context;
use gitbutler_forge::{
    forge::ForgeName,
    review::{ReviewTemplateFunctions, available_review_templates, get_review_template_functions},
};
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewTemplatesParams {
    project_id: ProjectId,
    forge: ForgeName,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewTemplateContentsParams {
    project_id: ProjectId,
    relative_path: PathBuf,
    forge: ForgeName,
}

pub fn get_available_review_templates(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ReviewTemplatesParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get_validated(params.project_id)?;
    let templates = available_review_templates(&project.path, &params.forge);
    Ok(serde_json::to_value(templates)?)
}

pub fn get_review_template_contents(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ReviewTemplateContentsParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get_validated(params.project_id)?;

    let ReviewTemplateFunctions {
        is_valid_review_template_path,
        ..
    } = get_review_template_functions(&params.forge);

    if !is_valid_review_template_path(&params.relative_path) {
        return Err(anyhow::format_err!(
            "Invalid review template path: {:?}",
            project.path.join(&params.relative_path)
        ));
    }

    let content = project
        .read_file_from_workspace(&params.relative_path)?
        .content
        .context("PR template was not valid UTF-8")?;

    Ok(serde_json::to_value(content)?)
}
