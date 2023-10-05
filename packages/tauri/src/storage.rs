use std::{
    fs,
    os::unix::prelude::PermissionsExt,
    path::{self, Path, PathBuf},
    sync::{Arc, RwLock},
};

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

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let app_local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .ok_or(Error::LocalDataDir)?;
        fs::create_dir_all(&app_local_data_dir).map_err(Error::IO)?;
        Ok(Self::from(&app_local_data_dir))
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

        // Set the permissions to be user-only.
        let metadata = fs::metadata(file_path.clone())?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600); // User read/write
        fs::set_permissions(file_path.clone(), permissions)?;

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
