use std::io::Write;

use anyhow::{Context, Result};

pub trait Writer {
    fn write(&self, path: &str, contents: &[u8]) -> Result<()>;
    fn write_u128(&self, path: &str, contents: &u128) -> Result<()> {
        self.write_string(path, &contents.to_string())
    }
    fn write_bool(&self, path: &str, contents: &bool) -> Result<()> {
        self.write_string(path, &contents.to_string())
    }
    fn write_string(&self, path: &str, contents: &str) -> Result<()> {
        self.write(path, contents.as_bytes())
    }
    fn append_string(&self, path: &str, contents: &str) -> Result<()>;
}

pub struct DirWriter {
    root: std::path::PathBuf,
}

impl DirWriter {
    pub fn open(root: std::path::PathBuf) -> Self {
        Self { root }
    }
}

impl Writer for DirWriter {
    fn write(&self, path: &str, contents: &[u8]) -> Result<()> {
        let file_path = self.root.join(path);
        let dir_path = file_path.parent().unwrap();
        std::fs::create_dir_all(dir_path).context("failed to create directory")?;
        std::fs::write(file_path, contents)?;
        Ok(())
    }

    fn append_string(&self, path: &str, contents: &str) -> Result<()> {
        let file_path = self.root.join(path);
        let dir_path = file_path.parent().unwrap();
        std::fs::create_dir_all(dir_path).context("failed to create directory")?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .with_context(|| format!("failed to open file: {}", path))?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write() {
        let root = tempfile::tempdir().unwrap();
        let writer = DirWriter::open(root.path().to_path_buf());
        writer.write("foo/bar", b"baz").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("foo/bar")).unwrap(),
            "baz"
        );
    }

    #[test]
    fn test_append_string() {
        let root = tempfile::tempdir().unwrap();
        let writer = DirWriter::open(root.path().to_path_buf());
        writer.append_string("foo/bar", "baz").unwrap();
        writer.append_string("foo/bar", "qux").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("foo/bar")).unwrap(),
            "bazqux"
        );
    }
}
