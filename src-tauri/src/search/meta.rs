use std::{fs, path};

use anyhow::Result;

use crate::storage;

use super::index;

#[derive(Clone)]
pub struct Storage {
    storage: storage::Storage,
}

impl Storage {
    pub fn new(base_path: path::PathBuf) -> Self {
        Self {
            storage: storage::Storage::from_path(base_path),
        }
    }

    pub fn delete_all(&self) -> Result<()> {
        let filepath = self
            .storage
            .local_data_dir()
            .join("indexes")
            .join(format!("v{}", index::VERSION))
            .join("meta");
        fs::remove_dir_all(filepath)?;
        Ok(())
    }

    pub fn get(&self, project_id: &str, session_hash: &str) -> Result<Option<u64>> {
        let filepath = path::Path::new("indexes")
            .join(format!("v{}", index::VERSION))
            .join("meta")
            .join(project_id)
            .join(session_hash);
        let meta = match self.storage.read(&filepath.to_str().unwrap())? {
            None => None,
            Some(meta) => meta.parse::<u64>().ok(),
        };
        Ok(meta)
    }

    pub fn set(&self, project_id: &str, session_hash: &str, version: u64) -> Result<()> {
        let filepath = path::Path::new("indexes")
            .join(format!("v{}", index::VERSION))
            .join("meta")
            .join(project_id)
            .join(session_hash);
        self.storage
            .write(&filepath.to_str().unwrap(), &version.to_string())?;
        Ok(())
    }
}
