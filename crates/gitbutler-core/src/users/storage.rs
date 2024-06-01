use anyhow::Result;
use std::path::PathBuf;

use crate::{storage, users::user};

const USER_FILE: &str = "user.json";

#[derive(Debug, Clone)]
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

    pub fn get(&self) -> Result<Option<user::User>> {
        match self.inner.read(USER_FILE)? {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    pub fn set(&self, user: &user::User) -> Result<()> {
        let data = serde_json::to_string(user)?;
        Ok(self.inner.write(USER_FILE, &data)?)
    }

    pub fn delete(&self) -> Result<()> {
        Ok(self.inner.delete(USER_FILE)?)
    }
}
