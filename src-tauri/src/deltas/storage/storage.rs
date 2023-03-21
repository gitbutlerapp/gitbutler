use crate::{deltas, projects, sessions};
use anyhow::{anyhow, Context, Result};
use std::{collections::HashMap, path::Path, time};

use super::{current, persistent};

#[derive(Clone)]
pub struct Store {
    persistent: persistent::Store,
    current: current::Store,
    sessions_store: sessions::Store,
}

impl Store {
    pub fn new(project: projects::Project, sessions_store: sessions::Store) -> Result<Self> {
        Ok(Self {
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
            Some(session) => {
                let updated_session = sessions::Session {
                    meta: sessions::Meta {
                        last_timestamp_ms: time::SystemTime::now()
                            .duration_since(time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis(),
                        ..session.meta
                    },
                    ..session
                };
                self.sessions_store
                    .update(&updated_session)
                    .with_context(|| format!("failed to touch session {}", updated_session.id))?;
                Ok(updated_session)
            }
            None => self.sessions_store.create_current(),
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
