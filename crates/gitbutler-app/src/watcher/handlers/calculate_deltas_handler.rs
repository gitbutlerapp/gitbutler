use std::{path, vec};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use gitbutler::{
    deltas, gb_repository, project_repository,
    projects::{self, ProjectId},
    reader, sessions, users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let handler = Self::new(
                app_data_dir,
                value.state::<projects::Controller>().inner().clone(),
                value.state::<users::Controller>().inner().clone(),
            );
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

impl Handler {
    fn new(
        local_data_dir: path::PathBuf,
        projects: projects::Controller,
        users: users::Controller,
    ) -> Self {
        Self {
            local_data_dir,
            projects,
            users,
        }
    }

    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self::new(
            path.as_ref().to_path_buf(),
            projects::Controller::from_path(&path),
            users::Controller::from_path(path),
        )
    }

    // Returns Some(file_content) or None if the file is ignored.
    fn get_current_file(
        project_repository: &project_repository::Repository,
        path: &std::path::Path,
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

    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;

        let project_repository = project_repository::Repository::open(&project)
            .with_context(|| "failed to open project repository for project")?;

        let user = self.users.get_user().context("failed to get user")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
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

        let current_wd_file_content = match Self::get_current_file(&project_repository, path) {
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

        if let Some(new_delta) = new_delta {
            let deltas = text_doc.get_deltas();

            let writer =
                deltas::Writer::new(&gb_repository).context("failed to open deltas writer")?;
            writer
                .write(path, &deltas)
                .context("failed to write deltas")?;

            match &current_wd_file_content {
                Some(reader::Content::UTF8(text)) => writer.write_wd_file(path, text),
                Some(_) => writer.write_wd_file(path, ""),
                None => writer.remove_wd_file(path),
            }?;

            Ok(vec![
                events::Event::SessionFile((
                    *project_id,
                    current_session.id,
                    path.to_path_buf(),
                    latest_file_content,
                )),
                events::Event::Session(*project_id, current_session.clone()),
                events::Event::SessionDelta((
                    *project_id,
                    current_session.id,
                    path.to_path_buf(),
                    new_delta.clone(),
                )),
            ])
        } else {
            tracing::debug!(%project_id, path = %path.display(), "no new deltas, ignoring");
            Ok(vec![])
        }
    }
}
