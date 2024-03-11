use std::{num, path, str};

use anyhow::{Context, Result};
use serde::{ser::SerializeStruct, Serialize};

use crate::{fs, git, lock};

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("file not found")]
    NotFound,
    #[error("io error: {0}")]
    Io(std::sync::Arc<std::io::Error>),
    #[error(transparent)]
    From(FromError),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(std::sync::Arc::new(error))
    }
}

impl From<FromError> for Error {
    fn from(error: FromError) -> Self {
        Error::From(error)
    }
}

pub enum Reader<'reader> {
    Filesystem(FilesystemReader),
    Commit(CommitReader<'reader>),
    Prefixed(PrefixedReader<'reader>),
}

impl<'reader> Reader<'reader> {
    pub fn open<P: AsRef<path::Path>>(root: P) -> Result<Self, std::io::Error> {
        FilesystemReader::open(root).map(Reader::Filesystem)
    }

    pub fn sub<P: AsRef<path::Path>>(&'reader self, prefix: P) -> Self {
        Reader::Prefixed(PrefixedReader::new(self, prefix))
    }

    pub fn commit_id(&self) -> Option<git::Oid> {
        match self {
            Reader::Filesystem(_) => None,
            Reader::Commit(reader) => Some(reader.get_commit_oid()),
            Reader::Prefixed(reader) => reader.reader.commit_id(),
        }
    }

    pub fn from_commit(
        repository: &'reader git::Repository,
        commit: &git::Commit<'reader>,
    ) -> Result<Self> {
        Ok(Reader::Commit(CommitReader::new(repository, commit)?))
    }

    pub fn exists<P: AsRef<path::Path>>(&self, file_path: P) -> Result<bool, std::io::Error> {
        match self {
            Reader::Filesystem(reader) => reader.exists(file_path),
            Reader::Commit(reader) => Ok(reader.exists(file_path)),
            Reader::Prefixed(reader) => reader.exists(file_path),
        }
    }

    pub fn read<P: AsRef<path::Path>>(&self, path: P) -> Result<Content, Error> {
        let mut contents = self.batch(&[path])?;
        contents
            .pop()
            .expect("batch should return at least one result")
    }

    pub fn batch<P: AsRef<path::Path>>(
        &self,
        paths: &[P],
    ) -> Result<Vec<Result<Content, Error>>, std::io::Error> {
        match self {
            Reader::Filesystem(reader) => reader.batch(|root| {
                paths
                    .iter()
                    .map(|path| {
                        let path = root.join(path);
                        if !path.exists() {
                            return Err(Error::NotFound);
                        }
                        let content = Content::try_from(&path)?;
                        Ok(content)
                    })
                    .collect()
            }),
            Reader::Commit(reader) => Ok(paths
                .iter()
                .map(|path| reader.read(path.as_ref()))
                .collect()),
            Reader::Prefixed(reader) => reader.batch(paths),
        }
    }

    pub fn list_files<P: AsRef<std::path::Path>>(&self, dir_path: P) -> Result<Vec<path::PathBuf>> {
        match self {
            Reader::Filesystem(reader) => reader.list_files(dir_path.as_ref()),
            Reader::Commit(reader) => reader.list_files(dir_path.as_ref()),
            Reader::Prefixed(reader) => reader.list_files(dir_path.as_ref()),
        }
    }
}

pub struct FilesystemReader(lock::Dir);

impl FilesystemReader {
    fn open<P: AsRef<std::path::Path>>(root: P) -> Result<Self, std::io::Error> {
        lock::Dir::new(root).map(Self)
    }

    fn exists<P: AsRef<std::path::Path>>(&self, path: P) -> Result<bool, std::io::Error> {
        let exists = self.0.batch(|root| root.join(path.as_ref()).exists())?;
        Ok(exists)
    }

    fn batch<R>(&self, action: impl FnOnce(&std::path::Path) -> R) -> Result<R, std::io::Error> {
        self.0.batch(action)
    }

    fn list_files<P: AsRef<std::path::Path>>(&self, path: P) -> Result<Vec<path::PathBuf>> {
        let path = path.as_ref();
        self.0
            .batch(|root| fs::list_files(root.join(path).as_path(), &[path::Path::new(".git")]))?
    }
}

pub struct CommitReader<'reader> {
    repository: &'reader git::Repository,
    commit_oid: git::Oid,
    tree: git::Tree<'reader>,
}

