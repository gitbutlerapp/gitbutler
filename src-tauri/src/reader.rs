use std::{path, str};

use anyhow::{Context, Result};

use crate::fs;

#[derive(Debug, PartialEq)]
pub enum Content {
    UTF8(String),
    Binary(Vec<u8>),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file not found")]
    NotFound,
    #[error("io error: {0}")]
    IOError(std::io::Error),
}

pub trait Reader {
    fn read(&self, file_path: &str) -> Result<Content, Error>;
    fn list_files(&self, dir_path: &str) -> Result<Vec<String>>;
    fn exists(&self, file_path: &str) -> bool;
    fn size(&self, file_path: &str) -> Result<usize>;

    fn read_string(&self, file_path: &str) -> Result<String, Error> {
        match self.read(file_path)? {
            Content::UTF8(s) => Ok(s),
            Content::Binary(_) => Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "file is not utf8",
            ))),
        }
    }

    fn read_u128(&self, file_path: &str) -> Result<u128, Error> {
        let s = self.read_string(file_path)?;
        s.parse::<u128>().map_err(|_| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "file is not u128",
            ))
        })
    }

    fn read_bool(&self, file_path: &str) -> Result<bool, Error> {
        let s = self.read_string(file_path)?;
        s.parse::<bool>().map_err(|_| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "file is not bool",
            ))
        })
    }
}

pub struct DirReader {
    root: std::path::PathBuf,
}

impl DirReader {
    pub fn open(root: std::path::PathBuf) -> Self {
        Self { root }
    }
}

impl Reader for DirReader {
    fn size(&self, file_path: &str) -> Result<usize> {
        let path = self.root.join(file_path);
        if !path.exists() {
            return Ok(0);
        }
        let metadata = std::fs::metadata(path)?;
        Ok(metadata.len().try_into()?)
    }

    fn read(&self, path: &str) -> Result<Content, Error> {
        let path = self.root.join(path);
        if !path.exists() {
            return Err(Error::NotFound);
        }
        let content = std::fs::read(path).map_err(Error::IOError)?;
        match String::from_utf8_lossy(&content).into_owned() {
            s if s.as_bytes().eq(&content) => Ok(Content::UTF8(s)),
            _ => Ok(Content::Binary(content)),
        }
    }

    fn list_files(&self, dir_path: &str) -> Result<Vec<String>> {
        let files: Vec<String> = fs::list_files(self.root.join(dir_path))?
            .iter()
            .map(|f| f.to_str().unwrap().to_string())
            .filter(|f| !f.starts_with(".git"))
            .collect();
        Ok(files)
    }

    fn exists(&self, file_path: &str) -> bool {
        std::path::Path::new(self.root.join(file_path).as_path()).exists()
    }
}

pub struct CommitReader<'reader> {
    repository: &'reader git2::Repository,
    commit_oid: git2::Oid,
    tree: git2::Tree<'reader>,
}

impl<'reader> CommitReader<'reader> {
    pub fn from_commit(
        repository: &'reader git2::Repository,
        commit: git2::Commit<'reader>,
    ) -> Result<CommitReader<'reader>> {
        let tree = commit
            .tree()
            .with_context(|| format!("{}: tree not found", commit.id()))?;
        Ok(CommitReader {
            repository,
            tree,
            commit_oid: commit.id(),
        })
    }

    pub fn get_commit_oid(&self) -> git2::Oid {
        self.commit_oid
    }
}

impl Reader for CommitReader<'_> {
    fn size(&self, file_path: &str) -> Result<usize> {
        let entry = match self
            .tree
            .get_path(std::path::Path::new(file_path))
            .with_context(|| format!("{}: tree entry not found", file_path))
        {
            Ok(entry) => entry,
            Err(_) => return Ok(0),
        };
        let blob = match self.repository.find_blob(entry.id()) {
            Ok(blob) => blob,
            Err(_) => return Ok(0),
        };
        Ok(blob.size())
    }

    fn read(&self, path: &str) -> Result<Content, Error> {
        let entry = match self
            .tree
            .get_path(std::path::Path::new(path))
            .with_context(|| format!("{}: tree entry not found", path))
        {
            Ok(entry) => entry,
            Err(_) => return Err(Error::NotFound),
        };
        let blob = match self.repository.find_blob(entry.id()) {
            Ok(blob) => blob,
            Err(_) => return Err(Error::NotFound),
        };
        let content = blob.content();
        match String::from_utf8_lossy(content).into_owned() {
            s if s.as_bytes().eq(content) => Ok(Content::UTF8(s)),
            _ => Ok(Content::Binary(content.to_vec())),
        }
    }

    fn list_files(&self, dir_path: &str) -> Result<Vec<String>> {
        let mut files: Vec<String> = Vec::new();
        let dir_path = std::path::Path::new(dir_path);
        self.tree
            .walk(git2::TreeWalkMode::PreOrder, |root, entry| {
                if entry.kind() == Some(git2::ObjectType::Tree) {
                    return git2::TreeWalkResult::Ok;
                }

                if entry.name().is_none() {
                    return git2::TreeWalkResult::Ok;
                }
                let entry_path = std::path::Path::new(root).join(entry.name().unwrap());

                if !entry_path.starts_with(dir_path) {
                    return git2::TreeWalkResult::Ok;
                }

                files.push(
                    entry_path
                        .strip_prefix(dir_path)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                );

                git2::TreeWalkResult::Ok
            })
            .with_context(|| format!("{}: tree walk failed", dir_path.display()))?;

        Ok(files)
    }

    fn exists(&self, file_path: &str) -> bool {
        self.tree.get_path(std::path::Path::new(file_path)).is_ok()
    }
}

