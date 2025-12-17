use std::path::PathBuf;

use anyhow::Result;

use crate::User;

const USER_FILE: &str = "user.json";

#[derive(Debug, Clone)]
pub(crate) struct Storage {
    inner: but_fs::Storage,
}

impl Storage {
    pub fn from_path(path: impl Into<PathBuf>) -> Storage {
        Storage {
            inner: but_fs::Storage::new(path),
        }
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
