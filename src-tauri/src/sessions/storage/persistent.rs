use crate::{projects, sessions};
use anyhow::{Context, Result};

pub struct Store {
    project: projects::Project,
    git_repository: git2::Repository,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            git_repository: git2::Repository::open(&self.project.path).unwrap(),
        }
    }
}

impl Store {
    pub fn new(git_repository: git2::Repository, project: projects::Project) -> Result<Self> {
        Ok(Self {
            project: project.clone(),
            git_repository,
        })
    }

    // returns list of sessions in reverse chronological order
    // except for the first session. The first created session
    // is special and used to bootstrap the gitbutler state inside a repo.
    // see crate::repositories::inib
    pub fn list(&self, earliest_timestamp_ms: Option<u128>) -> Result<Vec<sessions::Session>> {
        let reference = self
            .git_repository
            .find_reference(&self.project.refname())?;
        let head = self
            .git_repository
            .find_commit(reference.target().unwrap())?;

        // list all commits from gitbutler head to the first commit
        let mut walker = self.git_repository.revwalk()?;
        walker.push(head.id())?;
        walker.set_sorting(git2::Sort::TOPOLOGICAL)?;

        let mut sessions: Vec<sessions::Session> = vec![];
        for id in walker {
            let id = id?;
            let commit = self.git_repository.find_commit(id).with_context(|| {
                format!(
                    "failed to find commit {} in repository {}",
                    id.to_string(),
                    self.git_repository.path().display()
                )
            })?;
            let session = sessions::Session::from_commit(&self.git_repository, &commit)?;
            match earliest_timestamp_ms {
                Some(earliest_timestamp_ms) => {
                    if session.meta.start_timestamp_ms <= earliest_timestamp_ms {
                        break;
                    }
                }
                None => {}
            }
            sessions.push(session);
        }

        // drop the first session, which is the bootstrap session
        sessions.pop();

        Ok(sessions)
    }
}
