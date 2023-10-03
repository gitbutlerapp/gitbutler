use std::{num, path, str};

use anyhow::{Context, Result};
use serde::{ser::SerializeStruct, Serialize};

use crate::{fs, git};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file not found")]
    NotFound,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    From(#[from] FromError),
}

pub trait Reader {
    fn read(&self, file_path: &path::Path) -> Result<Content, Error>;
    fn list_files(&self, dir_path: &path::Path) -> Result<Vec<path::PathBuf>>;
    fn is_dir(&self, file_path: &path::Path) -> bool;
    fn exists(&self, file_path: &path::Path) -> bool;
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
    fn is_dir(&self, file_path: &path::Path) -> bool {
        let path = self.root.join(file_path);
        path.exists() && path.is_dir()
    }

    fn exists(&self, file_path: &path::Path) -> bool {
        let path = self.root.join(file_path);
        path.exists()
    }

    fn read(&self, path: &path::Path) -> Result<Content, Error> {
        let path = self.root.join(path);
        if !path.exists() {
            return Err(Error::NotFound);
        }
        let content = Content::try_from(&path).map_err(Error::Io)?;
        Ok(content)
    }

    fn list_files(&self, dir_path: &path::Path) -> Result<Vec<path::PathBuf>> {
        fs::list_files(self.root.join(dir_path)).map(|files| {
            files
                .into_iter()
                .filter(|f| !f.starts_with(".git"))
                .collect::<Vec<_>>()
        })
    }
}

pub struct CommitReader<'reader> {
    repository: &'reader git::Repository,
    commit_oid: git::Oid,
    tree: git::Tree<'reader>,
}

impl<'reader> CommitReader<'reader> {
    pub fn from_commit(
        repository: &'reader git::Repository,
        commit: &git::Commit<'reader>,
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

    pub fn get_commit_oid(&self) -> git::Oid {
        self.commit_oid
    }
}

impl Reader for CommitReader<'_> {
    fn is_dir(&self, file_path: &path::Path) -> bool {
        let entry = match self
            .tree
            .get_path(std::path::Path::new(file_path))
            .context(format!("{}: tree entry not found", file_path.display()))
        {
            Ok(entry) => entry,
            Err(_) => return false,
        };
        entry.kind() == Some(git2::ObjectType::Tree)
    }

    fn read(&self, path: &path::Path) -> Result<Content, Error> {
        let entry = match self
            .tree
            .get_path(std::path::Path::new(path))
            .context(format!("{}: tree entry not found", path.display()))
        {
            Ok(entry) => entry,
            Err(_) => return Err(Error::NotFound),
        };
        let blob = match self.repository.find_blob(entry.id()) {
            Ok(blob) => blob,
            Err(_) => return Err(Error::NotFound),
        };
        Ok(Content::from(&blob))
    }

