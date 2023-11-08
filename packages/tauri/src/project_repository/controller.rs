use tauri::{AppHandle, Manager};

use crate::{error::Error, projects::ProjectId, sessions};

#[derive(Clone)]
pub struct Controller {
    sessions: sessions::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            sessions: value.state::<sessions::Controller>().inner().clone(),
        })
    }
}

impl Controller {
    pub fn flush(&self, project_id: &ProjectId) -> Result<(), FlushError> {
        //TODO: error if no current session ?
        let _session = self.sessions.flush(project_id)?;

        //TODO: events::Event::Session(*project_id, session),
        //TODO: events::Event::PushGitbutlerData(*project_id),
        //TODO: events::Event::PushProjectToGitbutler(*project_id),

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FlushError {
    #[error(transparent)]
    SessionsError(#[from] sessions::FlushError),
    #[error(transparent)]
    CurrentSessionError(#[from] sessions::CurrentSessionError),
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<FlushError> for Error {
    fn from(value: FlushError) -> Self {
        match value {
            FlushError::SessionsError(error) => Error::from(error),
            FlushError::CurrentSessionError(error) => Error::from(error),
            FlushError::Other(error) => {
                tracing::error!(?error);
                Error::Unknown
            }
        }
    }
}
