use crate::{deltas, projects, sessions};
use anyhow::{anyhow, Context, Result};
use std::{collections::HashMap, path::Path};

use super::{current, persistent};

pub struct Store {
    project: projects::Project,
    git_repository: git2::Repository,

    persistent: persistent::Store,
    current: current::Store,

    sessions_store: sessions::Store,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            git_repository: git2::Repository::open(&self.project.path).unwrap(),
            current: self.current.clone(),
            persistent: self.persistent.clone(),
            sessions_store: self.sessions_store.clone(),
        }
    }
}

impl Store {
    pub fn new(
        git_repository: git2::Repository,
        project: projects::Project,
        sessions_store: sessions::Store,
    ) -> Result<Self> {
        Ok(Self {
            project: project.clone(),
            git_repository,
            current: current::Store::new(project.clone()),
            persistent: persistent::Store::new(project)?,
            sessions_store,
        })
    }

    pub fn read<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Vec<deltas::Delta>>> {
        self.current.read(file_path)
    }

    pub fn write<P: AsRef<Path>>(
        &self,
        file_path: P,
        deltas: &Vec<deltas::Delta>,
    ) -> Result<sessions::Session> {
        // make sure we always have a session before writing deltas
        let session = match self.sessions_store.get_current()? {
            Some(mut session) => {
                session
                    .touch(&self.project)
                    .with_context(|| format!("failed to touch session {}", session.id))?;
                Ok(session)
            }
            None => sessions::Session::from_head(&self.git_repository, &self.project),
        }?;

        self.current.write(file_path, deltas)?;

        Ok(session)
    }

    pub fn list(
        &self,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let session = match self.sessions_store.get_by_id(session_id)? {
            Some(session) => Ok(session),
            None => Err(anyhow!("Session {} not found", session_id)),
        }?;

        if session.hash.is_none() {
            self.current.list(paths)
        } else {
            self.persistent.list(&session, paths)
        }
    }
}
