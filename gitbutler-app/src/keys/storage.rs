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

impl Storage {
    pub fn new(storage: storage::Storage) -> Storage {
        Storage { storage }
    }

    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Storage {
        Storage::new(storage::Storage::new(path))
    }

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
