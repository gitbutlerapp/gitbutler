use std::path::PathBuf;

use anyhow::Context;
use gitbutler_error::{error, error::Code};
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProjectArchivePathParams {
    pub project_id: String,
}

pub fn get_project_archive_path(
    app: &App,
    params: GetProjectArchivePathParams,
) -> Result<PathBuf, Error> {
    let project_id = params
        .project_id
        .parse()
        .context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
    app.archival
        .zip_entire_repository(project_id)
        .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAnonymousGraphPathParams {
    pub project_id: String,
}

pub fn get_anonymous_graph_path(
    app: &App,
    params: GetAnonymousGraphPathParams,
) -> Result<PathBuf, Error> {
    let project_id = params
        .project_id
        .parse()
        .context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
    app.archival
        .zip_anonymous_graph(project_id)
        .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLogsArchivePathParams {}

pub fn get_logs_archive_path(
    app: &App,
    _params: GetLogsArchivePathParams,
) -> Result<PathBuf, Error> {
    app.archival.zip_logs().map_err(Into::into)
}
