use std::path;

use anyhow::Context;
use tauri::{AppHandle, Manager};

use crate::{
    gb_repository, project_repository,
    projects::{self, ProjectId},
    users,
};

use super::{Database, Session};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: path::PathBuf,
    sessions_database: Database,

    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(controller) = value.try_state::<Controller>() {
            Ok(controller.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            Ok(Self {
                local_data_dir: app_data_dir,
                sessions_database: Database::try_from(value)?,
                projects: projects::Controller::try_from(value)?,
                users: users::Controller::try_from(value)?,
            })
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error(transparent)]
    ProjectsError(#[from] projects::GetError),
    #[error(transparent)]
    ProjectRepositoryError(#[from] project_repository::OpenError),
    #[error(transparent)]
    UsersError(#[from] users::GetError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Controller {
    pub fn list(
        &self,
        project_id: &ProjectId,
        earliest_timestamp_ms: Option<u128>,
    ) -> Result<Vec<Session>, ListError> {
        let sessions = self
            .sessions_database
            .list_by_project_id(project_id, earliest_timestamp_ms)?;

        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repository")?;

        // this is a hack to account for a case when we have a session created, but fs was never
        // touched, so the wathcer never picked up the session
        let current_session = gb_repository
            .get_current_session()
            .context("failed to get current session")?;
        let have_to_index = matches!(
            (current_session.as_ref(), sessions.first()),
            (Some(_), None)
        );
        if !have_to_index {
            return Ok(sessions);
        }

        let sessions_iter = gb_repository.get_sessions_iterator()?;
        let mut sessions = sessions_iter.collect::<Result<Vec<_>, _>>()?;
        self.sessions_database
            .insert(project_id, &sessions.iter().collect::<Vec<_>>())?;
        if let Some(session) = current_session {
            self.sessions_database.insert(project_id, &[&session])?;
            sessions.insert(0, session);
        }
        Ok(sessions)
    }
}
