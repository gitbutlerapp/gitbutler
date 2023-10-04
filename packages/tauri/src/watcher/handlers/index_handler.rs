use std::path;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    bookmarks, deltas, events as app_events, gb_repository, projects, search, sessions, users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    project_store: projects::Storage,
    user_store: users::Storage,
    deltas_searcher: search::Searcher,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
    bookmarks_database: bookmarks::Database,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("failed to get local data dir")?;
        Ok(Self {
            local_data_dir: local_data_dir.to_path_buf(),
            project_store: projects::Storage::try_from(value)?,
            user_store: users::Storage::try_from(value)?,
            deltas_searcher: value.state::<search::Searcher>().inner().clone(),
            sessions_database: sessions::Database::try_from(value)?,
            deltas_database: deltas::Database::try_from(value)?,
            bookmarks_database: bookmarks::Database::try_from(value)?,
        })
    }
}

impl Handler {
    pub fn index_deltas(
        &self,
        project_id: &str,
        session_id: &str,
        file_path: &path::Path,
        deltas: &Vec<deltas::Delta>,
    ) -> Result<Vec<events::Event>> {
        self.deltas_database
            .insert(project_id, session_id, file_path, deltas)
            .context("failed to insert deltas into database")?;
        Ok(vec![])
    }

    pub fn index_bookmark(
        &self,
        project_id: &str,
        bookmark: &bookmarks::Bookmark,
    ) -> Result<Vec<events::Event>> {
        let updated = self.bookmarks_database.upsert(bookmark)?;
        self.deltas_searcher.index_bookmark(bookmark)?;
        if let Some(updated) = updated {
            Ok(vec![events::Event::Emit(app_events::Event::bookmark(
                project_id, &updated,
            ))])
        } else {
            Ok(vec![])
        }
    }

    pub fn reindex(&self, project_id: &str) -> Result<Vec<events::Event>> {
        let user = self.user_store.get()?;
        let project = self
            .project_store
            .get_project(project_id)?
            .context(format!("failed to get project with id {}", project_id))?;

        let gb_repository =
            gb_repository::Repository::open(self.local_data_dir.clone(), &project, user.as_ref())
                .context("failed to open repository")?;

        let sessions_iter = gb_repository.get_sessions_iterator()?;
        let mut events = vec![];
        for session in sessions_iter {
            events.extend(self.index_session(project_id, &session?)?);
        }
        Ok(events)
    }

    pub fn index_session(
        &self,
        project_id: &str,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let user = self.user_store.get()?;
        let project = self
            .project_store
            .get_project(project_id)?
            .context(format!("failed to get project with id {}", project_id))?;

        let gb_repository =
            gb_repository::Repository::open(self.local_data_dir.clone(), &project, user.as_ref())
                .context("failed to open repository")?;

        // first of all, index session for searching. searhcer keeps it's own state to
        // decide if the actual indexing needed
        self.deltas_searcher
            .index_session(&gb_repository, session)
            .context("failed to index session")?;

        // index bookmarks right away. bookmarks are stored in the session during which it was
        // created, not in the session that is actually bookmarked. so we want to make sure all of
        // them are indexed at all times
        let session_reader = sessions::Reader::open(&gb_repository, session)?;
        let bookmarks_reader = bookmarks::Reader::new(&session_reader);
        for bookmark in bookmarks_reader.read()? {
            self.index_bookmark(project_id, &bookmark)?;
        }

        // now, index session if it has changed to the database.
        let from_db = self.sessions_database.get_by_id(&session.id)?;
        if from_db.is_some() && from_db.unwrap() == *session {
            return Ok(vec![]);
        }

        self.sessions_database
            .insert(project_id, &[session])
            .context("failed to insert session into database")?;

        let mut events: Vec<events::Event> = vec![events::Event::Emit(app_events::Event::session(
            project_id, session,
        ))];

        let deltas_reader = deltas::Reader::new(&session_reader);
        for (file_path, deltas) in deltas_reader
            .read(None)
            .context("could not list deltas for session")?
            .into_iter()
        {
            let delta_events = self.index_deltas(project_id, &session.id, &file_path, &deltas)?;
            events.extend(delta_events);
        }

        Ok(events)
    }
}
