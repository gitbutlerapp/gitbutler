use std::path;

use tauri::{AppHandle, Manager};

use crate::storage;

use super::PrivateKey;

#[derive(Clone)]
pub struct Storage {
    storage: storage::Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Storage(#[from] storage::Error),
    #[error("SSH key error: {0}")]
    SSHKey(#[from] ssh_key::Error),
}

impl From<&storage::Storage> for Storage {
    fn from(storage: &storage::Storage) -> Self {
        Self {
            storage: storage.clone(),
        }
    }
}

impl From<&AppHandle> for Storage {
    fn from(handle: &AppHandle) -> Self {
        Self::from(handle.state::<storage::Storage>().inner())
    }
}

impl From<&path::PathBuf> for Storage {
    fn from(path: &path::PathBuf) -> Self {
        Self::from(&storage::Storage::from(path))
    }
}

impl Storage {
    pub fn get(&self) -> Result<Option<PrivateKey>, Error> {
        self.storage
            .read("keys/ed25519")
            .map_err(Error::Storage)
            .and_then(|s| s.map(|s| s.parse().map_err(Error::SSHKey)).transpose())
    }

    pub fn create(&self, key: &PrivateKey) -> Result<(), Error> {
        self.storage
            .write("keys/ed25519", &key.to_string())
            .map_err(Error::Storage)?;
        self.storage
            .write("keys/ed25519.pub", &key.public_key().to_string())
            .map_err(Error::Storage)?;
        Ok(())
    }
}
