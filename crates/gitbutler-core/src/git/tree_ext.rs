use crate::path::Normalize;
use anyhow::Result;
use git2::TreeEntry;
use std::path::Path;

/// Extension trait for `git2::Tree`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Tree`
pub trait TreeExt {
    fn get_path<P: AsRef<Path>>(&self, path: P) -> Result<TreeEntry<'_>>;
}

impl<'repo> TreeExt for git2::Tree<'repo> {
    fn get_path<P: AsRef<Path>>(&self, path: P) -> Result<TreeEntry<'repo>> {
        self.get_path(path.normalize().as_path())
            .map(Into::into)
            .map_err(Into::into)
    }
}
