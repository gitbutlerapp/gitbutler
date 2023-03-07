use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::PathResolver;

#[derive(Debug, Default, Clone)]
pub struct Storage {
    local_data_dir: PathBuf,
}

impl Storage {
    pub fn from_path(path: PathBuf) -> Self {
        Storage {
            local_data_dir: path,
        }
    }

    pub fn from_path_resolver(resolver: &PathResolver) -> Self {
        Self {
            local_data_dir: resolver.app_local_data_dir().unwrap(),
        }
    }

    pub fn read<P: AsRef<Path>>(&self, path: P) -> Result<Option<String>> {
        let file_path = self.local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(file_path.clone())
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;
        Ok(Some(contents))
    }

    pub fn write(&self, path: &str, content: &str) -> Result<()> {
        let file_path = self.local_data_dir.join(path);
        let dir = file_path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create directory: {:?}", dir))?;
        }
        fs::write(file_path.clone(), content)
            .with_context(|| format!("Failed to write file: {:?}", file_path))?;
        Ok(())
    }

    pub fn delete(&self, path: &str) -> Result<()> {
        let file_path = self.local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(());
        }
        fs::remove_file(file_path.clone())
            .with_context(|| format!("Failed to delete file: {:?}", file_path))?;
        Ok(())
    }
}