    fn list_files(&self, dir_path: &path::Path) -> Result<Vec<path::PathBuf>> {
        let mut files = vec![];
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

                files.push(entry_path.strip_prefix(dir_path).unwrap().to_path_buf());

                git2::TreeWalkResult::Ok
            })
            .with_context(|| format!("{}: tree walk failed", dir_path.display()))?;

        Ok(files)
    }

    fn exists(&self, file_path: &path::Path) -> bool {
        self.tree.get_path(file_path).is_ok()
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
    fn is_dir(&self, file_path: &path::Path) -> bool {
        self.reader.is_dir(&self.prefix.join(file_path))
    }

    fn read(&self, path: &path::Path) -> Result<Content, Error> {
        self.reader.read(&self.prefix.join(path))
    }

    fn list_files(&self, dir_path: &path::Path) -> Result<Vec<path::PathBuf>> {
        self.reader.list_files(&self.prefix.join(dir_path))
    }

    fn exists(&self, file_path: &path::Path) -> bool {
        self.reader.exists(&self.prefix.join(file_path))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FromError {
    #[error(transparent)]
    ParseInt(#[from] num::ParseIntError),
    #[error(transparent)]
    ParseBool(#[from] str::ParseBoolError),
    #[error("file is binary")]
    Binary,
    #[error("file too large")]
    Large,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    UTF8(String),
    Binary,
    Large,
}

impl Serialize for Content {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Content::UTF8(text) => {
                let mut state = serializer.serialize_struct("Content", 2)?;
                state.serialize_field("type", "utf8")?;
                state.serialize_field("value", text)?;
                state.end()
            }
            Content::Binary => {
                let mut state = serializer.serialize_struct("Content", 1)?;
                state.serialize_field("type", "binary")?;
                state.end()
            }
            Content::Large => {
                let mut state = serializer.serialize_struct("Content", 1)?;
                state.serialize_field("type", "large")?;
                state.end()
            }
        }
    }
}

impl Content {
    const MAX_SIZE: usize = 1024 * 1024 * 10; // 10 MB
}

impl From<&str> for Content {
    fn from(text: &str) -> Self {
        if text.len() > Self::MAX_SIZE {
            Content::Large
        } else {
            Content::UTF8(text.to_string())
        }
    }
}

impl TryFrom<&path::PathBuf> for Content {
    type Error = std::io::Error;

    fn try_from(value: &path::PathBuf) -> Result<Self, Self::Error> {
        let metadata = std::fs::metadata(value)?;
        if metadata.len() > Content::MAX_SIZE as u64 {
            return Ok(Content::Large);
        }
        let content = std::fs::read(value)?;
        Ok(content.as_slice().into())
    }
}

impl From<&git::Blob<'_>> for Content {
    fn from(value: &git::Blob) -> Self {
        if value.size() > Content::MAX_SIZE {
            Content::Large
        } else {
            value.content().into()
        }
    }
}

impl From<&[u8]> for Content {
    fn from(bytes: &[u8]) -> Self {
        if bytes.len() > Self::MAX_SIZE {
            Content::Large
        } else {
            match String::from_utf8(bytes.to_vec()) {
                Err(_) => Content::Binary,
                Ok(text) => Content::UTF8(text),
            }
        }
    }
}

impl TryFrom<Content> for usize {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        match content {
            Content::UTF8(text) => text.parse().map_err(FromError::ParseInt),
            Content::Binary => Err(FromError::Binary),
            Content::Large => Err(FromError::Large),
        }
    }
}

impl TryFrom<Content> for String {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        match content {
            Content::UTF8(text) => Ok(text),
            Content::Binary => Err(FromError::Binary),
            Content::Large => Err(FromError::Large),
        }
    }
}

impl TryFrom<Content> for u128 {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(FromError::ParseInt)
    }
}

