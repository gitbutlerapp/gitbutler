use std::path;

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    bookmarks, deltas, events as app_events, gb_repository,
    paths::DataDir,
    project_repository,
    projects::{self, ProjectId},
    sessions::{self, SessionId},
    users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
    bookmarks: bookmarks::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
            sessions_database: sessions::Database::from(value),
            deltas_database: deltas::Database::from(value),
            bookmarks: bookmarks::Controller::try_from(value)?,
        })
    }
}

impl Handler {
    pub fn index_deltas(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
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
        project_id: &ProjectId,
        bookmark: &bookmarks::Bookmark,
    ) -> Result<Vec<events::Event>> {
        let updated = self.bookmarks.upsert(bookmark)?;
        if let Some(updated) = updated {
            Ok(vec![events::Event::Emit(app_events::Event::bookmark(
                project_id, &updated,
            ))])
        } else {
            Ok(vec![])
        }
    }

    pub fn reindex(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let user = self.users.get_user()?;
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let sessions_iter = gb_repository.get_sessions_iterator()?;
        let mut events = vec![];
        for session in sessions_iter {
            events.extend(self.process_session(&gb_repository, &session?)?);
        }
        Ok(events)
    }

    pub fn index_session(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let user = self.users.get_user()?;
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        self.process_session(&gb_repository, session)
    }

    fn process_session(
        &self,
        gb_repository: &gb_repository::Repository,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let project_id = gb_repository.get_project_id();

        // index bookmarks right away. bookmarks are stored in the session during which it was
        // created, not in the session that is actually bookmarked. so we want to make sure all of
        // them are indexed at all times
        let session_reader = sessions::Reader::open(gb_repository, session)?;
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
        {
            let delta_events = self.index_deltas(project_id, &session.id, &file_path, &deltas)?;
            events.extend(delta_events);
        }

        Ok(events)
    }
}
