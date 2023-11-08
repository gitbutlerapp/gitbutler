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
        let session = self.sessions.current_session(project_id)?;

        if let Some(session) = session {
            self.sessions.flush(project_id, &session)?;
        }

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
