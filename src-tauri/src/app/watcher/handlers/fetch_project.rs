use std::time;

use anyhow::{Context, Result};

use crate::{
    app::{gb_repository, search},
    projects,
};

use super::events;

pub struct Handler<'handler> {
    project_id: String,
    project_storage: projects::Storage,
    deltas_searcher: search::Deltas,
    gb_repository: &'handler gb_repository::Repository,
}

impl<'handler> Handler<'handler> {
    pub fn new(
        project_id: String,
        project_storage: projects::Storage,
        deltas_searcher: search::Deltas,
        gb_repository: &'handler gb_repository::Repository,
    ) -> Self {
        Self {
            project_id,
            project_storage,
            deltas_searcher,
            gb_repository,
        }
    }

    pub fn handle(&self) -> Result<Vec<events::Event>> {
        let sessions_before_fetch = self
            .gb_repository
            .get_sessions_iterator()?
            .filter_map(|s| s.ok())
            .collect::<Vec<_>>();
        if !self.gb_repository.fetch().context("failed to fetch")? {
            return Ok(vec![]);
        }

        self.project_storage
            .update_project(&projects::UpdateRequest {
                id: self.project_id.clone(),
                last_fetched_ts: Some(
                    time::SystemTime::now()
                        .duration_since(time::UNIX_EPOCH)
                        .context("failed to get time since epoch")?
                        .as_millis()
                        .try_into()
                        .context("failed to convert time to millis")?,
                ),
                ..Default::default()
            })
            .context("failed to update project")?;

        let sessions_after_fetch = self
            .gb_repository
            .get_sessions_iterator()?
            .filter_map(|s| s.ok())
            .collect::<Vec<_>>();

        let new_sessions = sessions_after_fetch
            .iter()
            .filter(|s| !sessions_before_fetch.contains(s))
            .collect::<Vec<_>>();

        for session in &new_sessions {
            if let Err(e) = self
                .deltas_searcher
                .index_session(&self.gb_repository, &session)
            {
                log::error!("{}: failed to index session: {:#}", self.project_id, e);
            }
        }

        let events = new_sessions
            .into_iter()
            .cloned()
            .map(|session| events::Event::Session(session))
            .collect::<Vec<_>>();

        Ok(events)
    }
}
