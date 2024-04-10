use std::path::Path;
use std::vec;

use anyhow::{Context, Result};
use gitbutler_core::{
    deltas, gb_repository, project_repository,
    projects::{self, ProjectId},
    reader, sessions, users,
};

use super::events;

impl super::Handler {
    pub(super) fn calculate_deltas<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: ProjectId,
    ) -> Result<Vec<events::PrivateEvent>> {
        Self::calculate_deltas_pure(
            &self.local_data_dir,
            &self.projects,
            &self.users,
            path,
            project_id,
        )
    }

    // TODO(ST): ignored checks shouldn't be necessary here as `path` is only here because it's not ignored.
    //           Also it seems odd it fails if the file is ignored, and that it uses `reader::Error` even though
    //           itself just uses `std::io::Error`.
    fn file_content_if_not_ignored(
        project_repository: &project_repository::Repository,
        path: &Path,
    ) -> Result<reader::Content, reader::Error> {
        if project_repository.is_path_ignored(path).unwrap_or(false) {
            return Err(reader::Error::NotFound);
        }
        let full_path = project_repository.project().path.join(path);
        if !full_path.exists() {
            return Err(reader::Error::NotFound);
        }
        Ok(reader::Content::read_from_file(&full_path)?)
    }
}

/// Currently required to make functionality testable without requiring a `Handler` with all of its state.
impl super::Handler {
    pub fn calculate_deltas_pure<P: AsRef<Path>>(
        local_data_dir: &Path,
        projects: &projects::Controller,
        users: &users::Controller,
        path: P,
        project_id: ProjectId,
    ) -> Result<Vec<events::PrivateEvent>> {
        let project = projects.get(&project_id).context("failed to get project")?;
        let project_repository = project_repository::Repository::open(&project)
            .with_context(|| "failed to open project repository for project")?;
        let user = users.get_user().context("failed to get user")?;
        let gb_repository =
            gb_repository::Repository::open(local_data_dir, &project_repository, user.as_ref())
                .context("failed to open gb repository")?;

        // If current session's branch is not the same as the project's head, flush it first.
        if let Some(session) = gb_repository
            .get_current_session()
            .context("failed to get current session")?
        {
            let project_head = project_repository
                .get_head()
                .context("failed to get head")?;
            if session.meta.branch != project_head.name().map(|n| n.to_string()) {
                gb_repository
                    .flush_session(&project_repository, &session, user.as_ref())
                    .context(format!("failed to flush session {}", session.id))?;
            }
        }

        let path = path.as_ref();
        let current_wd_file_content =
            match Self::file_content_if_not_ignored(&project_repository, path) {
                Ok(content) => Some(content),
                Err(reader::Error::NotFound) => None,
                Err(err) => Err(err).context("failed to get file content")?,
            };
        let current_session = gb_repository
            .get_or_create_current_session()
            .context("failed to get or create current session")?;
        let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)
            .context("failed to get session reader")?;
        let latest_file_content = match current_session_reader.file(path) {
            Ok(content) => Some(content),
            Err(reader::Error::NotFound) => None,
            Err(err) => Err(err).context("failed to get file content")?,
        };

        let deltas_reader = deltas::Reader::new(&current_session_reader);
        let current_deltas = deltas_reader
            .read_file(path)
            .context("failed to get file deltas")?;
        let mut text_doc = deltas::Document::new(
            latest_file_content.as_ref(),
            current_deltas.unwrap_or_default(),
        )?;
        let new_delta = text_doc
            .update(current_wd_file_content.as_ref())
            .context("failed to calculate new deltas")?;

        let Some(new_delta) = new_delta else {
            tracing::debug!(%project_id, path = %path.display(), "no new deltas, ignoring");
            return Ok(vec![]);
        };

        let deltas = text_doc.get_deltas();
        let writer = deltas::Writer::new(&gb_repository).context("failed to open deltas writer")?;
        writer
            .write(path, &deltas)
            .context("failed to write deltas")?;

        match &current_wd_file_content {
            Some(reader::Content::UTF8(text)) => writer.write_wd_file(path, text),
            Some(_) => writer.write_wd_file(path, ""),
            None => writer.remove_wd_file(path),
        }?;

        Ok(vec![
            events::PrivateEvent::SessionFile((
                project_id,
                current_session.id,
                path.to_path_buf(),
                latest_file_content,
            )),
            events::PrivateEvent::Session(project_id, current_session.clone()),
            events::PrivateEvent::SessionDelta((
                project_id,
                current_session.id,
                path.to_path_buf(),
                new_delta.clone(),
            )),
        ])
    }
}
