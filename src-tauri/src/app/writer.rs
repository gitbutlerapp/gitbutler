use std::io::Write;

use anyhow::{Context, Result};

pub trait Writer {
    fn write(&self, path: &str, contents: &[u8]) -> Result<()>;
    fn write_string(&self, path: &str, contents: &str) -> Result<()> {
        self.write(path, contents.as_bytes())
    }
    fn append_string(&self, path: &str, contents: &str) -> Result<()>;
}

pub struct DirWriter {
    root: std::path::PathBuf,
}

impl<'writer> DirWriter {
    pub fn open(root: std::path::PathBuf) -> Self {
        Self { root }
    }
}

impl Writer for DirWriter {
    fn write(&self, path: &str, contents: &[u8]) -> Result<()> {
        let file_path = self.root.join(path);
        let dir_path = file_path.parent().unwrap();
        std::fs::create_dir_all(dir_path)
            .with_context(|| format!("failed to create dir: {}", dir_path.display()))?;
        std::fs::write(file_path, contents)
            .with_context(|| format!("failed to write file: {}", path))?;
        Ok(())
    }

    fn append_string(&self, path: &str, contents: &str) -> Result<()> {
        let file_path = self.root.join(path);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .with_context(|| format!("failed to open file: {}", path))?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}
