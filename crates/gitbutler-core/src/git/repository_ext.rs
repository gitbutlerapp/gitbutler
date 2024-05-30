use anyhow::{bail, Context, Result};
use git2::{Repository, Tree};
use tracing::instrument;

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    /// Based on the index, add all data similar to `git add .` and create a tree from it, which is returned.
    fn get_wd_tree(&self) -> Result<Tree>;

    /// Returns the `gitbutler/integration` branch if the head currently points to it, or fail otherwise.
    /// Use it before any modification to the repository, or extra defensively each time the
    /// integration is needed.
    ///
    /// This is for safety to assure the repository actually is in 'gitbutler mode'.
    fn integration_ref_from_head(&self) -> Result<git2::Reference<'_>>;
}

impl RepositoryExt for Repository {
    #[instrument(level = tracing::Level::DEBUG, skip(self), err(Debug))]
    fn get_wd_tree(&self) -> Result<Tree> {
        let mut index = self.index()?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
        let oid = index.write_tree()?;
        self.find_tree(oid).map(Into::into).map_err(Into::into)
    }

    fn integration_ref_from_head(&self) -> Result<git2::Reference<'_>> {
        let head_ref = self.head().context("BUG: head must point to a reference")?;
        if head_ref.name_bytes() == b"refs/heads/gitbutler/integration" {
            Ok(head_ref)
        } else {
            bail!("Unexpected state: cannot perform operation on non-integration branch")
        }
    }
}
