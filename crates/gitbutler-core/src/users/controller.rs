use anyhow::Context;

use super::{storage::Storage, User};

#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl Controller {
    pub fn new(storage: Storage) -> Controller {
        Controller { storage }
    }

    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Controller {
        Controller::new(Storage::from_path(path))
    }

    pub fn get_user(&self) -> anyhow::Result<Option<User>> {
        self.storage.get().context("failed to get user")
    }

    pub fn set_user(&self, user: &User) -> anyhow::Result<()> {
        self.storage.set(user).context("failed to set user")
    }

    pub fn delete_user(&self) -> anyhow::Result<()> {
        self.storage.delete().context("failed to delete user")
    }
}
