use crate::{projects, sessions};
use anyhow::{anyhow, Context, Ok, Result};

pub struct Repository {
    pub(crate) project_id: String,
    pub(crate) git_repository: git2::Repository,
}

impl Repository {
    pub fn open(project: &projects::Project) -> Result<Self> {
        let git_repository = git2::Repository::open(&project.path)
            .with_context(|| format!("{}: failed to open git repository", project.path))?;
        Ok(Self {
            project_id: project.id.clone(),
            git_repository,
        })
    }

    pub fn sessions(&self) -> Result<Vec<sessions::Session>> {
        Err(anyhow!("TODO"))
    }

    pub(crate) fn session_path(&self) -> std::path::PathBuf {
        self.git_repository.path().parent().unwrap().join("session")
    }

    pub(crate) fn deltas_path(&self) -> std::path::PathBuf {
        self.session_path().join("deltas")
    }

    pub(crate) fn wd_path(&self) -> std::path::PathBuf {
        self.session_path().join("wd")
    }

    pub(crate) fn logs_path(&self) -> std::path::PathBuf {
        self.git_repository.path().parent().unwrap().join("logs")
    }
}
