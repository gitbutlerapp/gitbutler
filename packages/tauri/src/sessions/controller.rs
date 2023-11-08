use anyhow::Context;
use tauri::AppHandle;

use crate::{
    gb_repository, paths, project_repository,
    projects::{self, ProjectId},
    users,
};

use super::{Database, Session};

pub struct Controller {
    local_data_dir: paths::DataDir,
    sessions_database: Database,

    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: paths::DataDir::try_from(value)?,
            sessions_database: Database::from(value),
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error(transparent)]
    ProjectsError(#[from] projects::GetError),
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
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open project repository")?;
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
