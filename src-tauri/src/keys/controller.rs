use std::path;

use tauri::{AppHandle, Manager};

use crate::storage;

use super::PrivateKey;

#[derive(Clone)]
pub struct Controller {
    storage: storage::Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("data directory not found")]
    DirNotFound,
    #[error("IO error: {0}")]
    Storage(#[from] storage::Error),
    #[error("SSH key error: {0}")]
    SSHKey(#[from] ssh_key::Error),
}

impl From<storage::Storage> for Controller {
    fn from(storage: storage::Storage) -> Self {
        Self { storage }
    }
}

impl From<&AppHandle> for Controller {
    fn from(handle: &AppHandle) -> Self {
        Self {
            storage: handle.state::<storage::Storage>().inner().clone(),
        }
    }
}

impl From<&path::PathBuf> for Controller {
    fn from(path: &path::PathBuf) -> Self {
        Self::from(storage::Storage::from(path))
    }
}

impl Controller {
    pub fn get_or_create(&self) -> Result<PrivateKey, Error> {
        match self.get_private_key() {
            Ok(Some(key)) => Ok(key),
            Ok(None) => self.create(),
            Err(e) => Err(e),
        }
    }

    fn get_private_key(&self) -> Result<Option<PrivateKey>, Error> {
        self.storage
            .read("keys/ed25519")
            .map_err(Error::Storage)
            .and_then(|s| s.map(|s| s.parse().map_err(Error::SSHKey)).transpose())
    }

    fn create(&self) -> Result<PrivateKey, Error> {
        let key = PrivateKey::generate();
        self.storage
            .write("keys/ed25519", &key.to_string())
            .map_err(Error::Storage)?;
        self.storage
            .write("keys/ed25519.pub", &key.public_key().to_string())
            .map_err(Error::Storage)?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create() {
        let dir = tempfile::tempdir().unwrap();
        let controller = Controller::from(&dir.path().to_path_buf());
        let once = controller.get_or_create().unwrap();
        let twice = controller.get_or_create().unwrap();
        assert_eq!(once, twice);
    }
}
