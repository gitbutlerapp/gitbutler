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

    pub fn read(&self, path: &str) -> Result<Option<String>, std::io::Error> {
        let file_path = self.local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(file_path)?;
        Ok(Some(contents))
    }

    pub fn write(&self, path: &str, content: &str) -> Result<(), std::io::Error> {
        let file_path = self.local_data_dir.join(path);
        let dir = file_path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        fs::write(file_path, content)?;
        Ok(())
    }
}
