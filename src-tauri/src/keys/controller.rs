use std::{fs, path};

use ed25519_dalek::pkcs8::Error as PKSC8Error;
use tauri::AppHandle;

use super::key;

pub struct Controller {
    dir: path::PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("data directory not found")]
    DirNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse key{0}")]
    Pkcs8(String),
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
        let key = fs::read_to_string(&path).map_err(Error::Io).and_then(|s| {
            s.parse()
                .map_err(|e: PKSC8Error| Error::Pkcs8(e.to_string()))
        })?;
        Ok(PrivateKey {
            key,
            path: path.to_path_buf(),
        })
    }

    fn private_key_path(&self) -> path::PathBuf {
        self.dir.join("ed25519")
    }

    fn create(&self) -> Result<PrivateKey, Error> {
        let key = key::PrivateKey::generate();
        let key_path = self.private_key_path();
        fs::write(&key_path, key.to_string()).map_err(Error::Io)?;
        fs::write(key_path.with_extension("pub"), key.public_key().to_string())
            .map_err(Error::Io)?;
        Ok(PrivateKey {
            key,
            path: self.private_key_path().to_path_buf(),
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct PrivateKey {
    key: key::PrivateKey,
    path: path::PathBuf,
}

impl PrivateKey {
    pub fn public_key(&self) -> PublicKey {
        PublicKey {
            key: self.key.public_key(),
            path: self.path.with_extension("pub"),
        }
    }

    pub fn path(&self) -> &path::Path {
        &self.path
    }
}

pub struct PublicKey {
    key: key::PublicKey,
    path: path::PathBuf,
}

impl PublicKey {
    pub fn path(&self) -> &path::Path {
        &self.path
    }
}

impl serde::Serialize for PublicKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.key.serialize(serializer)
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
