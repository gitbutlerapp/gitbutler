use crate::{projects, sessions};
use anyhow::Result;

mod current;
mod persistent;

pub struct Store {
    project: projects::Project,
    git_repository: git2::Repository,

    current: current::Store,
    persistent: persistent::Store,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            git_repository: git2::Repository::open(&self.project.path).unwrap(),
            current: self.current.clone(),
            persistent: self.persistent.clone(),
        }
    }
}

impl Store {
    pub fn new(git_repository: git2::Repository, project: projects::Project) -> Result<Self> {
        Ok(Self {
            project: project.clone(),
            git_repository,
            current: current::Store::new(git2::Repository::open(&project.path)?, project.clone())?,
            persistent: persistent::Store::new(
                git2::Repository::open(&project.path)?,
                project.clone(),
            )?,
        })
    }

    // returns list of sessions in reverse chronological order
    pub fn list(&self, earliest_timestamp_ms: Option<u128>) -> Result<Vec<sessions::Session>> {
        let mut sessions = self.persistent.list(earliest_timestamp_ms)?;
        if let Some(session) = self.current.get()? {
            sessions.insert(0, session);
        }
        Ok(sessions)
    }

    pub fn get_current(&self) -> Result<Option<sessions::Session>> {
        self.current.get()
    }

    pub fn get_by_id(&self, session_id: &str) -> Result<Option<sessions::Session>> {
        if is_current_session_id(&self.project, session_id)? {
            return self.get_current();
        }

        let reference = self
            .git_repository
            .find_reference(&self.project.refname())?;
        let head = self
            .git_repository
            .find_commit(reference.target().unwrap())?;
        let mut walker = self.git_repository.revwalk()?;
        walker.push(head.id())?;
        walker.set_sorting(git2::Sort::TIME)?;

        for commit_id in walker {
            let commit = self.git_repository.find_commit(commit_id?)?;
            if sessions::id_from_commit(&self.git_repository, &commit)? == session_id {
                return Ok(Some(sessions::Session::from_commit(
                    &self.git_repository,
                    &commit,
                )?));
            }
        }

        Ok(None)
    }
}

fn is_current_session_id(project: &projects::Project, session_id: &str) -> Result<bool> {
    let current_id_path = project.session_path().join("meta").join("id");
    if !current_id_path.exists() {
        return Ok(false);
    }
    let current_id = std::fs::read_to_string(current_id_path)?;
    return Ok(current_id == session_id);
}
