use std::{
    fs,
    path::{self, Path, PathBuf},
    sync::{Arc, RwLock},
};

use anyhow::Result;
use tauri::AppHandle;

#[derive(Debug, Default, Clone)]
pub struct Storage {
    local_data_dir: Arc<RwLock<PathBuf>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to get local data dir")]
    LocalDataDir,
    #[error(transparent)]
    IO(#[from] std::io::Error),
}

impl TryFrom<&AppHandle> for Storage {
    type Error = Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        value
            .path_resolver()
            .app_local_data_dir()
            .map(|path| Self::from(&path))
            .ok_or(Error::LocalDataDir)
    }
}

impl From<&path::PathBuf> for Storage {
    fn from(value: &path::PathBuf) -> Self {
        Storage {
            local_data_dir: Arc::new(RwLock::new(value.to_path_buf())),
        }
    }
}

impl Storage {
    pub fn local_data_dir(&self) -> PathBuf {
        let local_data_dir = self.local_data_dir.read().unwrap();
        local_data_dir.clone()
    }

    pub fn read<P: AsRef<Path>>(&self, path: P) -> Result<Option<String>, Error> {
        let local_data_dir = self.local_data_dir.read().unwrap();
        let file_path = local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(file_path.clone()).map_err(Error::IO)?;
        Ok(Some(contents))
    }

    pub fn write(&self, path: &str, content: &str) -> Result<(), Error> {
        let local_data_dir = self.local_data_dir.write().unwrap();
        let file_path = local_data_dir.join(path);
        let dir = file_path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(Error::IO)?;
        }
        fs::write(file_path.clone(), content).map_err(Error::IO)?;
        Ok(())
    }

    pub fn delete(&self, path: &str) -> Result<(), Error> {
        let local_data_dir = self.local_data_dir.write().unwrap();
        let file_path = local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(());
        }
        fs::remove_file(file_path.clone()).map_err(Error::IO)?;
        Ok(())
    }
}
