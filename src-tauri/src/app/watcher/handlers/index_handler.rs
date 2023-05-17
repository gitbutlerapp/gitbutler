use anyhow::{Context, Result};

use crate::app::{deltas, files, gb_repository, search, sessions};

use super::events;

pub struct Handler<'handler> {
    project_id: String,
    deltas_searcher: search::Deltas,
    gb_repository: &'handler gb_repository::Repository,
    files_database: files::Database,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
}

impl<'handler> Handler<'handler> {
    pub fn new(
        project_id: String,
        deltas_searcher: search::Deltas,
        gb_repository: &'handler gb_repository::Repository,
        files_database: files::Database,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
    ) -> Self {
        Self {
            project_id,
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
            .insert(&self.project_id, session_id, file_path, deltas)
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
            .insert(&self.project_id, session_id, file_path, content)
            .context("failed to insert file into database")?;
        Ok(vec![])
    }

    pub fn index_session(
        &self,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        self.deltas_searcher
            .index_session(&self.gb_repository, &session)
            .context("failed to index session")?;

        let from_db = self.sessions_database.get_by_id(&session.id)?;
        if from_db.is_some() && from_db.unwrap() == *session {
            return Ok(vec![]);
        }

        self.sessions_database
            .insert(&self.project_id, &vec![session])
            .context("failed to insert session into database")?;

        let mut events: Vec<events::Event> = vec![];

        let session_reader = sessions::Reader::open(&self.gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);

        for (file_path, content) in session_reader
            .files(None)
            .context("could not list files for session")?
        {
            let file_events = self.index_file(&session.id, &file_path, &content)?;
            events.extend(file_events);
        }

        for (file_path, deltas) in deltas_reader
            .read(None)
            .context("could not list deltas for session")?
            .into_iter()
        {
            let delta_events = self.index_deltas(&session.id, &file_path, &deltas)?;
            events.extend(delta_events);
        }

        Ok(events)
    }
}
