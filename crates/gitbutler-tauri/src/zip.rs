#![allow(clippy::used_underscore_binding)]
use std::path::PathBuf;

use but_api::{json::Error, legacy::zip};
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_project_archive_path(
    archival: State<'_, but_feedback::Archival>,
    project_id: ProjectId,
) -> Result<PathBuf, Error> {
    zip::get_project_archive_path(&archival, project_id)
}

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_anonymous_graph_path(
    archival: State<'_, but_feedback::Archival>,
    project_id: ProjectId,
) -> Result<PathBuf, Error> {
    zip::get_anonymous_graph_path(&archival, project_id)
}

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_logs_archive_path(
    archival: State<'_, but_feedback::Archival>,
) -> Result<PathBuf, Error> {
    zip::get_logs_archive_path(&archival)
}
