use tauri::AppHandle;

use super::{storage::Storage, User};

pub struct Controller {
    storage: Storage,
}

impl From<&AppHandle> for Controller {
    fn from(app: &AppHandle) -> Self {
        Self {
            storage: Storage::from(app),
        }
    }
}

impl Controller {
    pub fn set_user(&self, user: &User) -> Result<(), SetError> {
        self.storage
            .set(user)
            .map_err(|error| SetError::Other(error.into()))
    }

    pub fn get_user(&self) -> Result<Option<User>, GetError> {
        self.storage.get().map_err(|error| match error {
            error => GetError::Other(error.into()),
        })
    }

    pub fn delete_user(&self) -> Result<(), DeleteError> {
        self.storage.delete().map_err(|error| match error {
            error => DeleteError::Other(error.into()),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error(transparent)]
    Other(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error(transparent)]
    Other(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
