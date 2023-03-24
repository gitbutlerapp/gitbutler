use crate::{projects, sessions, users};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time,
};

mod current;
mod persistent;

#[derive(Clone)]
pub struct Store {
    project: projects::Project,
    current: current::Store,
    persistent: persistent::Store,
}

impl Store {
    pub fn new(git_repository: Arc<Mutex<git2::Repository>>, project: projects::Project) -> Self {
        Self {
            current: current::Store::new(git_repository.clone(), project.clone()),
            persistent: persistent::Store::new(git_repository, project.clone()),
            project,
        }
    }

    pub fn create_current(&self) -> Result<sessions::Session> {
        self.current.create()
    }

    pub fn flush(
        &self,
        session: &sessions::Session,
        user: Option<users::User>,
    ) -> Result<sessions::Session> {
        let meta = session.meta.clone();
        let updated_time = sessions::Session {
            id: session.id.clone(),
            hash: session.hash.clone(),
            activity: session.activity.clone(),
            meta: sessions::Meta {
                last_timestamp_ms: time::SystemTime::now()
                    .duration_since(time::SystemTime::UNIX_EPOCH)?
                    .as_millis(),
                ..meta
            },
        };
        self.current.update(&updated_time)?;
        let flushed_session = self.persistent.flush(user, &updated_time)?;
        self.current.delete()?;
        Ok(flushed_session)
    }

    pub fn update(&self, session: &sessions::Session) -> Result<()> {
        if session.hash.is_some() {
            Err(anyhow::anyhow!("cannot update session that is not current"))
        } else {
            self.current.update(session)
        }
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
        let mut sessions = self
            .persistent
            .list(earliest_timestamp_ms)
            .with_context(|| "failed to list sessions for project {}")?;
        if let Some(session) = self
            .current
            .get()
            .with_context(|| "failed to get current session")?
        {
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