pub struct SubReader<'reader> {
    reader: &'reader dyn Reader,
    prefix: path::PathBuf,
}

impl<'reader> SubReader<'reader> {
    pub fn new<P: AsRef<path::Path>>(reader: &'reader dyn Reader, prefix: P) -> SubReader<'reader> {
        SubReader {
            reader,
            prefix: prefix.as_ref().to_path_buf(),
        }
    }
}

impl Reader for SubReader<'_> {
    fn size(&self, file_path: &str) -> Result<usize> {
        self.reader
            .size(self.prefix.join(file_path).to_str().unwrap())
    }

    fn read(&self, path: &str) -> Result<Content, Error> {
        self.reader.read(self.prefix.join(path).to_str().unwrap())
    }

    fn list_files(&self, dir_path: &str) -> Result<Vec<String>> {
        self.reader
            .list_files(self.prefix.join(dir_path).to_str().unwrap())
    }

    fn exists(&self, file_path: &str) -> bool {
        self.reader
            .exists(self.prefix.join(file_path).to_str().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    fn commit(repository: &git2::Repository) -> Result<git2::Oid> {
        let mut index = repository.index()?;
        index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let oid = index.write_tree()?;
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        let commit_oid = repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "some commit",
            &repository.find_tree(oid)?,
            &[&repository.find_commit(repository.refname_to_id("HEAD")?)?],
        )?;
        Ok(commit_oid)
    }

    fn test_repository() -> Result<git2::Repository> {
        let path = tempdir()?.path().to_str().unwrap().to_string();
        let repository = git2::Repository::init(path)?;
        let mut index = repository.index()?;
        let oid = index.write_tree()?;
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &repository.find_tree(oid)?,
            &[],
        )?;
        Ok(repository)
    }

    #[test]
    fn test_directory_reader_read_file() -> Result<()> {
        let dir = tempdir()?;

        let file_path = "test.txt";
        std::fs::write(dir.path().join(file_path), "test")?;

        let reader = DirReader::open(dir.path().to_path_buf());
        assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_read_file() -> Result<()> {
        let repository = test_repository()?;

        let file_path = "test.txt";
        std::fs::write(repository.path().parent().unwrap().join(file_path), "test")?;

        let oid = commit(&repository)?;

        std::fs::write(repository.path().parent().unwrap().join(file_path), "test2")?;

        let reader = CommitReader::from_commit(&repository, repository.find_commit(oid)?)?;
        assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

        Ok(())
    }

    #[test]
    fn test_reader_list_files_should_return_relative() -> Result<()> {
        let dir = tempdir()?;

        std::fs::write(dir.path().join("test1.txt"), "test")?;
        std::fs::create_dir(dir.path().join("dir"))?;
        std::fs::write(dir.path().join("dir").join("test.txt"), "test")?;

        let reader = DirReader::open(dir.path().to_path_buf());
        let files = reader.list_files("dir")?;
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"test.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_reader_list_files() -> Result<()> {
        let dir = tempdir()?;

        std::fs::write(dir.path().join("test.txt"), "test")?;
        std::fs::create_dir(dir.path().join("dir"))?;
        std::fs::write(dir.path().join("dir").join("test.txt"), "test")?;

        let reader = DirReader::open(dir.path().to_path_buf());
        let files = reader.list_files("")?;
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"test.txt".to_string()));
        assert!(files.contains(&"dir/test.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_list_files_should_return_relative() -> Result<()> {
        let repository = test_repository()?;

        std::fs::write(
            repository.path().parent().unwrap().join("test1.txt"),
            "test",
        )?;
        std::fs::create_dir(repository.path().parent().unwrap().join("dir"))?;
        std::fs::write(
            repository
                .path()
                .parent()
                .unwrap()
                .join("dir")
                .join("test.txt"),
            "test",
        )?;

        let oid = commit(&repository)?;

        std::fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

        let reader = CommitReader::from_commit(&repository, repository.find_commit(oid)?)?;
        let files = reader.list_files("dir")?;
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"test.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_list_files() -> Result<()> {
        let repository = test_repository()?;

        std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;
        std::fs::create_dir(repository.path().parent().unwrap().join("dir"))?;
        std::fs::write(
            repository
                .path()
                .parent()
                .unwrap()
                .join("dir")
                .join("test.txt"),
            "test",
        )?;

        let oid = commit(&repository)?;

        std::fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

        let reader = CommitReader::from_commit(&repository, repository.find_commit(oid)?)?;
        let files = reader.list_files("")?;
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"test.txt".to_string()));
        assert!(files.contains(&"dir/test.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_directory_reader_exists() -> Result<()> {
        let dir = tempdir()?;

        std::fs::write(dir.path().join("test.txt"), "test")?;

        let reader = DirReader::open(dir.path().to_path_buf());
        assert!(reader.exists("test.txt"));
        assert!(!reader.exists("test2.txt"));

        Ok(())
    }

    #[test]
    fn test_commit_reader_exists() -> Result<()> {
        let repository = test_repository()?;

        std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;

        let oid = commit(&repository)?;

        std::fs::remove_file(repository.path().parent().unwrap().join("test.txt"))?;

        let reader = CommitReader::from_commit(&repository, repository.find_commit(oid)?)?;
        assert!(reader.exists("test.txt"));
        assert!(!reader.exists("test2.txt"));

        Ok(())
    }
}
