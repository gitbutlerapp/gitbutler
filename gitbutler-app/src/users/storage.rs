use std::path::Path;

use anyhow::Result;

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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Storage {
        Storage { storage }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Storage {
        Storage::new(storage::Storage::new(path))
    }

    pub fn get(&self) -> Result<Option<user::User>, Error> {
        match self.storage.read(USER_FILE)? {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    pub fn set(&self, user: &user::User) -> Result<(), Error> {
        let data = serde_json::to_string(user)?;
        self.storage.write(USER_FILE, &data)?;
        Ok(())
    }

    pub fn delete(&self) -> Result<(), Error> {
        self.storage.delete(USER_FILE)?;
        Ok(())
    }
}
