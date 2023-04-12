use anyhow::{Context, Result};
use std::io::Write;

pub trait Writer {
    fn write_string(&self, path: &str, contents: &str) -> Result<()>;
    fn append_string(&self, path: &str, contents: &str) -> Result<()>;
}

pub struct WdWriter<'writer> {
    git_repository: &'writer git2::Repository,
}

pub fn get_working_directory_writer<'writer>(
    git_repository: &'writer git2::Repository,
) -> WdWriter {
    WdWriter { git_repository }
}

impl Writer for WdWriter<'_> {
    fn write_string(&self, path: &str, contents: &str) -> Result<()> {
        let file_path = self.git_repository.path().parent().unwrap().join(path);
        let dir_path = file_path.parent().unwrap();
        std::fs::create_dir_all(dir_path)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    fn append_string(&self, path: &str, contents: &str) -> Result<()> {
        let file_path = self.git_repository.path().parent().unwrap().join(path);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .with_context(|| format!("failed to open file: {}", path))?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}
