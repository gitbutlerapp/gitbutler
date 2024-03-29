use std::{
    fs, io, num,
    path::{Path, PathBuf},
    str,
    sync::Arc,
};

use anyhow::{Context, Result};
use serde::{ser::SerializeStruct, Serialize};

use crate::{git, lock, path::Normalize};

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("file not found")]
    NotFound,
    #[error("io error: {0}")]
    Io(Arc<io::Error>),
    #[error(transparent)]
    From(FromError),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(Arc::new(error))
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
    pub fn open<P: AsRef<Path>>(root: P) -> Result<Self, io::Error> {
        FilesystemReader::open(root).map(Reader::Filesystem)
    }

    pub fn sub<P: AsRef<Path>>(&'reader self, prefix: P) -> Self {
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

    pub fn exists<P: AsRef<Path>>(&self, file_path: P) -> Result<bool, io::Error> {
        match self {
            Reader::Filesystem(reader) => reader.exists(file_path),
            Reader::Commit(reader) => Ok(reader.exists(file_path)),
            Reader::Prefixed(reader) => reader.exists(file_path),
        }
    }

    pub fn read<P: AsRef<Path>>(&self, path: P) -> Result<Content, Error> {
        let mut contents = self.batch(&[path])?;
        contents
            .pop()
            .expect("batch should return at least one result")
    }

    pub fn batch<P: AsRef<Path>>(
        &self,
        paths: &[P],
    ) -> Result<Vec<Result<Content, Error>>, io::Error> {
        match self {
            Reader::Filesystem(reader) => reader.batch(|root| {
                paths
                    .iter()
                    .map(|path| {
                        let path = root.join(path);
                        if !path.exists() {
                            return Err(Error::NotFound);
                        }
                        let content = Content::read_from_file(&path)?;
                        Ok(content)
                    })
                    .collect()
            }),
            Reader::Commit(reader) => Ok(paths
                .iter()
                .map(|path| reader.read(path.normalize()))
                .collect()),
            Reader::Prefixed(reader) => reader.batch(paths),
        }
    }

    pub fn list_files<P: AsRef<Path>>(&self, dir_path: P) -> Result<Vec<PathBuf>> {
        match self {
            Reader::Filesystem(reader) => reader.list_files(dir_path.as_ref()),
            Reader::Commit(reader) => reader.list_files(dir_path.as_ref()),
            Reader::Prefixed(reader) => reader.list_files(dir_path.as_ref()),
        }
    }
}

pub struct FilesystemReader(lock::Dir);

impl FilesystemReader {
    fn open<P: AsRef<Path>>(root: P) -> Result<Self, io::Error> {
        lock::Dir::new(root).map(Self)
    }

    fn exists<P: AsRef<Path>>(&self, path: P) -> Result<bool, io::Error> {
        let exists = self.0.batch(|root| root.join(path.as_ref()).exists())?;
        Ok(exists)
    }

    fn batch<R>(&self, action: impl FnOnce(&Path) -> R) -> Result<R, io::Error> {
        self.0.batch(action)
    }

    fn list_files<P: AsRef<Path>>(&self, path: P) -> Result<Vec<PathBuf>> {
        let path = path.as_ref();
        self.0
            .batch(|root| crate::fs::list_files(root.join(path).as_path(), &[Path::new(".git")]))?
    }
}

pub struct CommitReader<'reader> {
    repository: &'reader git::Repository,
    commit_oid: git::Oid,
    tree: git::Tree<'reader>,
}

impl<'reader> CommitReader<'reader> {
    pub fn new(
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

    fn read<P: AsRef<Path>>(&self, path: P) -> Result<Content, Error> {
        let path = path.as_ref();
        let entry = match self
            .tree
            .get_path(Path::new(path))
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

    pub fn list_files<P: AsRef<Path>>(&self, dir_path: P) -> Result<Vec<PathBuf>> {
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
                let entry_path = Path::new(root).join(entry.name().unwrap());

                if !entry_path.starts_with(dir_path) {
                    return git::TreeWalkResult::Continue;
                }

                files.push(entry_path.strip_prefix(dir_path).unwrap().to_path_buf());

                git::TreeWalkResult::Continue
            })
            .with_context(|| format!("{}: tree walk failed", dir_path.display()))?;

        Ok(files)
    }

    pub fn exists<P: AsRef<Path>>(&self, file_path: P) -> bool {
        self.tree.get_path(file_path.normalize()).is_ok()
    }
}

pub struct PrefixedReader<'r> {
    reader: &'r Reader<'r>,
    prefix: PathBuf,
}

impl<'r> PrefixedReader<'r> {
    fn new<P: AsRef<Path>>(reader: &'r Reader, prefix: P) -> Self {
        PrefixedReader {
            reader,
            prefix: prefix.as_ref().to_path_buf(),
        }
    }

    pub fn batch<P: AsRef<Path>>(
        &self,
        paths: &[P],
    ) -> Result<Vec<Result<Content, Error>>, io::Error> {
        let paths = paths
            .iter()
            .map(|path| self.prefix.join(path))
            .collect::<Vec<_>>();
        self.reader.batch(paths.as_slice())
    }

    fn list_files<P: AsRef<Path>>(&self, dir_path: P) -> Result<Vec<PathBuf>> {
        self.reader.list_files(self.prefix.join(dir_path.as_ref()))
    }

    fn exists<P: AsRef<Path>>(&self, file_path: P) -> Result<bool, io::Error> {
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

    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)?;
        if metadata.len() > Content::MAX_SIZE as u64 {
            return Ok(Content::Large);
        }
        let content = fs::read(path)?;
        Ok(content.as_slice().into())
    }
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
