use super::PrivateKey;
use crate::storage;
use anyhow::Result;
use std::path::PathBuf;

// TODO(ST): get rid of this type, it's more trouble than it's worth.
#[derive(Clone)]
pub struct Storage {
    inner: storage::Storage,
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Storage {
        Storage { inner: storage }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Storage {
        Storage::new(storage::Storage::new(path))
    }

    pub fn get(&self) -> Result<Option<PrivateKey>> {
        let key = self.inner.read("keys/ed25519")?;
        key.map(|s| s.parse().map_err(Into::into)).transpose()
    }

    // TODO(ST): see if Key should rather deal with bytes instead for this kind of serialization.
    pub fn create(&self, key: &PrivateKey) -> Result<()> {
        self.inner.write("keys/ed25519", &key.to_string())?;
        self.inner
            .write("keys/ed25519.pub", &key.public_key().to_string())?;
        Ok(())
    }
}
