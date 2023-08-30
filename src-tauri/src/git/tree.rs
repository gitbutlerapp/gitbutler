use std::path;

use super::Result;

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
    pub fn id(&self) -> git2::Oid {
        self.tree.id()
    }

    pub fn get_path(&self, path: &path::Path) -> Result<git2::TreeEntry<'repo>> {
        self.tree.get_path(path)
    }

    pub fn walk<C, T>(&self, mode: git2::TreeWalkMode, callback: C) -> Result<()>
    where
        C: FnMut(&str, &git2::TreeEntry) -> T,
        T: Into<i32>,
    {
        self.tree.walk(mode, callback)
    }
}
