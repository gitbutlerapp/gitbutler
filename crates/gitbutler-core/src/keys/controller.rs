use anyhow::Context;
use std::path::PathBuf;

use super::{storage::Storage, PrivateKey};

#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl Controller {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::new(Storage::from_path(path))
    }

    pub fn get_or_create(&self) -> anyhow::Result<PrivateKey> {
        if let Some(key) = self.storage.get().context("failed to get key")? {
            Ok(key)
        } else {
            let key = PrivateKey::generate();
            self.storage.create(&key).context("failed to save key")?;
            Ok(key)
        }
    }
}
