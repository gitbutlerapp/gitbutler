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
    Other(std::io::Error),
}

pub trait Reader {
    fn read(&self, file_path: &str) -> Result<Content, Error>;
    fn list_files(&self, dir_path: &str) -> Result<Vec<String>>;
    fn exists(&self, file_path: &str) -> bool;
    fn size(&self, file_path: &str) -> Result<usize>;

    fn read_to_string(&self, file_path: &str) -> Result<String, Error> {
        match self.read(file_path)? {
            Content::UTF8(s) => Ok(s),
            Content::Binary(_) => Err(Error::Other(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "file is not utf8",
            ))),
        }
    }
}

pub struct DirReader<'reader> {
    root: &'reader std::path::Path,
}

impl<'reader> DirReader<'reader> {
    pub fn open(root: &'reader std::path::Path) -> Self {
        Self { root }
    }
}

impl Reader for DirReader<'_> {
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
        let content = std::fs::read(path).map_err(Error::Other)?;
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
        match String::from_utf8_lossy(&content).into_owned() {
            s if s.as_bytes().eq(content) => Ok(Content::UTF8(s)),
            _ => Ok(Content::Binary(content.to_vec())),
        }
    }

    fn list_files(&self, dir_path: &str) -> Result<Vec<String>> {
        let mut files: Vec<String> = Vec::new();
        let repo_root = self.repository.path().parent().unwrap();
        self.tree
            .walk(git2::TreeWalkMode::PreOrder, |root, entry| {
                if entry.name().is_none() {
                    return git2::TreeWalkResult::Ok;
                }

                let abs_dir_path = repo_root.join(dir_path);
                let abs_entry_path = repo_root.join(root).join(entry.name().unwrap());
                if !abs_entry_path.starts_with(&abs_dir_path) {
                    return git2::TreeWalkResult::Ok;
                }
                if abs_dir_path.eq(&abs_entry_path) {
                    return git2::TreeWalkResult::Ok;
                }
                if entry.kind() == Some(git2::ObjectType::Tree) {
                    return git2::TreeWalkResult::Ok;
                }

                let relpath = abs_entry_path.strip_prefix(abs_dir_path).unwrap();

                files.push(relpath.to_str().unwrap().to_string());

                git2::TreeWalkResult::Ok
            })
            .with_context(|| format!("{}: tree walk failed", dir_path))?;

        Ok(files)
    }

    fn exists(&self, file_path: &str) -> bool {
        self.tree.get_path(std::path::Path::new(file_path)).is_ok()
    }
}
