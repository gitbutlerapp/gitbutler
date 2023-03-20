use std::collections::HashMap;

use crate::{projects, sessions};
use anyhow::Result;

mod current;
mod persistent;

#[derive(Clone)]
pub struct Store {
    project: projects::Project,
    current: current::Store,
    persistent: persistent::Store,
}

impl Store {
    pub fn new(git_repository: git2::Repository, project: projects::Project) -> Result<Self> {
        Ok(Self {
            project: project.clone(),
            current: current::Store::new(git_repository, project.clone())?,
            persistent: persistent::Store::new(
                git2::Repository::open(&project.path)?,
                project.clone(),
            )?,
        })
    }

    pub fn list_files(
        &self,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        return self.persistent.list_files(session_id, paths);
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
            return self.current.get();
        }
        return self.persistent.get_by_id(session_id);
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
