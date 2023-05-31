use anyhow::{Context, Result};

use crate::{bookmarks, deltas, events as app_events, files, gb_repository, search, sessions};

use super::events;

pub struct Handler<'handler> {
    project_id: String,
    deltas_searcher: search::Searcher,
    gb_repository: &'handler gb_repository::Repository,
    files_database: files::Database,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
    bookmarks_database: bookmarks::Database,
    events_sender: app_events::Sender,
}

impl<'handler> Handler<'handler> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_id: String,
        deltas_searcher: search::Searcher,
        gb_repository: &'handler gb_repository::Repository,
        files_database: files::Database,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
        bookmarks_database: bookmarks::Database,
        events_sender: app_events::Sender,
    ) -> Self {
        Self {
            project_id,
            deltas_searcher,
            gb_repository,
            files_database,
            sessions_database,
            deltas_database,
            bookmarks_database,
            events_sender,
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

    pub fn index_bookmark(&self, bookmark: &bookmarks::Bookmark) -> Result<Vec<events::Event>> {
        let updated = self.bookmarks_database.upsert(bookmark)?;
        self.deltas_searcher.index_bookmark(bookmark)?;
        if let Some(updated) = updated {
            self.events_sender
                .send(app_events::Event::bookmark(&self.project_id, &updated))?;
        }
        Ok(vec![])
    }

    pub fn index_session(&self, session: &sessions::Session) -> Result<Vec<events::Event>> {
        // first of all, index session for searching. searhcer keeps it's own state to
        // decide if the actual indexing needed
        self.deltas_searcher
            .index_session(self.gb_repository, session)
            .context("failed to index session")?;

        // index bookmarks right away. bookmarks are stored in the session during which it was
        // created, not in the session that is actually bookmarked. so we want to make sure all of
        // them are indexed at all times
        let session_reader = sessions::Reader::open(self.gb_repository, session)?;
        let bookmarks_reader = bookmarks::Reader::new(&session_reader);
        for bookmark in bookmarks_reader.read()? {
            self.index_bookmark(&bookmark)?;
        }

        // now, index session if it has changed to the database.
        let from_db = self.sessions_database.get_by_id(&session.id)?;
        if from_db.is_some() && from_db.unwrap() == *session {
            return Ok(vec![]);
        }

        self.sessions_database
            .insert(&self.project_id, &[session])
            .context("failed to insert session into database")?;

        let mut events: Vec<events::Event> = vec![];

        for (file_path, content) in session_reader
            .files(None)
            .context("could not list files for session")?
        {
            let file_events = self.index_file(&session.id, &file_path, &content)?;
            events.extend(file_events);
        }

        let deltas_reader = deltas::Reader::new(&session_reader);
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