impl TryFrom<Content> for bool {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(FromError::ParseBool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;

    use crate::test_utils;

    #[test]
    fn test_directory_reader_is_dir() -> Result<()> {
        let dir = test_utils::temp_dir();
        let reader = DirReader::open(dir.clone());
        std::fs::create_dir(dir.join("dir"))?;
        std::fs::write(dir.join("dir/test.txt"), "test")?;
        assert!(reader.is_dir(path::Path::new(".")));
        assert!(reader.is_dir(path::Path::new("dir")));
        assert!(!reader.is_dir(path::Path::new("dir/test.txt")));
        assert!(!reader.is_dir(path::Path::new("404.txt")));
        Ok(())
    }

    #[test]
    fn test_directory_reader_read_file() -> Result<()> {
        let dir = test_utils::temp_dir();

        let file_path = path::Path::new("test.txt");
        std::fs::write(dir.join(file_path), "test")?;

        let reader = DirReader::open(dir.to_path_buf());
        assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_is_dir() -> Result<()> {
        let repository = test_utils::test_repository();

        std::fs::create_dir(repository.path().parent().unwrap().join("dir"))?;
        std::fs::write(
            repository.path().parent().unwrap().join("dir/test.txt"),
            "test",
        )?;
        let oid = test_utils::commit_all(&repository);

        let reader = CommitReader::from_commit(&repository, &repository.find_commit(oid)?)?;
        assert!(reader.is_dir(path::Path::new("dir")));
        assert!(!reader.is_dir(path::Path::new("dir/test.txt")));
        assert!(!reader.is_dir(path::Path::new("404.txt")));
        Ok(())
    }

    #[test]
    fn test_commit_reader_read_file() -> Result<()> {
        let repository = test_utils::test_repository();

        let file_path = path::Path::new("test.txt");
        std::fs::write(repository.path().parent().unwrap().join(file_path), "test")?;

        let oid = test_utils::commit_all(&repository);

        std::fs::write(repository.path().parent().unwrap().join(file_path), "test2")?;

        let reader = CommitReader::from_commit(&repository, &repository.find_commit(oid)?)?;
        assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

        Ok(())
    }

    #[test]
    fn test_reader_list_files_should_return_relative() -> Result<()> {
        let dir = test_utils::temp_dir();

        std::fs::write(dir.join("test1.txt"), "test")?;
        std::fs::create_dir(dir.join("dir"))?;
        std::fs::write(dir.join("dir").join("test.txt"), "test")?;

        let reader = DirReader::open(dir.to_path_buf());
        let files = reader.list_files(path::Path::new("dir"))?;
        assert_eq!(files.len(), 1);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_reader_list_files() -> Result<()> {
        let dir = test_utils::temp_dir();

        std::fs::write(dir.join("test.txt"), "test")?;
        std::fs::create_dir(dir.join("dir"))?;
        std::fs::write(dir.join("dir").join("test.txt"), "test")?;

        let reader = DirReader::open(dir.to_path_buf());
        let files = reader.list_files(path::Path::new(""))?;
        assert_eq!(files.len(), 2);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));
        assert!(files.contains(&path::Path::new("dir/test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_list_files_should_return_relative() -> Result<()> {
        let repository = test_utils::test_repository();

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

        let oid = test_utils::commit_all(&repository);

        std::fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

        let reader = CommitReader::from_commit(&repository, &repository.find_commit(oid)?)?;
        let files = reader.list_files(path::Path::new("dir"))?;
        assert_eq!(files.len(), 1);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_list_files() -> Result<()> {
        let repository = test_utils::test_repository();

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

        let oid = test_utils::commit_all(&repository);

        std::fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

        let reader = CommitReader::from_commit(&repository, &repository.find_commit(oid)?)?;
        let files = reader.list_files(path::Path::new(""))?;
        assert_eq!(files.len(), 2);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));
        assert!(files.contains(&path::Path::new("dir/test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_directory_reader_exists() -> Result<()> {
        let dir = test_utils::temp_dir();

        std::fs::write(dir.join("test.txt"), "test")?;

        let reader = DirReader::open(dir.to_path_buf());
        assert!(reader.exists(path::Path::new("test.txt")));
        assert!(!reader.exists(path::Path::new("test2.txt")));

        Ok(())
    }

    #[test]
    fn test_commit_reader_exists() -> Result<()> {
        let repository = test_utils::test_repository();

        std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;

        let oid = test_utils::commit_all(&repository);

        std::fs::remove_file(repository.path().parent().unwrap().join("test.txt"))?;

        let reader = CommitReader::from_commit(&repository, &repository.find_commit(oid)?)?;
        assert!(reader.exists(path::Path::new("test.txt")));
        assert!(!reader.exists(path::Path::new("test2.txt")));

        Ok(())
    }

    #[test]
    fn test_from_bytes() {
        vec![
            ("test".as_bytes(), Content::UTF8("test".to_string())),
            (&[0, 159, 146, 150, 159, 146, 150], Content::Binary),
        ]
        .into_iter()
        .for_each(|(bytes, expected)| {
            assert_eq!(Content::from(bytes), expected);
        });
    }

    #[test]
    fn test_serialize_content() {
        vec![
            (
                Content::UTF8("test".to_string()),
                r#"{"type":"utf8","value":"test"}"#,
            ),
            (Content::Binary, r#"{"type":"binary"}"#),
            (Content::Large, r#"{"type":"large"}"#),
        ]
        .into_iter()
        .for_each(|(content, expected)| {
            assert_eq!(serde_json::to_string(&content).unwrap(), expected);
        });
    }
}
