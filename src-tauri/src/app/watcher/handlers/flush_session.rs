use anyhow::{anyhow, Context, Result};

use crate::{
    app::{gb_repository, project_repository},
    projects, sessions,
};

use super::events;

pub struct Handler<'handler> {
    project_id: String,
    project_store: projects::Storage,
    gb_repository: &'handler gb_repository::Repository,
}

impl<'listener> Handler<'listener> {
    pub fn new(
        project_id: String,
        project_store: projects::Storage,
        gb_repository: &'listener gb_repository::Repository,
    ) -> Self {
        Self {
            project_id,
            gb_repository,
            project_store,
        }
    }

    pub fn handle(&self, session: &sessions::Session) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get_project(&self.project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow!("project not found"))?;

        let session = self
            .gb_repository
            .flush_session(&project_repository::Repository::open(&project)?, session)
            .context("failed to flush session")?;

        Ok(vec![
            events::Event::Session((project, session.clone())),
            events::Event::SessionFlushed(session),
        ])
    }
}
