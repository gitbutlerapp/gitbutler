use std::path::PathBuf;

use crate::settings::ForgeSettings;
use anyhow::Result;

const FORGE_SETTINGS_FILE: &str = "forge_settings.json";

#[derive(Debug, Clone)]
pub(crate) struct Storage {
    inner: gitbutler_storage::Storage,
}

impl Storage {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Storage {
            inner: gitbutler_storage::Storage::new(path),
        }
    }

    pub fn read(&self) -> Result<ForgeSettings> {
        match self.inner.read(FORGE_SETTINGS_FILE)? {
            Some(settings) => {
                let settings: ForgeSettings = serde_json::from_str(&settings)?;
                Ok(settings)
            }
            None => Ok(Default::default()),
        }
    }

    pub fn save(&self, settings: &ForgeSettings) -> Result<()> {
        let data = serde_json::to_string_pretty(settings)?;
        self.inner.write(FORGE_SETTINGS_FILE, &data)?;
        Ok(())
    }
}
