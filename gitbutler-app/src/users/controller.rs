use anyhow::Context;

use super::{storage::Storage, User};

#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl TryFrom<&std::path::PathBuf> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &std::path::PathBuf) -> Result<Self, Self::Error> {
        Ok(Controller::new(Storage::try_from(value)?))
    }
}

impl Controller {
    pub fn new(storage: Storage) -> Controller {
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
