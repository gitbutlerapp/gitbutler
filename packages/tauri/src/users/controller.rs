use tauri::AppHandle;

use super::{storage, User};

pub struct Controller {
    storage: storage::Storage,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(app: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            storage: storage::Storage::try_from(app)?,
        })
    }
}

impl Controller {
    pub fn get_user(&self) -> Result<Option<User>, GetError> {
        self.storage.get().map_err(Into::into)
    }

    pub fn set_user(&self, user: &User) -> Result<(), SetError> {
        self.storage.set(user).map_err(Into::into)
    }

    pub fn delete_user(&self) -> Result<(), DeleteError> {
        self.storage.delete().map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error(transparent)]
    Storage(#[from] storage::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error(transparent)]
    Storage(#[from] storage::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error(transparent)]
    Storage(#[from] storage::Error),
}
