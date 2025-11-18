use std::path::PathBuf;

use crate::json::Error;
use gitbutler_project::ProjectId;

pub fn get_project_archive_path(
    archival: &but_feedback::Archival,
    project_id: ProjectId,
) -> Result<PathBuf, Error> {
    archival
        .zip_entire_repository(project_id)
        .map_err(Into::into)
}

pub fn get_anonymous_graph_path(
    archival: &but_feedback::Archival,
    project_id: ProjectId,
) -> Result<PathBuf, Error> {
    archival.zip_anonymous_graph(project_id).map_err(Into::into)
}

pub fn get_logs_archive_path(archival: &but_feedback::Archival) -> Result<PathBuf, Error> {
    archival.zip_logs().map_err(Into::into)
}
