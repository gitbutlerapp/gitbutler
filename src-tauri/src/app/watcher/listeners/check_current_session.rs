use std::{sync, time};

use anyhow::{Context, Result};

use crate::{app::gb_repository, events, projects, search};

use super::flush_session;

pub struct Listener<'listener> {
    gb_repository: sync::Arc<sync::Mutex<&'listener gb_repository::Repository>>,
    flush_session_listener: flush_session::Listener<'listener>,
}

const FIVE_MINUTES: u128 = time::Duration::new(5 * 60, 0).as_millis();
const ONE_HOUR: u128 = time::Duration::new(60 * 60, 0).as_millis();

impl<'listener> Listener<'listener> {
    pub fn new(
        project_id: String,
        project_store: projects::Storage,
        gb_repository: &'listener gb_repository::Repository,
        deltas_searcher: search::Deltas,
        sender: sync::mpsc::Sender<events::Event>,
    ) -> Self {
        Self {
            gb_repository: sync::Arc::new(sync::Mutex::new(gb_repository)),
            flush_session_listener: flush_session::Listener::new(
                project_id,
                project_store,
                gb_repository,
                deltas_searcher,
                sender,
            ),
        }
    }

    pub fn register(&self, ts: time::SystemTime) -> Result<()> {
        let current_session = self
            .gb_repository
            .lock()
            .unwrap()
            .get_current_session()
            .context("failed to get current session")?;
        if current_session.is_none() {
            return Ok(());
        }
        let current_session = current_session.unwrap();

        let now = ts
            .duration_since(time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let elapsed_last = now - current_session.meta.last_timestamp_ms;
        if elapsed_last < FIVE_MINUTES {
            return Ok(());
        }

        let elapsed_start = now - current_session.meta.start_timestamp_ms;
        if elapsed_start < ONE_HOUR {
            return Ok(());
        }

        self.flush_session_listener
            .register(&current_session)
            .context("failed to flush session")?;

        Ok(())
    }
}
