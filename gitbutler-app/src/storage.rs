use std::{
    fs,
    path::{self, Path, PathBuf},
    sync::{Arc, RwLock},
};

#[cfg(target_family = "unix")]
use std::os::unix::prelude::*;

use tauri::AppHandle;

#[derive(Debug, Default, Clone)]
pub struct Storage {
    local_data_dir: Arc<RwLock<PathBuf>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] std::io::Error),
}

impl TryFrom<&AppHandle> for Storage {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let path = value.path_resolver().app_data_dir();
        match path {
            Some(path) => Ok(Self::from(&path)),
            // None => Error::new("failed to get app data dir"),
            None => Err(anyhow::anyhow!("failed to get app data dir")),
            // None => Ok(Self::default()),
        }
    }
}

impl From<&path::PathBuf> for Storage {
    fn from(value: &path::PathBuf) -> Self {
        Storage {
            local_data_dir: Arc::new(RwLock::new(value.clone())),
        }
    }
}

impl Storage {
    pub fn read<P: AsRef<Path>>(&self, path: P) -> Result<Option<String>, Error> {
        let local_data_dir = self.local_data_dir.read().unwrap();
        let file_path = local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&file_path).map_err(Error::IO)?;
        Ok(Some(contents))
    }

    pub fn write<P: AsRef<Path>>(&self, path: P, content: &str) -> Result<(), Error> {
        let local_data_dir = self.local_data_dir.write().unwrap();
        let file_path = local_data_dir.join(path);
        let dir = file_path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(Error::IO)?;
        }
        fs::write(&file_path, content).map_err(Error::IO)?;

        // Set the permissions to be user-only. We can't actually
        // do this on Windows, so we ignore that platform.
        #[cfg(target_family = "unix")]
        {
            let metadata = fs::metadata(file_path.clone())?;
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600); // User read/write
            fs::set_permissions(file_path.clone(), permissions)?;
        }

        Ok(())
    }

    pub fn delete<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let local_data_dir = self.local_data_dir.write().unwrap();
        let file_path = local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(());
        }
        if file_path.is_dir() {
            fs::remove_dir_all(file_path.clone()).map_err(Error::IO)?;
        } else {
            fs::remove_file(file_path.clone()).map_err(Error::IO)?;
        }
        Ok(())
    }
}
