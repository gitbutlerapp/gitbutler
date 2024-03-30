use anyhow::Context;

use super::{storage::Storage, PrivateKey};

#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl Controller {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self::new(Storage::from_path(path))
    }

    pub fn get_or_create(&self) -> Result<PrivateKey, GetOrCreateError> {
        if let Some(key) = self.storage.get().context("failed to get key")? {
            Ok(key)
        } else {
            let key = PrivateKey::generate();
            self.storage.create(&key).context("failed to save key")?;
            Ok(key)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetOrCreateError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
