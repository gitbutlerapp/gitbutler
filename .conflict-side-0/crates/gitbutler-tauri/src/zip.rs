#![allow(clippy::used_underscore_binding)]
use std::path::PathBuf;

use but_api::json::Error;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_project_archive_path(
    archival: State<'_, but_feedback::Archival>,
    project_id: ProjectId,
) -> Result<PathBuf, Error> {
    archival
        .zip_entire_repository(project_id)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_anonymous_graph_path(
    archival: State<'_, but_feedback::Archival>,
    project_id: ProjectId,
) -> Result<PathBuf, Error> {
    archival.zip_anonymous_graph(project_id).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_logs_archive_path(
    archival: State<'_, but_feedback::Archival>,
) -> Result<PathBuf, Error> {
    archival.zip_logs().map_err(Into::into)
}
