use std::path;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::events as app_events;
use gitbutler_core::{
    deltas, gb_repository, project_repository,
    projects::{self, ProjectId},
    sessions::{self, SessionId},
    users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    users: users::Controller,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let projects = value.state::<projects::Controller>().inner().clone();
            let users = value.state::<users::Controller>().inner().clone();
            let sessions_database = value.state::<sessions::Database>().inner().clone();
            let deltas_database = value.state::<deltas::Database>().inner().clone();
            let handler = Handler::new(
                app_data_dir,
                projects,
                users,
                sessions_database,
                deltas_database,
            );
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

impl Handler {
    fn new(
        local_data_dir: path::PathBuf,
        projects: projects::Controller,
        users: users::Controller,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
    ) -> Handler {
        Handler {
            local_data_dir,
            projects,
            users,
            sessions_database,
            deltas_database,
        }
    }

    pub fn index_deltas(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        file_path: &path::Path,
        deltas: &Vec<deltas::Delta>,
    ) -> Result<()> {
        self.deltas_database
            .insert(project_id, session_id, file_path, deltas)
            .context("failed to insert deltas into database")?;
        Ok(())
    }

    pub fn reindex(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let user = self.users.get_user()?;
        let project = self.projects.get(project_id)?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
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
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
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

        // now, index session if it has changed to the database.
        let from_db = self.sessions_database.get_by_id(&session.id)?;
        if from_db.is_some() && from_db.unwrap() == *session {
            return Ok(vec![]);
        }

        self.sessions_database
            .insert(project_id, &[session])
            .context("failed to insert session into database")?;

        let session_reader = sessions::Reader::open(gb_repository, session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        for (file_path, deltas) in deltas_reader
            .read(None)
            .context("could not list deltas for session")?
        {
            self.index_deltas(project_id, &session.id, &file_path, &deltas)?;
        }

        Ok(vec![events::Event::Emit(app_events::Event::session(
            project_id, session,
        ))])
    }
}
