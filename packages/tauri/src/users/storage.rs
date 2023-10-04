use std::path;

use anyhow::Result;
use tauri::{AppHandle, Manager};

use crate::{storage, users::user};

const USER_FILE: &str = "user.json";

#[derive(Debug, Clone)]
pub struct Storage {
    storage: storage::Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] storage::Error),
}

impl From<&storage::Storage> for Storage {
    fn from(storage: &storage::Storage) -> Self {
        Self {
            storage: storage.clone(),
        }
    }
}

impl From<&AppHandle> for Storage {
    fn from(value: &AppHandle) -> Self {
        Self::from(value.state::<storage::Storage>().inner())
    }
}

impl From<&path::PathBuf> for Storage {
    fn from(path: &path::PathBuf) -> Self {
        Self::from(&storage::Storage::from(path))
    }
}

impl Storage {
    pub fn get(&self) -> Result<Option<user::User>> {
        match self.storage.read(USER_FILE)? {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    pub fn set(&self, user: &user::User) -> Result<()> {
        let data = serde_json::to_string(user)?;
        self.storage.write(USER_FILE, &data)?;
        Ok(())
    }

    pub fn delete(&self) -> Result<()> {
        self.storage.delete(USER_FILE)?;
        Ok(())
    }
}
