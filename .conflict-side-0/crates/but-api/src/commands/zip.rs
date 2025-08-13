use crate::{App, error::Error};
use anyhow::Context;
use gitbutler_error::{error, error::Code};
use serde::Deserialize;
use std::path::PathBuf;

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
    app.archival.archive(project_id).map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLogsArchivePathParams {}

pub fn get_logs_archive_path(
    app: &App,
    _params: GetLogsArchivePathParams,
) -> Result<PathBuf, Error> {
    app.archival.logs_archive().map_err(Into::into)
}
