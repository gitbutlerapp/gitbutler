use std::io::Write;

use anyhow::{Context, Result};

pub trait Writer {
    fn write_string(&self, path: &str, contents: &str) -> Result<()>;
    fn append_string(&self, path: &str, contents: &str) -> Result<()>;
}

pub struct DirWriter<'writer> {
    root: &'writer std::path::Path,
}

impl<'writer> DirWriter<'writer> {
    pub fn open(root: &'writer std::path::Path) -> Self {
        Self { root }
    }
}

impl Writer for DirWriter<'_> {
    fn write_string(&self, path: &str, contents: &str) -> Result<()> {
        let file_path = self.root.join(path);
        let dir_path = file_path.parent().unwrap();
        std::fs::create_dir_all(dir_path)?;
        std::fs::write(path, contents)?;
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
