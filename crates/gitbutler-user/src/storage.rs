use anyhow::Result;
use std::path::PathBuf;

use gitbutler_storage::storage as core_storage;

use crate::User;

const USER_FILE: &str = "user.json";

#[derive(Debug, Clone)]
pub struct Storage {
    inner: core_storage::Storage,
}

impl Storage {
    pub fn new(storage: core_storage::Storage) -> Storage {
        Storage { inner: storage }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Storage {
        Storage::new(core_storage::Storage::new(path))
    }

    pub fn get(&self) -> Result<Option<User>> {
        match self.inner.read(USER_FILE)? {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    pub fn set(&self, user: &User) -> Result<()> {
        let data = serde_json::to_string(user)?;
        Ok(self.inner.write(USER_FILE, &data)?)
    }

    pub fn delete(&self) -> Result<()> {
        Ok(self.inner.delete(USER_FILE)?)
    }
}
