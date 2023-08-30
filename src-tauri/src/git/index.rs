use std::path;

use super::{Oid, Repository, Result, Tree};

pub struct Index {
    index: git2::Index,
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
        self.index.add_all(pathspecs, flag, cb)
    }

    pub fn conflicts(&self) -> Result<git2::IndexConflicts> {
        self.index.conflicts()
    }

    pub fn read_tree(&mut self, tree: &Tree) -> Result<()> {
        self.index.read_tree(tree.into())
    }

    pub fn write_tree_to(&mut self, repo: &Repository) -> Result<Oid> {
        self.index.write_tree_to(repo.into()).map(Into::into)
    }

    pub fn has_conflicts(&self) -> bool {
        self.index.has_conflicts()
    }

    pub fn write_tree(&mut self) -> Result<Oid> {
        self.index.write_tree().map(Into::into)
    }

    pub fn add(&mut self, entry: &git2::IndexEntry) -> Result<()> {
        self.index.add(entry)
    }

    pub fn write(&mut self) -> Result<()> {
        self.index.write()
    }

    pub fn add_path(&mut self, path: &path::Path) -> Result<()> {
        self.index.add_path(path)
    }

    pub fn remove_path(&mut self, path: &path::Path) -> Result<()> {
        self.index.remove_path(path)
    }

    pub fn get_path(&self, path: &path::Path, stage: i32) -> Option<git2::IndexEntry> {
        self.index.get_path(path, stage)
    }
}
