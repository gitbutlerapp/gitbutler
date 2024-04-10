use std::path::Path;

use anyhow::{Context, Result};
use gitbutler_core::{
    deltas, gb_repository, project_repository,
    projects::ProjectId,
    sessions::{self, SessionId},
};

use crate::events as app_events;

impl super::Handler {
    pub(super) fn index_deltas(
        &self,
        project_id: ProjectId,
        session_id: SessionId,
        file_path: &Path,
        deltas: &[deltas::Delta],
    ) -> Result<()> {
        self.deltas_db
            .insert(&project_id, &session_id, file_path, deltas)
            .context("failed to insert deltas into database")
    }

    pub fn reindex(&self, project_id: ProjectId) -> Result<()> {
        let user = self.users.get_user()?;
        let project = self.projects.get(&project_id)?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let sessions_iter = gb_repository.get_sessions_iterator()?;
        for session in sessions_iter {
            self.process_session(&gb_repository, &session?)?;
        }
        Ok(())
    }

    pub(super) fn index_session(
        &self,
        project_id: ProjectId,
        session: &sessions::Session,
    ) -> Result<()> {
        let project = self.projects.get(&project_id)?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let user = self.users.get_user()?;
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
    ) -> Result<()> {
        let project_id = gb_repository.get_project_id();

        // now, index session if it has changed to the database.
        let from_db = self.sessions_db.get_by_id(&session.id)?;
        if from_db.map_or(false, |from_db| from_db == *session) {
            return Ok(());
        }

        self.sessions_db
            .insert(project_id, &[session])
            .context("failed to insert session into database")?;

        let session_reader = sessions::Reader::open(gb_repository, session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        for (file_path, deltas) in deltas_reader
            .read(None)
            .context("could not list deltas for session")?
        {
            self.index_deltas(*project_id, session.id, &file_path, &deltas)?;
        }

        (self.send_event)(&app_events::Event::session(*project_id, session))?;
        Ok(())
    }
}
