use std::path;

use anyhow::Context;
use tauri::AppHandle;

use crate::storage;

use super::{storage::Storage, User};

#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl From<&path::PathBuf> for Controller {
    fn from(path: &path::PathBuf) -> Self {
        Self {
            storage: Storage::from(path),
        }
    }
}

impl From<&storage::Storage> for Controller {
    fn from(storage: &storage::Storage) -> Self {
        Self {
            storage: Storage::from(storage),
        }
    }
}

impl From<&AppHandle> for Controller {
    fn from(app: &AppHandle) -> Self {
        Self {
            storage: Storage::from(app),
        }
    }
}

impl Controller {
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
