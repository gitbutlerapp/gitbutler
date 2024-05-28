use anyhow::Result;
use git2::{Repository, Tree};

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    fn get_wd_tree(&self) -> Result<Tree>;
}

impl RepositoryExt for Repository {
    fn get_wd_tree(&self) -> Result<Tree> {
        let mut index = self.index()?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
        let oid = index.write_tree()?;
        self.find_tree(oid).map(Into::into).map_err(Into::into)
    }
}
