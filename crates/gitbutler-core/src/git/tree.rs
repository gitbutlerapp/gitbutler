use std::path::Path;

use super::{Oid, Repository, Result};

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
    pub fn new(repo: &'repo Repository, base: Option<&'repo git2::Tree>) -> Self {
        TreeBuilder {
            repo: repo.into(),
            builder: git2::build::TreeUpdateBuilder::new(),
            base,
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
