use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

#[derive(Debug, Default, Clone)]
pub struct Storage {
    local_data_dir: Arc<RwLock<PathBuf>>,
}

impl Storage {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        Storage {
            local_data_dir: Arc::new(RwLock::new(path.as_ref().to_path_buf())),
        }
    }

    pub fn local_data_dir(&self) -> PathBuf {
        let local_data_dir = self.local_data_dir.read().unwrap();
        local_data_dir.clone()
    }

    pub fn read<P: AsRef<Path>>(&self, path: P) -> Result<Option<String>> {
        let local_data_dir = self.local_data_dir.read().unwrap();
        let file_path = local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(file_path.clone())
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;
        Ok(Some(contents))
    }

    pub fn write(&self, path: &str, content: &str) -> Result<()> {
        let local_data_dir = self.local_data_dir.write().unwrap();
        let file_path = local_data_dir.join(path);
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
        let local_data_dir = self.local_data_dir.write().unwrap();
        let file_path = local_data_dir.join(path);
        if !file_path.exists() {
            return Ok(());
        }
        fs::remove_file(file_path.clone())
            .with_context(|| format!("Failed to delete file: {:?}", file_path))?;
        Ok(())
    }
}
