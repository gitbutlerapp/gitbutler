use std::{fs, path};

use tauri::AppHandle;

use super::PrivateKey;

#[derive(Clone)]
pub struct Controller {
    dir: path::PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("data directory not found")]
    DirNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SSH key error: {0}")]
    SSHKey(#[from] ssh_key::Error),
}

impl TryFrom<AppHandle> for Controller {
    type Error = Error;

    fn try_from(handle: AppHandle) -> Result<Self, Self::Error> {
        handle
            .path_resolver()
            .app_local_data_dir()
            .map(|p| p.join("keys"))
            .map(Self::new)
            .unwrap_or_else(|| Err(Error::DirNotFound))
    }
}

impl Controller {
    pub fn new<P: AsRef<path::Path>>(path: P) -> Result<Self, Error> {
        let dir = path.as_ref().to_path_buf();
        fs::create_dir_all(&dir).map_err(Error::Io)?;
        Ok(Self { dir })
    }

    pub fn get_or_create(&self) -> Result<PrivateKey, Error> {
        match self.get_private_key() {
            Ok(key) => Ok(key),
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => self.create(),
            Err(e) => Err(e),
        }
    }

    fn get_private_key(&self) -> Result<PrivateKey, Error> {
        let path = self.private_key_path();
        let key = fs::read_to_string(path)
            .map_err(Error::Io)
            .and_then(|s| s.parse().map_err(Error::SSHKey))?;
        Ok(key)
    }

    fn private_key_path(&self) -> path::PathBuf {
        self.dir.join("ed25519")
    }

    fn create(&self) -> Result<PrivateKey, Error> {
        let key = PrivateKey::generate();
        let key_path = self.private_key_path();
        fs::write(&key_path, key.to_string()).map_err(Error::Io)?;
        fs::write(key_path.with_extension("pub"), key.public_key().to_string())
            .map_err(Error::Io)?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create() {
        let dir = tempfile::tempdir().unwrap();
        let controller = Controller::new(dir.path()).unwrap();
        let once = controller.get_or_create().unwrap();
        let twice = controller.get_or_create().unwrap();
        assert_eq!(once, twice);
    }
}
