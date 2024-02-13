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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl TryFrom<&AppHandle> for Storage {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(storage) = value.try_state::<Storage>() {
            Ok(storage.inner().clone())
        } else {
            let storage = Storage::new(storage::Storage::try_from(value)?);
            value.manage(storage.clone());
            Ok(storage)
        }
    }
}

impl Storage {
    fn new(storage: storage::Storage) -> Storage {
        Storage { storage }
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
