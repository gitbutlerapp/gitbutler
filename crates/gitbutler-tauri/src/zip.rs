#![allow(clippy::used_underscore_binding)]
use std::path::PathBuf;

use but_api::json::Error;
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_project_archive_path(
    archival: State<'_, but_feedback::Archival>,
    project_id: ProjectHandleOrLegacyProjectId,
) -> Result<PathBuf, Error> {
    archival
        .zip_entire_repository(project_id)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_anonymous_graph_path(
    archival: State<'_, but_feedback::Archival>,
    project_id: ProjectHandleOrLegacyProjectId,
) -> Result<PathBuf, Error> {
    let ctx: Context = project_id.try_into()?;
    let _guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    let meta = ctx.meta()?;
    archival
        .zip_anonymous_graph(&repo, &meta)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(archival), err(Debug))]
pub fn get_logs_archive_path(
    archival: State<'_, but_feedback::Archival>,
) -> Result<PathBuf, Error> {
    archival.zip_logs().map_err(Into::into)
}
