use std::time;

use anyhow::{Context, Result};

use crate::{
    app::{deltas, files, gb_repository, search, sessions},
    projects,
};

use super::events;

pub struct Handler<'handler> {
    project_id: String,
    project_storage: projects::Storage,
    deltas_searcher: search::Deltas,
    gb_repository: &'handler gb_repository::Repository,
    files_database: files::Database,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
}

impl<'handler> Handler<'handler> {
    pub fn new(
        project_id: String,
        project_storage: projects::Storage,
        deltas_searcher: search::Deltas,
        gb_repository: &'handler gb_repository::Repository,
        files_database: files::Database,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
    ) -> Self {
        Self {
            project_id,
            project_storage,
            deltas_searcher,
            gb_repository,
            files_database,
            sessions_database,
            deltas_database,
        }
    }

    pub fn index_deltas(
        &self,
        session_id: &str,
        file_path: &str,
        deltas: &Vec<deltas::Delta>,
    ) -> Result<Vec<events::Event>> {
        self.deltas_database
            .insert(session_id, file_path, deltas)
            .context("failed to insert deltas into database")?;
        Ok(vec![])
    }

    pub fn index_file(
        &self,
        session_id: &str,
        file_path: &str,
        content: &str,
    ) -> Result<Vec<events::Event>> {
        self.files_database
            .insert(session_id, file_path, content)
            .context("failed to insert file into database")?;
        Ok(vec![])
    }

    pub fn index_session(&self, session: &sessions::Session) -> Result<Vec<events::Event>> {
        self.deltas_searcher
            .index_session(&self.gb_repository, &session)
            .context("failed to index session")?;

        self.sessions_database
            .insert(&self.project_id, &vec![session])
            .context("failed to insert session into database")?;

        let mut events: Vec<events::Event> = vec![];

        let session_reader = sessions::Reader::open(&self.gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);

        let deltas = deltas_reader
            .read(None)
            .with_context(|| "could not list deltas for session")?;
        let files = session_reader
            .files(Some(deltas.keys().map(|k| k.as_str()).collect()))
            .context("could not list files for session")?;

        for (file_path, content) in files.into_iter() {
            let file_events = self.index_file(&session.id, &file_path, &content)?;
            events.extend(file_events);
        }

        for (file_path, deltas) in deltas.into_iter() {
            let delta_events = self.index_deltas(&session.id, &file_path, &deltas)?;
            events.extend(delta_events);
        }

        Ok(events)
    }
}
