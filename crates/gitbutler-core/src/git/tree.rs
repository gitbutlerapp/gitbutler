use std::path::Path;

use super::{Oid, Repository, Result};
use crate::path::Normalize;

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

    pub fn get_path<P: AsRef<Path>>(&self, path: P) -> Result<TreeEntry<'repo>> {
        self.tree
            .get_path(path.normalize().as_path())
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn walk<C>(&self, mut callback: C) -> Result<()>
    where
        C: FnMut(&str, &TreeEntry) -> TreeWalkResult,
    {
        self.tree
            .walk(git2::TreeWalkMode::PreOrder, |root, entry| {
                match callback(root, &entry.clone().into()) {
                    TreeWalkResult::Continue => git2::TreeWalkResult::Ok,
                    TreeWalkResult::Skip => git2::TreeWalkResult::Skip,
                    TreeWalkResult::Stop => git2::TreeWalkResult::Abort,
                }
            })
            .map_err(Into::into)
    }

    pub fn get_name(&self, filename: &str) -> Option<TreeEntry> {
        self.tree.get_name(filename).map(Into::into)
    }
}

pub enum TreeWalkResult {
    Continue,
    Skip,
    Stop,
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
        self.entry.to_object(repo.into()).map_err(Into::into)
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

#[derive(PartialEq)]
pub enum FileMode {
    Blob,
    BlobExecutable,
    Link,
    Tree,
}

impl From<FileMode> for git2::FileMode {
    fn from(filemod: FileMode) -> Self {
        match filemod {
            FileMode::Blob => git2::FileMode::Blob,
            FileMode::BlobExecutable => git2::FileMode::BlobExecutable,
            FileMode::Link => git2::FileMode::Link,
            FileMode::Tree => git2::FileMode::Tree,
        }
    }
}

pub struct TreeBuilder<'repo> {
    repo: &'repo git2::Repository,
    builder: git2::build::TreeUpdateBuilder,
    base: Option<&'repo git2::Tree<'repo>>,
}

impl<'repo> TreeBuilder<'repo> {
    pub fn new(repo: &'repo Repository, base: Option<&'repo Tree>) -> Self {
        TreeBuilder {
            repo: repo.into(),
            builder: git2::build::TreeUpdateBuilder::new(),
            base: base.map(Into::into),
        }
    }

    pub fn upsert<P: AsRef<Path>>(&mut self, filename: P, oid: Oid, filemode: FileMode) {
        self.builder
            .upsert(filename.as_ref(), oid.into(), filemode.into());
    }

    pub fn remove<P: AsRef<Path>>(&mut self, filename: P) {
        self.builder.remove(filename.as_ref());
    }

    pub fn write(&mut self) -> Result<Oid> {
        let repo: &git2::Repository = self.repo;
        if let Some(base) = self.base {
            let tree_id = self.builder.create_updated(repo, base)?;
            Ok(tree_id.into())
        } else {
            let empty_tree_id = repo.treebuilder(None)?.write()?;
            let empty_tree = repo.find_tree(empty_tree_id)?;
            let tree_id = self.builder.create_updated(repo, &empty_tree)?;
            Ok(tree_id.into())
        }
    }
}
