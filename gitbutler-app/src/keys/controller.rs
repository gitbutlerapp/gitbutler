use anyhow::Context;

use super::{storage::Storage, PrivateKey};

#[derive(Clone)]
pub struct Controller {
    storage: Storage,
}

impl TryFrom<&std::path::PathBuf> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &std::path::PathBuf) -> Result<Self, Self::Error> {
        Ok(Controller::new(Storage::try_from(value)?))
    }
}

impl Controller {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn get_or_create(&self) -> Result<PrivateKey, GetOrCreateError> {
        if let Some(key) = self.storage.get().context("failed to get key")? {
            Ok(key)
        } else {
            let key = PrivateKey::generate();
            self.storage.create(&key).context("failed to save key")?;
            Ok(key)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetOrCreateError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(target_family = "unix")]
    use std::os::unix::prelude::*;

    use crate::tests::Suite;

    use super::*;

    #[test]
    fn test_get_or_create() {
        let suite = Suite::default();
        let controller = Controller::try_from(&suite.local_app_data).unwrap();

        let once = controller.get_or_create().unwrap();
        let twice = controller.get_or_create().unwrap();
        assert_eq!(once, twice);

        // check permissions of the private key
        let permissions = fs::metadata(suite.local_app_data.join("keys/ed25519"))
            .unwrap()
            .permissions();
        let perms = format!("{:o}", permissions.mode());
        assert_eq!(perms, "100600");
    }
}
