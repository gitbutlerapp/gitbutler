use anyhow::Context;
use tauri::AppHandle;

use crate::{paths::DataDir, storage};

use super::{storage::Storage, PrivateKey};

pub struct Controller {
    storage: Storage,
}

impl From<&DataDir> for Controller {
    fn from(value: &DataDir) -> Self {
        Self {
            storage: Storage::from(value),
        }
    }
}

impl From<&storage::Storage> for Controller {
    fn from(value: &storage::Storage) -> Self {
        Self {
            storage: Storage::from(value),
        }
    }
}

impl From<&AppHandle> for Controller {
    fn from(value: &AppHandle) -> Self {
        Self {
            storage: Storage::from(value),
        }
    }
}

impl Controller {
    pub fn get_or_create(&self) -> Result<PrivateKey, GetOrCreateError> {
        match self.storage.get().context("failed to get key")? {
            Some(key) => Ok(key),
            None => {
                let key = PrivateKey::generate();
                self.storage.create(&key).context("failed to save key")?;
                Ok(key)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetOrCreateError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::prelude::PermissionsExt};

    use crate::test_utils::Suite;

    use super::*;

    #[test]
    fn test_get_or_create() {
        let suite = Suite::default();
        let controller = Controller::from(&suite.local_app_data);

        let once = controller.get_or_create().unwrap();
        let twice = controller.get_or_create().unwrap();
        assert_eq!(once, twice);

        // check permissions of the private key
        let permissions = fs::metadata(suite.local_app_data.to_path_buf().join("keys/ed25519"))
            .unwrap()
            .permissions();
        let perms = format!("{:o}", permissions.mode());
        assert_eq!(perms, "100600");
    }
}
