use std::path;

use filetime::FileTime;

use super::{Error, Oid, Repository, Result, Tree};

pub struct Index {
    index: git2::Index,
}

impl TryFrom<Tree<'_>> for Index {
    type Error = Error;

    fn try_from(value: Tree<'_>) -> std::result::Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl TryFrom<&Tree<'_>> for Index {
    type Error = Error;

    fn try_from(value: &Tree) -> Result<Self> {
        let mut empty_index = Self::new()?;
        empty_index.read_tree(value)?;
        Ok(empty_index)
    }
}

impl<'a> From<&'a mut Index> for &'a mut git2::Index {
    fn from(index: &'a mut Index) -> Self {
        &mut index.index
    }
}

impl From<git2::Index> for Index {
    fn from(index: git2::Index) -> Self {
        Self { index }
    }
}

impl Index {
    pub fn new() -> Result<Self> {
        Ok(Index {
            index: git2::Index::new()?,
        })
    }

    pub fn add_all<I, T>(
        &mut self,
        pathspecs: I,
        flag: git2::IndexAddOption,
        cb: Option<&mut git2::IndexMatchedPath<'_>>,
    ) -> Result<()>
    where
        T: git2::IntoCString,
        I: IntoIterator<Item = T>,
    {
        self.index.add_all(pathspecs, flag, cb).map_err(Into::into)
    }

    pub fn conflicts(&self) -> Result<git2::IndexConflicts> {
        self.index.conflicts().map_err(Into::into)
    }

    pub fn read_tree(&mut self, tree: &Tree) -> Result<()> {
        self.index.read_tree(tree.into()).map_err(Into::into)
    }

    pub fn write_tree_to(&mut self, repo: &Repository) -> Result<Oid> {
        self.index
            .write_tree_to(repo.into())
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn has_conflicts(&self) -> bool {
        self.index.has_conflicts()
    }

    pub fn write_tree(&mut self) -> Result<Oid> {
        self.index.write_tree().map(Into::into).map_err(Into::into)
    }

    pub fn add(&mut self, entry: &IndexEntry) -> Result<()> {
        self.index.add(&entry.clone().into()).map_err(Into::into)
    }

    pub fn write(&mut self) -> Result<()> {
        self.index.write().map_err(Into::into)
    }

    pub fn add_path(&mut self, path: &path::Path) -> Result<()> {
        self.index.add_path(path).map_err(Into::into)
    }

    pub fn remove_path(&mut self, path: &path::Path) -> Result<()> {
        self.index.remove_path(path).map_err(Into::into)
    }

    pub fn get_path(&self, path: &path::Path, stage: i32) -> Option<IndexEntry> {
        self.index.get_path(path, stage).map(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub ctime: FileTime,
    pub mtime: FileTime,
    pub dev: u32,
    pub ino: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub file_size: u32,
    pub id: Oid,
    pub flags: u16,
    pub flags_extended: u16,
    pub path: Vec<u8>,
}

impl From<git2::IndexEntry> for IndexEntry {
    fn from(value: git2::IndexEntry) -> Self {
        Self {
            ctime: FileTime::from_unix_time(
                i64::from(value.ctime.seconds()),
                value.ctime.nanoseconds(),
            ),
            mtime: FileTime::from_unix_time(
                i64::from(value.mtime.seconds()),
                value.mtime.nanoseconds(),
            ),
            dev: value.dev,
            ino: value.ino,
            mode: value.mode,
            uid: value.uid,
            gid: value.gid,
            file_size: value.file_size,
            id: value.id.into(),
            flags: value.flags,
            flags_extended: value.flags_extended,
            path: value.path,
        }
    }
}

impl From<IndexEntry> for git2::IndexEntry {
    #[allow(clippy::cast_possible_truncation)]
    fn from(entry: IndexEntry) -> Self {
        Self {
            ctime: git2::IndexTime::new(entry.ctime.seconds() as i32, entry.ctime.nanoseconds()),
            mtime: git2::IndexTime::new(entry.mtime.seconds() as i32, entry.mtime.nanoseconds()),
            dev: entry.dev,
            ino: entry.ino,
            mode: entry.mode,
            uid: entry.uid,
            gid: entry.gid,
            file_size: entry.file_size,
            id: entry.id.into(),
            flags: entry.flags,
            flags_extended: entry.flags_extended,
            path: entry.path,
        }
    }
}
