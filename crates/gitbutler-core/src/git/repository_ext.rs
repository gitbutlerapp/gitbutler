use anyhow::Result;
use git2::{Repository, Tree};
use tracing::instrument;

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    /// Based on the index, add all data similar to `git add .` and create a tree from it, which is returned.
    fn get_wd_tree(&self) -> Result<Tree>;
}

impl RepositoryExt for Repository {
    #[instrument(level = tracing::Level::DEBUG, skip(self), err(Debug))]
    fn get_wd_tree(&self) -> Result<Tree> {
        let mut index = self.index()?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
        let oid = index.write_tree()?;
        self.find_tree(oid).map(Into::into).map_err(Into::into)
    }
}