impl<'reader> CommitReader<'reader> {
    fn new(
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

    fn read<P: AsRef<std::path::Path>>(&self, path: P) -> Result<Content, Error> {
        let path = path.as_ref();
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

    fn list_files<P: AsRef<std::path::Path>>(&self, dir_path: P) -> Result<Vec<path::PathBuf>> {
        let dir_path = dir_path.as_ref();
        let mut files = vec![];
        self.tree
            .walk(|root, entry| {
                if entry.kind() == Some(git2::ObjectType::Tree) {
                    return git::TreeWalkResult::Continue;
                }

                if entry.name().is_none() {
                    return git::TreeWalkResult::Continue;
                }
                let entry_path = std::path::Path::new(root).join(entry.name().unwrap());

                if !entry_path.starts_with(dir_path) {
                    return git::TreeWalkResult::Continue;
                }

                files.push(entry_path.strip_prefix(dir_path).unwrap().to_path_buf());

                git::TreeWalkResult::Continue
            })
            .with_context(|| format!("{}: tree walk failed", dir_path.display()))?;

        Ok(files)
    }

    fn exists<P: AsRef<std::path::Path>>(&self, file_path: P) -> bool {
        self.tree.get_path(file_path.as_ref()).is_ok()
    }
}

pub struct PrefixedReader<'r> {
    reader: &'r Reader<'r>,
    prefix: path::PathBuf,
}

impl<'r> PrefixedReader<'r> {
    fn new<P: AsRef<path::Path>>(reader: &'r Reader, prefix: P) -> Self {
        PrefixedReader {
            reader,
            prefix: prefix.as_ref().to_path_buf(),
        }
    }

    pub fn batch<P: AsRef<path::Path>>(
        &self,
        paths: &[P],
    ) -> Result<Vec<Result<Content, Error>>, std::io::Error> {
        let paths = paths
            .iter()
            .map(|path| self.prefix.join(path))
            .collect::<Vec<_>>();
        self.reader.batch(paths.as_slice())
    }

    fn list_files<P: AsRef<std::path::Path>>(&self, dir_path: P) -> Result<Vec<path::PathBuf>> {
        self.reader.list_files(self.prefix.join(dir_path.as_ref()))
    }

    fn exists<P: AsRef<std::path::Path>>(&self, file_path: P) -> Result<bool, std::io::Error> {
        self.reader.exists(self.prefix.join(file_path.as_ref()))
    }
}

#[derive(Debug, Clone, thiserror::Error)]
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

impl TryFrom<&Content> for usize {
    type Error = FromError;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        match content {
            Content::UTF8(text) => text.parse().map_err(FromError::ParseInt),
            Content::Binary => Err(FromError::Binary),
            Content::Large => Err(FromError::Large),
        }
    }
}

impl TryFrom<Content> for usize {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for String {
    type Error = FromError;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        match content {
            Content::UTF8(text) => Ok(text.clone()),
            Content::Binary => Err(FromError::Binary),
            Content::Large => Err(FromError::Large),
        }
    }
}

