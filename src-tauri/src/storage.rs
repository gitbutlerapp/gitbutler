use std::{fs, path::PathBuf};
use tauri::PathResolver;

#[derive(Default)]
pub struct Storage {
    local_data_dir: PathBuf,
}

impl Storage {
    pub fn new(resolver: &PathResolver) -> Self {
        log::info!(
            "Local data dir: {:?}",
            resolver.app_local_data_dir().unwrap()
        );
        Self {
            local_data_dir: resolver.app_local_data_dir().unwrap(),
        }
    }

    pub fn read(&self, path: &str) -> Result<Option<String>, Error> {
        let file_path = self.local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(file_path)?;
        Ok(Some(contents))
    }

    pub fn write(&self, path: &str, content: &str) -> Result<(), Error> {
        let file_path = self.local_data_dir.join(path);
        let dir = file_path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        fs::write(file_path, content)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    IOError(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IOError(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IOError(err) => write!(f, "IO error: {}", err),
        }
    }
}
