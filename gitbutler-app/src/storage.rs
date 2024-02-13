use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

#[cfg(target_family = "unix")]
use std::os::unix::prelude::*;

use tauri::{AppHandle, Manager};

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
        if let Some(storage) = value.try_state::<Storage>() {
            Ok(storage.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let storage = Storage::new(app_data_dir);
            value.manage(storage.clone());
            Ok(storage)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

impl TryFrom<&PathBuf> for Storage {
    type Error = anyhow::Error;

    fn try_from(value: &PathBuf) -> Result<Self, Self::Error> {
        Ok(Storage::new(value))
    }
}

impl Storage {
    fn new<P: AsRef<Path>>(local_data_dir: P) -> Storage {
        Storage {
            local_data_dir: Arc::new(RwLock::new(local_data_dir.as_ref().to_path_buf())),
        }
    }

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
