use std::path;

use super::{Oid, Repository, Result};

pub struct Tree<'repo> {
    tree: git2::Tree<'repo>,
}

impl<'repo> From<git2::Tree<'repo>> for Tree<'repo> {
    fn from(tree: git2::Tree<'repo>) -> Self {
        Tree { tree }
    }
}

impl<'repo> From<&'repo Tree<'repo>> for &'repo git2::Tree<'repo> {
    fn from(tree: &'repo Tree<'repo>) -> Self {
        &tree.tree
    }
}

impl<'repo> Tree<'repo> {
    pub fn id(&self) -> Oid {
        self.tree.id().into()
    }

    pub fn get_path(&self, path: &path::Path) -> Result<TreeEntry<'repo>> {
        self.tree.get_path(path).map(Into::into)
    }

    pub fn walk<C, T>(&self, mode: git2::TreeWalkMode, mut callback: C) -> Result<()>
    where
        C: FnMut(&str, &TreeEntry) -> T,
        T: Into<i32>,
    {
        self.tree
            .walk(mode, |root, entry| callback(root, &entry.clone().into()))
    }

    pub fn get_name(&self, filename: &str) -> Option<TreeEntry> {
        self.tree.get_name(filename).map(Into::into)
    }
}

pub struct TreeEntry<'repo> {
    entry: git2::TreeEntry<'repo>,
}

impl<'repo> From<git2::TreeEntry<'repo>> for TreeEntry<'repo> {
    fn from(entry: git2::TreeEntry<'repo>) -> Self {
        TreeEntry { entry }
    }
}

impl<'repo> TreeEntry<'repo> {
    pub fn filemode(&self) -> i32 {
        self.entry.filemode()
    }

    pub fn to_object(&self, repo: &'repo Repository) -> Result<git2::Object> {
        self.entry.to_object(repo.into())
    }

    pub fn kind(&self) -> Option<git2::ObjectType> {
        self.entry.kind()
    }

    pub fn id(&self) -> Oid {
        self.entry.id().into()
    }

    pub fn name(&self) -> Option<&str> {
        self.entry.name()
    }
}
