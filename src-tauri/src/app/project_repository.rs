use anyhow::{Context, Result};

use crate::projects;

use super::reader;

pub struct Repository {
    git_repository: git2::Repository,
}

impl Repository {
    pub fn open(project: &projects::Project) -> Result<Self> {
        let git_repository = git2::Repository::open(&project.path)
            .with_context(|| format!("{}: failed to open git repository", project.path))?;
        Ok(Self { git_repository })
    }

    pub fn get_head(&self) -> Result<git2::Reference> {
        let head = self.git_repository.head()?;
        Ok(head)
    }

    pub fn is_path_ignored<P: AsRef<std::path::Path>>(&self, path: P) -> Result<bool> {
        let path = path.as_ref();
        let ignored = self.git_repository.is_path_ignored(path)?;
        Ok(ignored)
    }

    pub fn get_wd_reader(&self) -> reader::DirReader {
        reader::DirReader::open(self.root())
    }

    pub fn get_head_reader(&self) -> Result<reader::CommitReader> {
        let head = self.git_repository.head()?;
        let commit = head.peel_to_commit()?;
        let reader = reader::CommitReader::from_commit(&self.git_repository, commit)?;
        Ok(reader)
    }

    pub fn root(&self) -> &std::path::Path {
        self.git_repository.path().parent().unwrap()
    }
}
