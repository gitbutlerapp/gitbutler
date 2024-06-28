use anyhow::Context;
use std::path::PathBuf;

use super::{storage::Storage, User};

/// TODO(ST): useless intermediary - remove
#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl Controller {
    pub fn new(storage: Storage) -> Controller {
        Controller { storage }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Controller {
        Controller::new(Storage::from_path(path))
    }

    pub fn get_user(&self) -> anyhow::Result<Option<User>> {
        match self.storage.get().context("failed to get user") {
            Ok(user) => Ok(user),
            Err(err) => {
                self.storage.delete().ok();
                Err(err)
            }
        }
    }

    pub fn set_user(&self, user: &User) -> anyhow::Result<()> {
        self.storage.set(user).context("failed to set user")
    }

    pub fn delete_user(&self) -> anyhow::Result<()> {
        self.storage.delete().context("failed to delete user")
    }
}
