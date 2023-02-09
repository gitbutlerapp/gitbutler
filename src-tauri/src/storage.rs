use std::{fs, path::PathBuf};
use tauri::PathResolver;

#[derive(Default)]
pub struct Storage {
    local_data_dir: PathBuf,
}

#[derive(Debug)]
pub enum ErrorCause {
    IOError(std::io::Error),
}

impl From<std::io::Error> for ErrorCause {
    fn from(err: std::io::Error) -> Self {
        ErrorCause::IOError(err)
    }
}

#[derive(Debug)]
pub struct Error {
    pub cause: ErrorCause,
    pub message: String,
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
        let contents = fs::read_to_string(file_path).map_err(|e| Error {
            cause: e.into(),
            message: "Could not read file".to_string(),
        })?;
        Ok(Some(contents))
    }

    pub fn write(&self, path: &str, content: &str) -> Result<(), Error> {
        let file_path = self.local_data_dir.join(path);
        let dir = file_path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(|e| Error {
                cause: e.into(),
                message: "Could not create directory".to_string(),
            })?;
        }
        fs::write(file_path, content).map_err(|e| Error {
            cause: e.into(),
            message: "Could not write file".to_string(),
        })?;
        Ok(())
    }
}