impl TryFrom<Content> for String {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<Content> for i64 {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for i64 {
    type Error = FromError;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(FromError::ParseInt)
    }
}

impl TryFrom<Content> for u64 {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for u64 {
    type Error = FromError;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(FromError::ParseInt)
    }
}

impl TryFrom<Content> for u128 {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for u128 {
    type Error = FromError;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(FromError::ParseInt)
    }
}

impl TryFrom<Content> for bool {
    type Error = FromError;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for bool {
    type Error = FromError;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(FromError::ParseBool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;

    use crate::tests;

    #[test]
    fn test_directory_reader_read_file() -> Result<()> {
        let dir = tests::temp_dir();

        let file_path = path::Path::new("test.txt");
        std::fs::write(dir.join(file_path), "test")?;

        let reader = Reader::open(dir.clone())?;
        assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_read_file() -> Result<()> {
        let repository = tests::test_repository();

        let file_path = path::Path::new("test.txt");
        std::fs::write(repository.path().parent().unwrap().join(file_path), "test")?;

        let oid = tests::commit_all(&repository);

        std::fs::write(repository.path().parent().unwrap().join(file_path), "test2")?;

        let reader = Reader::from_commit(&repository, &repository.find_commit(oid)?)?;
        assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

        Ok(())
    }

    #[test]
    fn test_reader_list_files_should_return_relative() -> Result<()> {
        let dir = tests::temp_dir();

        std::fs::write(dir.join("test1.txt"), "test")?;
        std::fs::create_dir_all(dir.join("dir"))?;
        std::fs::write(dir.join("dir").join("test.txt"), "test")?;

        let reader = Reader::open(dir.clone())?;
        let files = reader.list_files(path::Path::new("dir"))?;
        assert_eq!(files.len(), 1);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_reader_list_files() -> Result<()> {
        let dir = tests::temp_dir();

        std::fs::write(dir.join("test.txt"), "test")?;
        std::fs::create_dir_all(dir.join("dir"))?;
        std::fs::write(dir.join("dir").join("test.txt"), "test")?;

        let reader = Reader::open(dir.clone())?;
        let files = reader.list_files(path::Path::new(""))?;
        assert_eq!(files.len(), 2);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));
        assert!(files.contains(&path::Path::new("dir/test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_list_files_should_return_relative() -> Result<()> {
        let repository = tests::test_repository();

        std::fs::write(
            repository.path().parent().unwrap().join("test1.txt"),
            "test",
        )?;
        std::fs::create_dir_all(repository.path().parent().unwrap().join("dir"))?;
        std::fs::write(
            repository
                .path()
                .parent()
                .unwrap()
                .join("dir")
                .join("test.txt"),
            "test",
        )?;

        let oid = tests::commit_all(&repository);

        std::fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

        let reader = CommitReader::new(&repository, &repository.find_commit(oid)?)?;
        let files = reader.list_files(path::Path::new("dir"))?;
        assert_eq!(files.len(), 1);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_commit_reader_list_files() -> Result<()> {
        let repository = tests::test_repository();

        std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;
        std::fs::create_dir_all(repository.path().parent().unwrap().join("dir"))?;
        std::fs::write(
            repository
                .path()
                .parent()
                .unwrap()
                .join("dir")
                .join("test.txt"),
            "test",
        )?;

        let oid = tests::commit_all(&repository);

        std::fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

        let reader = CommitReader::new(&repository, &repository.find_commit(oid)?)?;
        let files = reader.list_files(path::Path::new(""))?;
        assert_eq!(files.len(), 2);
        assert!(files.contains(&path::Path::new("test.txt").to_path_buf()));
        assert!(files.contains(&path::Path::new("dir/test.txt").to_path_buf()));

        Ok(())
    }

    #[test]
    fn test_directory_reader_exists() -> Result<()> {
        let dir = tests::temp_dir();

        std::fs::write(dir.join("test.txt"), "test")?;

        let reader = Reader::open(dir.clone())?;
        assert!(reader.exists(path::Path::new("test.txt"))?);
        assert!(!reader.exists(path::Path::new("test2.txt"))?);

        Ok(())
    }

    #[test]
    fn test_commit_reader_exists() -> Result<()> {
        let repository = tests::test_repository();

        std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;

        let oid = tests::commit_all(&repository);

        std::fs::remove_file(repository.path().parent().unwrap().join("test.txt"))?;

        let reader = CommitReader::new(&repository, &repository.find_commit(oid)?)?;
        assert!(reader.exists(path::Path::new("test.txt")));
        assert!(!reader.exists(path::Path::new("test2.txt")));

        Ok(())
    }

    #[test]
    fn test_from_bytes() {
        for (bytes, expected) in [
            ("test".as_bytes(), Content::UTF8("test".to_string())),
            (&[0, 159, 146, 150, 159, 146, 150], Content::Binary),
        ] {
            assert_eq!(Content::from(bytes), expected);
        }
    }

    #[test]
    fn test_serialize_content() {
        for (content, expected) in [
            (
                Content::UTF8("test".to_string()),
                r#"{"type":"utf8","value":"test"}"#,
            ),
            (Content::Binary, r#"{"type":"binary"}"#),
            (Content::Large, r#"{"type":"large"}"#),
        ] {
            assert_eq!(serde_json::to_string(&content).unwrap(), expected);
        }
    }
}
