use std::path::PathBuf;

use anyhow::{Context, Result};
use tempfile::tempdir;
use uuid::Uuid;

/// A temporary workdir created in a temporary directory
///
/// Gets cleaned up on Drop
/// Drop can panic
pub struct TemporaryWorkdir {
    directory: PathBuf,
    worktree: git2::Worktree,
    repo: git2::Repository,
    branch_name: Uuid,
    cleaned_up: bool,
}

impl TemporaryWorkdir {
    pub fn open(repository: &git2::Repository) -> Result<Self> {
        let directory = tempdir().context("Failed to create temporary directory")?;
        // By using into path, we need to deconstruct the TempDir ourselves
        let path = directory.into_path();
        let branch_name = Uuid::new_v4();
        let worktree = repository
            .worktree(&branch_name.to_string(), &path.join("repository"), None)
            .context("Failed to create worktree")?;
        let worktree_repository = git2::Repository::open_from_worktree(&worktree)
            .context("Failed to open worktree repository")?;

        Ok(TemporaryWorkdir {
            repo: worktree_repository,
            directory: path,
            worktree,
            branch_name,
            cleaned_up: false,
        })
    }

    pub fn repository(&self) -> &git2::Repository {
        if self.cleaned_up {
            panic!("Can not access repository after its been closed")
        }

        &self.repo
    }

    pub fn close(&mut self) -> Result<()> {
        if self.cleaned_up {
            return Ok(());
        }

        std::fs::remove_dir_all(&self.directory)?;
        self.worktree.prune(None)?;
        self.repo
            .find_branch(&self.branch_name.to_string(), git2::BranchType::Local)?
            .delete()?;

        self.cleaned_up = true;

        Ok(())
    }
}

impl Drop for TemporaryWorkdir {
    fn drop(&mut self) {
        self.close().expect("Failed to close temporary workdir")
    }
}
