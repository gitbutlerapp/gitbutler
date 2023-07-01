use std::io::Write;

use anyhow::{Context, Result};

pub trait Writer {
    fn write(&self, path: &str, contents: &[u8]) -> Result<()>;
    fn append_string(&self, path: &str, contents: &str) -> Result<()>;
    fn remove(&self, path: &str) -> Result<()>;

    fn write_usize(&self, path: &str, contents: &usize) -> Result<()> {
        self.write_string(path, &contents.to_string())
    }
    fn write_u128(&self, path: &str, contents: &u128) -> Result<()> {
        self.write_string(path, &contents.to_string())
    }
    fn write_bool(&self, path: &str, contents: &bool) -> Result<()> {
        self.write_string(path, &contents.to_string())
    }
    fn write_string(&self, path: &str, contents: &str) -> Result<()> {
        self.write(path, contents.as_bytes())
    }
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

    fn remove(&self, path: &str) -> Result<()> {
        let file_path = self.root.join(path);
        if file_path.is_dir() {
            match std::fs::remove_dir_all(file_path) {
                Ok(_) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.into()),
            }
        } else {
            match std::fs::remove_file(file_path) {
                Ok(_) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.into()),
            }
        }
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

    #[test]
    fn test_remove() {
        let root = tempfile::tempdir().unwrap();
        let writer = DirWriter::open(root.path().to_path_buf());
        writer.remove("foo/bar").unwrap();
        assert!(!root.path().join("foo/bar").exists());
        writer.write("foo/bar", b"baz").unwrap();
        writer.remove("foo/bar").unwrap();
        assert!(!root.path().join("foo/bar").exists());
        writer.write("parent/child", b"baz").unwrap();
        writer.remove("parent").unwrap();
        assert!(!root.path().join("parent").exists());
    }
}
