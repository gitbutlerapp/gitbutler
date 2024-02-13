use anyhow::Context;
use tauri::{AppHandle, Manager};

use super::{storage::Storage, User};

#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(controller) = value.try_state::<Controller>() {
            Ok(controller.inner().clone())
        } else {
            let controller = Controller::new(Storage::try_from(value)?);
            value.manage(controller.clone());
            Ok(controller)
        }
    }
}

impl Controller {
    fn new(storage: Storage) -> Controller {
        Controller { storage }
    }

    pub fn get_user(&self) -> Result<Option<User>, GetError> {
        self.storage
            .get()
            .context("failed to get user")
            .map_err(Into::into)
    }

    pub fn set_user(&self, user: &User) -> Result<(), SetError> {
        self.storage
            .set(user)
            .context("failed to set user")
            .map_err(Into::into)
    }

    pub fn delete_user(&self) -> Result<(), DeleteError> {
        self.storage
            .delete()
            .context("failed to delete user")
            .map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
