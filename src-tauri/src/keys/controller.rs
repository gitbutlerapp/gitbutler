use std::{fs, path};

use tauri::AppHandle;

use super::PrivateKey;

pub struct Controller {
    dir: path::PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("data directory not found")]
    DirNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl TryFrom<AppHandle> for Controller {
    type Error = Error;

    fn try_from(handle: AppHandle) -> Result<Self, Self::Error> {
        handle
            .path_resolver()
            .app_local_data_dir()
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
        match self.get() {
            Ok(key) => Ok(key),
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => self.create(),
            Err(e) => Err(e),
        }
    }

    fn get(&self) -> Result<PrivateKey, Error> {
        let key = fs::read_to_string(self.dir.join("key.json"))
            .map_err(Error::Io)
            .and_then(|s| serde_json::from_str(&s).map_err(Error::Serde))?;
        Ok(key)
    }

    fn create(&self) -> Result<PrivateKey, Error> {
        let key = PrivateKey::generate();
        let serialized = serde_json::to_string(&key).map_err(Error::Serde)?;
        fs::write(self.dir.join("key.json"), serialized).map_err(Error::Io)?;
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
