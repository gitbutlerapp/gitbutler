use std::path::Path;

use anyhow::Result;

use crate::lock;

pub struct DirWriter(lock::Dir);

impl DirWriter {
    pub fn open<P: AsRef<Path>>(root: P) -> Result<Self, std::io::Error> {
        lock::Dir::new(root).map(Self)
    }
}

impl DirWriter {
    fn write<P, C>(&self, path: P, contents: C) -> Result<(), std::io::Error>
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>,
    {
        self.batch(&[BatchTask::Write(path, contents)])
    }

    pub fn remove<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        self.0.batch(|root| {
            let path = root.join(path);
            if path.exists() {
                if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                }
            } else {
                Ok(())
            }
        })?
    }

    pub fn batch<P, C>(&self, values: &[BatchTask<P, C>]) -> Result<(), std::io::Error>
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>,
    {
        self.0.batch(|root| {
            for value in values {
                match value {
                    BatchTask::Write(path, contents) => {
                        let path = root.join(path);
                        if let Some(dir_path) = path.parent() {
                            if !dir_path.exists() {
                                std::fs::create_dir_all(dir_path)?;
                            }
                        };
                        std::fs::write(path, contents)?;
                    }
                    BatchTask::Remove(path) => {
                        let path = root.join(path);
                        if path.exists() {
                            if path.is_dir() {
                                std::fs::remove_dir_all(path)?;
                            } else {
                                std::fs::remove_file(path)?;
                            }
                        }
                    }
                }
            }
            Ok(())
        })?
    }

    pub fn write_string<P: AsRef<Path>>(
        &self,
        path: P,
        contents: &str,
    ) -> Result<(), std::io::Error> {
        self.write(path, contents)
    }
}

pub enum BatchTask<P: AsRef<Path>, C: AsRef<[u8]>> {
    Write(P, C),
    Remove(P),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write() {
        let root = tempfile::tempdir().unwrap();
        let writer = DirWriter::open(root.path()).unwrap();
        writer.write("foo/bar", b"baz").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("foo/bar")).unwrap(),
            "baz"
        );
    }

    #[test]
    fn test_remove() {
        let root = tempfile::tempdir().unwrap();
        let writer = DirWriter::open(root.path()).unwrap();
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
